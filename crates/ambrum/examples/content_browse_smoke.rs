// SPDX-License-Identifier: MPL-2.0
//! Read-only real provider check; broad discovery must not authorize incompatible installation.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ambrum::{
        content::modrinth::{self, Browse, Context},
        task::Cancellation,
    };
    let c = Cancellation::default();
    let context = Context {
        game: "26.3".into(),
        loader: "paper".into(),
        server: true,
    };
    let strict = modrinth::versions("Vebnzrzj", "plugin", &context, &c)?;
    let broad = modrinth::versions_with_filters(
        "Vebnzrzj",
        "plugin",
        &context,
        Browse {
            game: false,
            loader: false,
        },
        &c,
    )?;
    if broad.len() <= strict.len() {
        return Err("Broader filter did not expose more versions".into());
    }
    let incompatible = broad
        .iter()
        .find(|v| v.title.contains("不匹配当前实例"))
        .ok_or("Missing mismatch label")?;
    if modrinth::plan(&incompatible.id, "plugin", &context, &c).is_ok() {
        return Err("Incompatible plan was accepted".into());
    }
    println!(
        "PASS real LuckPerms version filter override: {} -> {} versions; incompatible plan rejected; no instance writes",
        strict.len(),
        broad.len()
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {}
