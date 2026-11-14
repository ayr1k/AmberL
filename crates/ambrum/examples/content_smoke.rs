// SPDX-License-Identifier: MPL-2.0
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ambrum::{
        content::{self, modrinth},
        task::Cancellation,
    };
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if a.len() != 5 {
        return Err("content_smoke APP OWNED_DISPOSABLE_GAME VERSION PROJECT KIND".into());
    }
    let app = std::path::PathBuf::from(&a[0]);
    std::fs::create_dir_all(&app)?;
    let target = content::Target::Client {
        root: a[1].as_str().into(),
        version: a[2].clone(),
    };
    let c = Cancellation::default();
    let context = content::context(&app, &target, &c)?;
    let hits = modrinth::search(&a[3], &a[4], &context, &c)?;
    println!("SEARCH results={}", hits.len());
    let versions = modrinth::versions(&a[3], &a[4], &context, &c)?;
    let version = versions.first().ok_or("No compatible versions")?;
    let plan = modrinth::plan(&version.id, &a[4], &context, &c)?;
    for f in &plan.files {
        println!("PLAN {} {}", f.title, f.path);
    }
    let id = content::install(&app, &target, &plan, &c)?;
    let state = |wanted: &str| -> ambrum::Result<()> {
        if !content::list(&app, &target, &c)?
            .iter()
            .any(|g| g.id == id && g.state == wanted)
        {
            return Err("Content state mismatch".into());
        }
        Ok(())
    };
    state("installed")?;
    content::change(&app, &target, &id, "remove", &c)?;
    state("removed")?;
    content::change(&app, &target, &id, "restore", &c)?;
    state("installed")?;
    println!(
        "PASS search/version/dependency-plan/verified-install/recoverable-remove/restore; group={id}"
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {}
