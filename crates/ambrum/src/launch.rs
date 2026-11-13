// SPDX-License-Identifier: MPL-2.0
//! Authenticated standalone Vanilla sessions in AmberL-created instances only.
use crate::{
    Result,
    auth::Identity,
    download::Cache,
    java,
    manifest::{Platform, VersionPlan},
    registry::{DirectoryGuard, info, options},
    secure::Store,
    task::Cancellation,
    transaction::Installed,
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    os::windows::{fs::OpenOptionsExt, process::CommandExt},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM},
    Storage::FileSystem::FILE_SHARE_READ,
    UI::WindowsAndMessaging::*,
};
use zeroize::Zeroizing;

pub struct Selection<'a> {
    pub app_directory: &'a Path,
    pub game_directory: &'a Path,
    pub version: &'a str,
    pub java_executable: &'a Path,
    pub account_id: &'a str,
    pub max_memory_mib: u32,
    pub os_version: &'a str,
}
#[derive(Serialize)]
pub struct Preflight {
    pub version: String,
    pub game_directory: std::path::PathBuf,
    pub java_executable: std::path::PathBuf,
    pub java: java::Java,
    pub max_memory_mib: u32,
    pub resources_verified: bool,
}
#[derive(Serialize)]
pub enum Event {
    CheckingResources,
    Authenticating,
    Started {
        process_id: u32,
        readiness: &'static str,
    },
    CloseRequested {
        process_id: u32,
    },
}
#[derive(Serialize)]
pub struct Exit {
    pub process_id: u32,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub close_requested: bool,
    pub readiness: &'static str,
    pub diagnostic_tail: Vec<String>,
}

fn memory(value: u32) -> Result<()> {
    if !(512..=65536).contains(&value) {
        return Err("Memory must be explicitly selected between 512 and 65536 MiB".into());
    }
    Ok(())
}
struct Prepared {
    installed: Installed,
    plan: VersionPlan,
    metadata: serde_json::Value,
    report: Preflight,
    _java_parent: DirectoryGuard,
    _java: fs::File,
}
fn prepare(selection: &Selection<'_>, cancel: &Cancellation) -> Result<Prepared> {
    memory(selection.max_memory_mib)?;
    crate::paths::name(selection.version)?;
    let parent = selection
        .java_executable
        .parent()
        .ok_or("Missing Java parent")?;
    let java_parent = DirectoryGuard::open(parent)?;
    let executable = crate::paths::ordinary(selection.java_executable)?;
    if !executable
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("java.exe") || n.eq_ignore_ascii_case("javaw.exe"))
    {
        return Err("Select java.exe or javaw.exe".into());
    }
    let java_file = options()
        .share_mode(FILE_SHARE_READ)
        .open(&executable)
        .map_err(|_| "Selected Java is unavailable")?;
    if info(&java_file)?.nNumberOfLinks != 1 {
        return Err("Linked Java executable rejected".into());
    }
    let java = java::inspect(&executable, cancel)?;
    let cache = Cache::open(selection.app_directory)?;
    let resolved =
        crate::catalog::resolve(&cache, selection.version, &crate::download::Https, cancel)?;
    let metadata = crate::paths::json(
        resolved
            .metadata_path
            .parent()
            .ok_or("Missing metadata parent")?,
        &resolved.metadata_path,
    )?;
    let mut installed = Installed::open(selection.game_directory, cancel)?;
    let installed_metadata =
        installed.json(&format!("versions/{0}/{0}.json", selection.version))?;
    crate::loader::verify_metadata(&metadata, &installed_metadata)?;
    let metadata = installed_metadata;
    let plan = VersionPlan::parse(&metadata, &Platform::windows_x64(selection.os_version))?;
    java::compatible(&java, plan.java_major)?;
    installed.verify_official(&plan, cancel)?;
    installed.check_game_paths(cancel)?;
    let report = Preflight {
        version: plan.version.clone(),
        game_directory: installed.root.clone(),
        java_executable: executable,
        java,
        max_memory_mib: selection.max_memory_mib,
        resources_verified: true,
    };
    Ok(Prepared {
        installed,
        plan,
        metadata,
        report,
        _java_parent: java_parent,
        _java: java_file,
    })
}
/// Checks the chosen Java and immutable installation without authenticating or starting a process.
pub fn preflight(selection: &Selection<'_>, cancel: &Cancellation) -> Result<Preflight> {
    Ok(prepare(selection, cancel)?.report)
}

