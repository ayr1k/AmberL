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
        _ => Err("Usage: amberl-inspect scan <library> | java <java.exe>\nRead-only probe. Does not install or launch Minecraft.".into()),
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
