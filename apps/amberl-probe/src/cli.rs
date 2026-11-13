// SPDX-License-Identifier: MPL-2.0
use ambrum::{library::Library, task::Cancellation};
use std::path::Path;
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let result = match args.as_slice() {
        [command, path] if command == "scan" => Library::open(Path::new(path)).and_then(|l| l.scan(&Cancellation::default())).and_then(|s| serde_json::to_string_pretty(&s).map_err(|e| e.to_string())),
        [command, path] if command == "java" => ambrum::java::inspect(Path::new(path), &Cancellation::default()).and_then(|j| serde_json::to_string_pretty(&j).map_err(|e| e.to_string())),
        #[cfg(windows)]
        [command, directory, rest @ ..] if command == "libraries" => registered(Path::new(directory), rest),
        #[cfg(windows)]
        [command, directory, url, digest, size] if command == "fetch" => fetch(Path::new(directory), url.to_str(), digest.to_str(), size.to_str()),
        #[cfg(windows)]
        [command, metadata, os_version] if command == "plan-vanilla" => plan(Path::new(metadata), os_version.to_str()),
        #[cfg(windows)]
        [command, index] if command == "plan-assets" => plan_assets(Path::new(index)),
        #[cfg(windows)]
        [command, directory, metadata, os_version, max_bytes] if command == "prepare-vanilla" => prepare(Path::new(directory), Path::new(metadata), os_version.to_str(), max_bytes.to_str()),
        _ => Err("Usage: amberl-inspect scan <library> | java <java.exe> | libraries <app-directory> [add <library> | scan <id> | remove <id>] | fetch <app-directory> <https-url> <sha1:hex|sha256:hex> <bytes> | plan-vanilla <metadata.json> <windows-os-version> | plan-assets <asset-index.json> | prepare-vanilla <app-directory> <metadata.json> <windows-os-version> <max-bytes>\nDeveloper probe. Writes only explicit app configuration/cache, never game files. Does not install or launch Minecraft.".into()),
    };
    match result {
        Ok(output) => println!("{output}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

#[cfg(windows)]
fn plan_assets(path: &Path) -> ambrum::Result<String> {
    let path = ambrum::paths::ordinary(path)?;
    let index = ambrum::paths::json(path.parent().ok_or("Missing index parent")?, &path)?;
    let plan = ambrum::manifest::AssetPlan::parse(&index)?;
    serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn plan(path: &Path, os_version: Option<&str>) -> ambrum::Result<String> {
    let path = ambrum::paths::ordinary(path)?;
    let root = path.parent().ok_or("Missing metadata parent")?;
    let metadata = ambrum::paths::json(root, &path)?;
    let plan = ambrum::manifest::VersionPlan::parse(
        &metadata,
        &ambrum::manifest::Platform::windows_x64(os_version.ok_or("Invalid Windows version")?),
    )?;
    serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn cancellable<T>(run: impl FnOnce(&Cancellation) -> ambrum::Result<T>) -> ambrum::Result<T> {
    use windows_sys::Win32::System::Console::*;
    static CANCEL: std::sync::OnceLock<Cancellation> = std::sync::OnceLock::new();
    unsafe extern "system" fn interrupt(event: u32) -> i32 {
        if matches!(event, CTRL_C_EVENT | CTRL_BREAK_EVENT) {
            if let Some(token) = CANCEL.get() {
                token.cancel();
            }
            1
        } else {
            0
        }
    }
    let token = CANCEL.get_or_init(Cancellation::default);
    if unsafe { SetConsoleCtrlHandler(Some(interrupt), 1) } == 0 {
        return Err("Cannot register cancellation handler".into());
    }
    let result = run(token);
    unsafe {
        SetConsoleCtrlHandler(Some(interrupt), 0);
    }
    result
}

#[cfg(windows)]
fn prepare(
    directory: &Path,
    metadata: &Path,
    os_version: Option<&str>,
    max_bytes: Option<&str>,
) -> ambrum::Result<String> {
    let path = ambrum::paths::ordinary(metadata)?;
    let value = ambrum::paths::json(path.parent().ok_or("Missing metadata parent")?, &path)?;
    let platform =
        ambrum::manifest::Platform::windows_x64(os_version.ok_or("Invalid Windows version")?);
    let budget = max_bytes
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|n| *n > 0)
        .ok_or("Explicit positive byte budget required")?;
    let prepared = cancellable(|cancel| {
        ambrum::prepare::vanilla(
            &ambrum::download::Cache::open(directory)?,
            &value,
            &platform,
            budget,
            &ambrum::download::Https,
            cancel,
            |p| {
                // Discovery has no known total. Completion only means verified cache resources.
                eprintln!(
                    "{:?}: {} / {:?} cache resources",
                    p.phase, p.verified_files, p.total_files
                );
            },
        )
    })?;
    serde_json::to_string_pretty(&prepared).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn fetch(
    directory: &Path,
    url: Option<&str>,
    checksum: Option<&str>,
    size: Option<&str>,
) -> ambrum::Result<String> {
    use ambrum::download::{Algorithm, Artifact, Cache, Https};
    let (algorithm, digest) = checksum
        .and_then(|s| s.split_once(':'))
        .ok_or("Expected sha1:hex or sha256:hex")?;
    let artifact = Artifact {
        url: url.ok_or("Invalid URL encoding")?.into(),
        algorithm: match algorithm {
            "sha1" => Algorithm::Sha1,
            "sha256" => Algorithm::Sha256,
            _ => return Err("Unsupported digest algorithm".into()),
        },
        digest: digest.into(),
        size: size
            .and_then(|s| s.parse().ok())
            .ok_or("Invalid byte count")?,
    };
    let result = cancellable(|token| {
        Cache::open(directory).and_then(|cache| {
            cache.fetch(&artifact, &Https, token, |p| {
                // No URL/headers/tokens in progress or diagnostics.
                eprintln!("{:?}: {}/{}", p.phase, p.received, p.total);
            })
        })
    });
    serde_json::to_string_pretty(&result?).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn registered(directory: &Path, args: &[std::ffi::OsString]) -> ambrum::Result<String> {
    use ambrum::registry::Registry;
    let mut registry = Registry::open(directory)?;
    let value = match args {
        [] => serde_json::to_value(registry.entries()),
        [action, path] if action == "add" => serde_json::to_value(registry.attach(Path::new(path))?),
        [action, id] if action == "scan" => serde_json::to_value(registry.scan(id.to_str().ok_or("Invalid library ID")?, &Cancellation::default())?),
        [action, id] if action == "remove" => {
            registry.detach(id.to_str().ok_or("Invalid library ID")?)?;
            serde_json::to_value(registry.entries())
        },
        _ => return Err("Usage: amberl-inspect libraries <existing-app-directory> [add <library> | scan <id> | remove <id>]".into()),
    }.map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}