fn java_path(path: &Path) -> Result<String> {
    let text = path.to_str().ok_or("Java paths must be valid Unicode")?;
    // Java does not consistently accept Windows verbatim path syntax.
    if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        return Ok(format!(r"\\{unc}"));
    }
    Ok(text.strip_prefix(r"\\?\").unwrap_or(text).to_owned())
}
fn expand(value: &str, values: &BTreeMap<&str, String>) -> Result<String> {
    let mut output = String::new();
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        output.push_str(&rest[..start]);
        let suffix = &rest[start + 2..];
        let end = suffix.find('}').ok_or("Unterminated launch placeholder")?;
        output.push_str(
            values
                .get(&suffix[..end])
                .ok_or("Unsupported launch placeholder")?,
        );
        rest = &suffix[end + 1..];
    }
    output.push_str(rest);
    if output.contains('\0') {
        return Err("Invalid launch argument".into());
    }
    Ok(output)
}
fn arguments(
    prepared: &Prepared,
    identity: &Identity,
    native: &Path,
) -> Result<Zeroizing<Vec<String>>> {
    if !identity.is_current() {
        return Err("Login expired before game start".into());
    }
    let plan = &prepared.plan;
    let root = &prepared.installed.root;
    let mut values = BTreeMap::from([
        ("auth_player_name", identity.profile.name.clone()),
        ("auth_uuid", identity.profile.id.clone()),
        ("version_name", plan.version.clone()),
        (
            "version_type",
            prepared
                .metadata
                .get("type")
                .and_then(|v| v.as_str())
                .ok_or("Missing official version type")?
                .into(),
        ),
        ("game_directory", java_path(root)?),
        ("assets_root", java_path(&root.join("assets"))?),
        ("assets_index_name", plan.asset_index.clone()),
        ("natives_directory", java_path(native)?),
        ("library_directory", java_path(&root.join("libraries"))?),
        ("classpath_separator", ";".into()),
        // Relative entries shorten the Windows command line; the pinned instance is always cwd.
        ("classpath", plan.classpath.join(";")),
        ("launcher_name", "AmberL".into()),
        ("launcher_version", env!("CARGO_PKG_VERSION").into()),
        ("clientid", crate::MICROSOFT_CLIENT_ID.into()),
        ("auth_xuid", identity.xuid.clone()),
        ("user_type", "msa".into()),
        ("user_properties", "{}".into()),
    ]);
    let mut args = Zeroizing::new(vec![
        format!("-Xmx{}M", prepared.report.max_memory_mib),
        "-Xms256M".into(),
    ]);
    for arg in &plan.jvm_arguments {
        args.push(expand(arg, &values)?);
    }
    if let Some(logging) = &plan.logging_argument {
        let resource = plan
            .files
            .iter()
            .find(|f| f.role == "logging")
            .ok_or("Missing logging configuration")?;
        values.insert("path", java_path(&root.join(&resource.path))?);
        args.push(expand(logging, &values)?);
    }
    args.push(plan.main_class.clone());
    // Keep credential replacement last and in zeroizing argument storage.
    for arg in &plan.game_arguments {
        if arg == "${auth_access_token}" {
            args.push(identity.token.expose().into());
        } else {
            args.push(expand(arg, &values)?);
        }
    }
    // Conservatively account for quoting, escaping, executable and NUL.
    let units: usize = args.iter().map(|a| a.encode_utf16().count() * 2 + 3).sum();
    if units + prepared.report.java_executable.as_os_str().len() * 2 + 4 >= 32767 {
        return Err("Launch command exceeds the Windows argument budget".into());
    }
    Ok(args)
}

pub fn run(
    selection: &Selection<'_>,
    cancel: &Cancellation,
    mut notify: impl FnMut(Event),
) -> Result<Exit> {
    notify(Event::CheckingResources);
    let prepared = prepare(selection, cancel)?;
    let root = &prepared.installed.root;
    let natives = root.join("natives");
    match fs::create_dir(&natives) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err("Cannot create owned native directory".into()),
    }
    let _native = DirectoryGuard::open(&natives)?;
    notify(Event::Authenticating);
    let mut store = Store::open(selection.app_directory)?;
    let identity = store.authenticate(selection.account_id, &crate::auth::http::Https, cancel)?;
    let args = arguments(&prepared, &identity, &natives)?;
    cancel.check()?;
    let mut command = Command::new(&prepared.report.java_executable);
    command
        .args(args.iter())
        .current_dir(root)
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .env_remove("CLASSPATH")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000);
    let mut child = command.spawn().map_err(|e| {
        format!(
            "Cannot start selected Java (Windows {})",
            e.raw_os_error().unwrap_or(0)
        )
    })?;
    // std::process owns its own argument copies until Command is dropped.
    drop(command);
    drop(args);
    drop(store);
    let process_id = child.id();
    let stdout = output::drain(child.stdout.take().unwrap(), identity.token.clone());
    let stderr = output::drain(child.stderr.take().unwrap(), identity.token.clone());
    notify(Event::Started {
        process_id,
        readiness: "unknown",
    });
    let mut close_requested = false;
    let mut close_sent = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Cannot observe owned Java process; owned process stopped".into());
            }
        }
        if cancel.check().is_err() && !close_requested {
            close_requested = true;
            notify(Event::CloseRequested { process_id });
        }
        if close_requested && !close_sent {
            close_sent = request_close(process_id);
        }
        // Keep locks and handles until the owned child exits. Cancellation sends WM_CLOSE,
        // never bulk-kills Java or reports successful exit while the game is still alive.
        thread::sleep(Duration::from_millis(100));
    };
    let mut diagnostic_tail = output::finish(stdout);
    diagnostic_tail.extend(output::finish(stderr));
    Ok(Exit {
        process_id,
        exit_code: status.code(),
        success: status.success(),
        close_requested,
        readiness: "unknown",
        diagnostic_tail,
    })
}
fn request_close(process_id: u32) -> bool {
    unsafe extern "system" fn close(window: HWND, data: LPARAM) -> i32 {
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(window, &mut pid);
            let context = &mut *(data as *mut (u32, bool));
            if pid == context.0 && IsWindowVisible(window) != 0 {
                context.1 |= PostMessageW(window, WM_CLOSE, 0, 0) != 0;
            }
        }
        1
    }
    let mut context = (process_id, false);
    unsafe {
        EnumWindows(Some(close), &mut context as *mut (u32, bool) as LPARAM);
    }
    context.1
}
mod output;
#[cfg(test)]
mod tests;
