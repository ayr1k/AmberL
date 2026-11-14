// SPDX-License-Identifier: MPL-2.0
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ambrum::{content::modrinth as api, task::Cancellation};
    use std::collections::BTreeSet;
    let context = api::Context {
        game: "1.21.1".into(),
        loader: "fabric".into(),
        server: false,
    };
    let c = Cancellation::default();
    let versions = api::versions("jade", "mod", &context, &c)?;
    let version = &versions.first().ok_or("No compatible Jade version")?.id;
    let base = api::plan(version, "mod", &context, &c)?;
    let choice = base.optional.first().ok_or("No optional dependency")?;
    assert!(!choice.selected);
    let choices = BTreeSet::from([choice.key.clone()]);
    let selected = api::plan_with_choices(version, "mod", &context, &choices, &c)?;
    assert!(
        selected
            .optional
            .iter()
            .any(|o| o.key == choice.key && o.selected)
    );
    assert!(selected.files.len() > base.files.len());
    let cleared = api::plan(version, "mod", &context, &c)?;
    assert_eq!(cleared.files.len(), base.files.len());
    println!(
        "PASS optional dependency {}: {} -> {} -> {} files; no game data changed",
        choice.title,
        base.files.len(),
        selected.files.len(),
        cleared.files.len()
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {}
