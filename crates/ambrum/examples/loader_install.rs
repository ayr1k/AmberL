// SPDX-License-Identifier: MPL-2.0
// Explicit developer smoke runner: no account access; installs only into a new directory.
#[cfg(windows)]
fn main() {
    fn run() -> ambrum::Result<()> {
        use ambrum::{
            catalog,
            download::{Cache, Https},
            loader::{self, Choice},
            manifest::Platform,
            task::Cancellation,
        };
        use std::path::Path;
        let args: Vec<_> = std::env::args().skip(1).collect();
        let cancel = Cancellation::default();
        if let [mode, game, kind] = args.as_slice()
            && mode == "catalog"
        {
            println!(
                "{}",
                serde_json::to_string(&loader::versions(game, kind, &Https, &cancel)?).unwrap()
            );
            return Ok(());
        }
        let [app, game, kind, version, java, parent, name, os] = args.as_slice() else {
            return Err("loader_install <app> <game> <kind> <loader-version> <java> <parent> <new-name> <os-version> | catalog <game> <kind>".into());
        };
        let cache = Cache::open(Path::new(app))?;
        let resolved = catalog::resolve(&cache, game, &Https, &cancel)?;
        let base = ambrum::paths::json(
            resolved.metadata_path.parent().unwrap(),
            &resolved.metadata_path,
        )?;
        let platform = Platform::windows_x64(os);
        let budget = 8 * 1024 * 1024 * 1024;
        let mut metadata = loader::resolve(
            &cache,
            &base,
            &Choice {
                kind: kind.clone(),
                version: version.clone(),
            },
            Path::new(java),
            &platform,
            budget,
            &Https,
            &cancel,
        )?;
        loader::display(&mut metadata, name, "anvil")?;
        loader::verify_metadata(&base, &metadata)?;
        ambrum::prepare::vanilla(&cache, &metadata, &platform, budget, &Https, &cancel, |p| {
            if p.resource.is_none() {
                eprintln!(
                    "verified {}/{}",
                    p.verified_files,
                    p.total_files.unwrap_or(0)
                );
            }
        })?;
        let installed = ambrum::install::vanilla(
            &cache,
            &metadata,
            &platform,
            Path::new(parent),
            name,
            budget,
            &cancel,
            |_| {},
        )?;
        let selection = ambrum::launch::Selection {
            app_directory: Path::new(app),
            game_directory: &installed.game_directory,
            version: game,
            java_executable: Path::new(java),
            account_id: "",
            max_memory_mib: 4096,
            os_version: os,
        };
        let checked = ambrum::launch::preflight(&selection, &cancel)?;
        ambrum::registry::Registry::open(Path::new(app))?.attach(&installed.game_directory)?;
        println!("{}", serde_json::to_string_pretty(&checked).unwrap());
        Ok(())
    }
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
#[cfg(not(windows))]
fn main() {}
