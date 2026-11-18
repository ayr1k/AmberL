// SPDX-License-Identifier: MPL-2.0
//! Only pass an explicitly disposable AmberL-created client; never an external game library.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ambrum::{maintenance, task::Cancellation};
    use std::path::PathBuf;
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if a.len() != 6 || a[5] != "--disposable-owned-game" {
        return Err(
            "instance_backup_smoke APP GAME VERSION PARENT NEW_FOLDER --disposable-owned-game"
                .into(),
        );
    }
    let app = PathBuf::from(&a[0]);
    let root = PathBuf::from(&a[1]);
    let parent = PathBuf::from(&a[3]);
    std::fs::create_dir_all(&app)?;
    std::fs::create_dir_all(&parent)?;
    let options = root.join("options.txt");
    let before = std::fs::read(&options)?;
    let c = Cancellation::default();
    let id = maintenance::backup(&app, &root, &a[2], &c)?;
    let restored = maintenance::restore(&app, &id, &parent, &a[4], &c)?;
    if std::fs::read(&options)? != before || std::fs::read(restored.join("options.txt"))? != before
    {
        return Err("Game settings changed during backup/restore".into());
    }
    maintenance::save_profile(&app, &restored, &a[2], "恢复验证世界", "anvil", true, &c)?;
    if maintenance::ready(&app, &restored).is_ok() {
        return Err("Archived game remained launchable".into());
    }
    maintenance::save_profile(&app, &restored, &a[2], "恢复验证世界", "anvil", false, &c)?;
    maintenance::ready(&app, &restored)?;
    println!(
        "PASS complete owned-instance snapshot/restore, original settings preserved, rename/icon/archive/return; snapshot={id}; restored={}",
        restored.display()
    );
    println!("Real authenticated client launch must be verified separately.");
    Ok(())
}
#[cfg(not(windows))]
fn main() {}
