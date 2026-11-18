// SPDX-License-Identifier: MPL-2.0
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ambrum::{java, task::Cancellation};
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("java_runtime_smoke APP MAJOR".into());
    }
    let app = std::path::PathBuf::from(&args[0]);
    std::fs::create_dir_all(&app)?;
    let major = args[1].parse()?;
    let cancel = Cancellation::default();
    let path = java::runtime::install(&app, major, &cancel, |done, total| {
        if done % 25 == 0 || done == total {
            println!("{done}/{total}");
        }
    })?;
    let runtime = java::inspect(&path, &cancel)?;
    java::compatible(&runtime, Some(major))?;
    let found = java::runtime::discover(&app, &cancel)?
        .into_iter()
        .any(|c| c.executable == path && c.java.is_some());
    if !found {
        return Err("Installed runtime not rediscovered".into());
    }
    println!(
        "PASS Java {} {}: {}",
        runtime.version,
        runtime.architecture,
        path.display()
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {}
