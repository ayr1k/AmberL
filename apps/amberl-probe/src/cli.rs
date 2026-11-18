// SPDX-License-Identifier: MPL-2.0
use ambrum::{library::Library, task::Cancellation};
use std::path::Path;
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let result = match args.as_slice() {
        [command, path] if command == "scan" => Library::open(Path::new(path)).and_then(|l| l.scan(&Cancellation::default())).and_then(|s| serde_json::to_string_pretty(&s).map_err(|e| e.to_string())),
        [command, path] if command == "java" => ambrum::java::inspect(Path::new(path), &Cancellation::default()).and_then(|j| serde_json::to_string_pretty(&j).map_err(|e| e.to_string())),
        [command] if command == "java-list" => ambrum::java::discover(&Cancellation::default()).and_then(|j| serde_json::to_string_pretty(&j).map_err(|e| e.to_string())),
        #[cfg(windows)]
        [command, app, game, version, java, account, memory, os] if command == "launch-vanilla" || command == "check-launch" => cancellable(|cancel| {
            let selection = ambrum::launch::Selection {
                app_directory: Path::new(app), game_directory: Path::new(game),
                version: version.to_str().ok_or("Invalid version")?, java_executable: Path::new(java),
                account_id: account.to_str().ok_or("Invalid account ID")?,
                max_memory_mib: memory.to_str().and_then(|s| s.parse().ok()).ok_or("Explicit memory in MiB required")?,
                os_version: os.to_str().ok_or("Invalid Windows version")?,
            };
            if command == "check-launch" {
                serde_json::to_string_pretty(&ambrum::launch::preflight(&selection, cancel)?).map_err(|e| e.to_string())
            } else {
                let result = ambrum::launch::run(&selection, cancel, |event| {
                    if let Ok(text) = serde_json::to_string(&event) { eprintln!("{text}"); }
                })?;
                let output = serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?;
                if result.success { Ok(output) } else { Err(output) }
            }
        }),
        #[cfg(windows)]
        [command, directory, rest @ ..] if command == "accounts" => accounts(Path::new(directory), rest),
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
        #[cfg(windows)]
        [command, directory, version] if command == "resolve-vanilla" => resolve(Path::new(directory), version.to_str()),
        #[cfg(windows)]
        [command, directory, metadata, os_version, parent, name, max_bytes] if command == "install-vanilla" => install(Path::new(directory), Path::new(metadata), os_version.to_str(), Path::new(parent), name.to_str(), max_bytes.to_str()),
        #[cfg(windows)]
        [command, parent, operation] if command == "recover-install" => cancellable(|cancel| {
            let path = ambrum::install::recover(Path::new(parent), operation.to_str().ok_or("Invalid operation ID")?, cancel)?;
            serde_json::to_string_pretty(&serde_json::json!({"game_directory":path,"launch_verified":false})).map_err(|e| e.to_string())
        }),
        _ => Err("Usage: amberl-inspect java-list | accounts <app-directory> [login | verify <id> | remove <id>] | check-launch/launch-vanilla <app-directory> <game-directory> <version> <java.exe> <account-id> <memory-MiB> <windows-os-version> | scan <library> | java <java.exe> | libraries <app-directory> [add <library> | scan <id> | remove <id>] | fetch <app-directory> <https-url> <sha1:hex|sha256:hex> <bytes> | plan-vanilla <metadata.json> <windows-os-version> | plan-assets <asset-index.json> | prepare-vanilla <app-directory> <metadata.json> <windows-os-version> <max-bytes> | resolve-vanilla <app-directory> <version> | install-vanilla <app-directory> <metadata.json> <windows-os-version> <install-parent> <new-name> <max-bytes> | recover-install <install-parent> <operation-id>\nDeveloper probe. Installation creates new isolated directories only. Authenticated launch supports complete AmberL-created Vanilla installations with an explicit matching x64 Java. Native probe UI remains read-only.".into()),
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
fn accounts(directory: &Path, args: &[std::ffi::OsString]) -> ambrum::Result<String> {
    use ambrum::{
        auth::{DeviceFlow, Poll, http::Https},
        secure::Store,
    };
    use std::io::Write;
    match args {
        [] => serde_json::to_string_pretty(&Store::open(directory)?.profiles())
            .map_err(|e| e.to_string()),
        [action] if action == "login" => cancellable(|cancel| {
            // Check secure storage accessibility before requesting account consent.
            drop(Store::open(directory)?);
            ambrum::secure::check_available()?;
            let mut flow = DeviceFlow::begin(&Https, cancel)?;
            println!(
                "{}",
                serde_json::to_string(flow.prompt()).map_err(|_| "Cannot present device code")?
            );
            std::io::stdout()
                .flush()
                .map_err(|_| "Cannot present device code")?;
            loop {
                match flow.poll(&Https, cancel)? {
                    Poll::Waiting => std::thread::sleep(std::time::Duration::from_millis(200)),
                    Poll::Authorized(auth) => {
                        return serde_json::to_string_pretty(
                            &Store::open(directory)?.save_authorization(auth)?,
                        )
                        .map_err(|e| e.to_string());
                    }
                }
            }
        }),
        [action, id] if action == "verify" => cancellable(|cancel| {
            let identity = Store::open(directory)?.authenticate(
                id.to_str().ok_or("Invalid account ID")?,
                &Https,
                cancel,
            )?;
            serde_json::to_string_pretty(&identity.profile).map_err(|e| e.to_string())
        }),
        [action, id] if action == "remove" => {
            let mut store = Store::open(directory)?;
            store.forget(id.to_str().ok_or("Invalid account ID")?)?;
            serde_json::to_string_pretty(&store.profiles()).map_err(|e| e.to_string())
        }
        _ => Err(
            "Usage: accounts <app-directory> [login | verify <account-id> | remove <account-id>]"
                .into(),
        ),
    }
}

#[cfg(windows)]
fn resolve(directory: &Path, version: Option<&str>) -> ambrum::Result<String> {
    let result = cancellable(|cancel| {
        ambrum::catalog::resolve(
            &ambrum::download::Cache::open(directory)?,
            version.ok_or("Invalid version ID")?,
            &ambrum::download::Https,
            cancel,
        )
    })?;
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn install(
    directory: &Path,
    metadata: &Path,
    os_version: Option<&str>,
    parent: &Path,
    name: Option<&str>,
    max_bytes: Option<&str>,
) -> ambrum::Result<String> {
    let path = ambrum::paths::ordinary(metadata)?;
    let value = ambrum::paths::json(path.parent().ok_or("Missing metadata parent")?, &path)?;
    let budget = max_bytes
        .and_then(|n| n.parse::<u64>().ok())
        .filter(|n| *n > 0)
        .ok_or("Positive byte budget required")?;
    let result = cancellable(|cancel| {
        ambrum::install::vanilla(
            &ambrum::download::Cache::open(directory)?,
            &value,
            &ambrum::manifest::Platform::windows_x64(os_version.ok_or("Invalid Windows version")?),
            parent,
            name.ok_or("Invalid instance name")?,
            budget,
            cancel,
            |p| {
                eprintln!(
                    "{} {:?}: {}/{} files",
                    p.operation, p.phase, p.files_written, p.total_files
                );
            },
        )
    })?;
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
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
