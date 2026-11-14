// SPDX-License-Identifier: MPL-2.0
//! Official loader profiles, resolved before the existing new-directory transaction.
use crate::{
    Result,
    catalog::bounded,
    download::{Algorithm, Artifact, Cache, Transport, windows::Hash},
    paths,
    task::Cancellation,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    io::{Cursor, Read},
    path::Path,
};
mod forge;
pub mod server;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Choice {
    pub kind: String,
    pub version: String,
}
fn endpoint(kind: &str) -> Result<&'static str> {
    match kind {
        "Fabric" => Ok("https://meta.fabricmc.net/v2"),
        "Quilt" => Ok("https://meta.quiltmc.org/v3"),
        _ => Err("Unknown loader".into()),
    }
}
pub fn versions(
    game: &str,
    kind: &str,
    net: &impl Transport,
    cancel: &Cancellation,
) -> Result<Vec<String>> {
    segment(game)?;
    if matches!(kind, "Forge" | "NeoForge") {
        return forge::versions(game, kind, net, cancel);
    }
    let bytes = bounded(
        net,
        &format!("{}/versions/loader/{game}", endpoint(kind)?),
        cancel,
    )?;
    let data: Value = serde_json::from_slice(&bytes).map_err(|_| "Invalid loader catalog")?;
    let entries = data.as_array().ok_or("Missing loader versions")?;
    if entries.len() > 4096 {
        return Err("Loader version limit".into());
    }
    let mut seen = BTreeSet::new();
    let mut versions = entries
        .iter()
        .map(|v| {
            let id = v
                .pointer("/loader/version")
                .and_then(Value::as_str)
                .ok_or("Missing loader version")?;
            segment(id)?;
            if !seen.insert(id) {
                return Err("Duplicate loader version".into());
            }
            Ok(id.to_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    versions.sort_by(|a, b| version_order(b, a));
    Ok(versions)
}
fn version_order(a: &str, b: &str) -> std::cmp::Ordering {
    let key = |s: &str| {
        s.split(['-', '+'])
            .next()
            .unwrap_or(s)
            .split('.')
            .map(|n| n.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    key(a)
        .cmp(&key(b))
        .then_with(|| (!a.contains('-')).cmp(&!b.contains('-')))
        .then_with(|| a.cmp(b))
}
fn segment(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 128
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_+".contains(&b))
    {
        return Err("Invalid provider identity".into());
    }
    paths::name(s)
}
pub fn maven_path(name: &str) -> Result<String> {
    let parts: Vec<_> = name.split(':').collect();
    if !(3..=4).contains(&parts.len()) {
        return Err("Unsupported Maven coordinate".into());
    }
    for part in &parts {
        segment(part)?;
    }
    let path = format!(
        "{}/{}/{}/{}-{}{}.jar",
        parts[0].replace('.', "/"),
        parts[1],
        parts[2],
        parts[1],
        parts[2],
        if parts.len() == 4 {
            format!("-{}", parts[3])
        } else {
            String::new()
        }
    );
    crate::manifest::relative_path(&path)?;
    Ok(path)
}
fn origin(url: &str) -> Result<()> {
    if ![
        "https://maven.fabricmc.net/",
        "https://maven.quiltmc.org/",
        "https://libraries.minecraft.net/",
        "https://maven.minecraftforge.net/",
        "https://maven.neoforged.net/",
        "https://repo.maven.apache.org/maven2/",
    ]
    .iter()
    .any(|p| url.starts_with(p))
    {
        return Err("Unrecognized loader dependency origin".into());
    }
    crate::download::Https::validate(url)
}
struct Bytes(Vec<u8>);
impl Transport for Bytes {
    fn open(&self, _: &str) -> Result<Box<dyn Read>> {
        Ok(Box::new(Cursor::new(self.0.clone())))
    }
}
// Maven checksum endpoints omit size. Bound the input, verify provider SHA-1, then
// import through the same hash-addressed cache; never accept a self-computed network hash.
fn fetch(
    cache: &Cache,
    url: &str,
    digest: Option<&str>,
    net: &impl Transport,
    cancel: &Cancellation,
    max_bytes: u64,
) -> Result<Artifact> {
    origin(url)?;
    let digest = match digest {
        Some(s) if !s.is_empty() => s.to_owned(),
        _ => String::from_utf8(bounded(net, &format!("{url}.sha1"), cancel)?)
            .map_err(|_| "Invalid Maven checksum")?
            .split_whitespace()
            .next()
            .ok_or("Empty Maven checksum")?
            .to_owned(),
    };
    let mut a = Artifact {
        url: url.into(),
        size: 0,
        algorithm: Algorithm::Sha1,
        digest,
    };
    a.validate()?;
    let mut input = net.open(url)?;
    let mut bytes = Vec::new();
    let start = std::time::Instant::now();
    let mut buf = [0; 65536];
    loop {
        cancel.check()?;
        if start.elapsed().as_secs() > 180 {
            return Err("Loader download timed out".into());
        }
        let n = input
            .read(&mut buf)
            .map_err(|_| "Loader download interrupted")?;
        if n == 0 {
            break;
        }
        if (bytes.len() + n) as u64 > max_bytes.min(256 * 1024 * 1024) {
            return Err("Loader resource exceeds remaining budget or 256 MiB".into());
        }
        bytes.extend_from_slice(&buf[..n]);
    }
    a.size = bytes.len() as u64;
    cache.fetch(&a, &Bytes(bytes), cancel, |_| {})?;
    Ok(a)
}
fn descriptor(a: &Artifact, path: &str) -> Value {
    json!({"path":path,"url":a.url,"size":a.size,"sha1":a.digest})
}
fn normalize(
    cache: &Cache,
    profile: &mut Value,
    mut remaining: u64,
    net: &impl Transport,
    cancel: &Cancellation,
) -> Result<()> {
    let libs = profile["libraries"]
        .as_array_mut()
        .ok_or("Missing loader libraries")?;
    if libs.len() > 4096 {
        return Err("Loader library limit".into());
    }
    for lib in libs {
        cancel.check()?;
        let path = maven_path(lib["name"].as_str().ok_or("Missing Maven coordinate")?)?;
        let base = lib["url"]
            .as_str()
            .unwrap_or("https://libraries.minecraft.net/");
        let url = lib
            .pointer("/downloads/artifact/url")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{}/{path}", base.trim_end_matches('/')));
        let sha = lib
            .pointer("/downloads/artifact/sha1")
            .and_then(Value::as_str);
        let a = fetch(cache, &url, sha, net, cancel, remaining)?;
        remaining = remaining
            .checked_sub(a.size)
            .ok_or("Loader budget exceeded")?;
        lib["downloads"] = json!({"artifact":descriptor(&a,&path)});
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub fn resolve(
    cache: &Cache,
    base: &Value,
    choice: &Choice,
    java: &Path,
    platform: &crate::manifest::Platform,
    budget: u64,
    net: &impl Transport,
    cancel: &Cancellation,
) -> Result<Value> {
    let game = base["id"].as_str().ok_or("Missing game identity")?;
    if !versions(game, &choice.kind, net, cancel)?.contains(&choice.version) {
        return Err("所选加载器版本不适配当前游戏，请刷新列表".into());
    }
    let profile = if matches!(choice.kind.as_str(), "Forge" | "NeoForge") {
        forge::profile(cache, base, choice, java, platform, budget, net, cancel)?
    } else {
        let bytes = bounded(
            net,
            &format!(
                "{}/versions/loader/{game}/{}/profile/json",
                endpoint(&choice.kind)?,
                choice.version
            ),
            cancel,
        )?;
        let mut profile: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid loader profile")?;
        normalize(cache, &mut profile, budget, net, cancel)?;
        profile
    };
    let mut merged = merge(base, &profile, &choice.kind)?;
    merged["amberlLoader"] = json!({"choice":choice,"profile":profile});
    Ok(merged)
}
/// Pure merge, also replayed at launch against newly verified official base metadata.
pub fn merge(base: &Value, profile: &Value, kind: &str) -> Result<Value> {
    if profile["inheritsFrom"] != base["id"] {
        return Err("Loader/game identity mismatch".into());
    }
    let main = profile["mainClass"]
        .as_str()
        .ok_or("Missing loader entrypoint")?;
    let valid = match kind {
        "Fabric" => {
            main == "net.fabricmc.loader.impl.launch.knot.KnotClient"
                || main == "net.fabricmc.loader.launch.knot.KnotClient"
        }
        "Quilt" => main == "org.quiltmc.loader.impl.launch.knot.KnotClient",
        "Forge" | "NeoForge" => matches!(
            main,
            "net.minecraftforge.bootstrap.ForgeBootstrap"
                | "cpw.mods.bootstraplauncher.BootstrapLauncher"
                | "cpw.mods.modlauncher.Launcher"
                | "net.neoforged.fml.startup.Client"
                | "net.neoforged.bootstraploader.Bootstrap"
        ),
        _ => false,
    };
    if !valid || profile.get("minecraftArguments").is_some() {
        return Err("This loader entrypoint/legacy profile is not supported yet".into());
    }
    let libs = profile["libraries"]
        .as_array()
        .ok_or("Missing loader libraries")?;
    let key = |v: &Value| -> Result<String> {
        Ok(v["name"]
            .as_str()
            .ok_or("Missing library name")?
            .split(':')
            .take(2)
            .collect::<Vec<_>>()
            .join(":"))
    };
    let replaced = libs.iter().map(key).collect::<Result<BTreeSet<_>>>()?;
    let mut combined = Vec::new();
    for lib in base["libraries"]
        .as_array()
        .ok_or("Missing base libraries")?
    {
        if !replaced.contains(&key(lib)?) {
            combined.push(lib.clone());
        }
    }
    combined.extend(libs.clone());
    let mut result = base.clone();
    result["libraries"] = Value::Array(combined);
    result["mainClass"] = main.into();
    for group in ["game", "jvm"] {
        let mut args = base
            .pointer(&format!("/arguments/{group}"))
            .and_then(Value::as_array)
            .ok_or("Loader installation requires modern game arguments")?
            .clone();
        if let Some(extra) = profile.pointer(&format!("/arguments/{group}")) {
            args.extend(extra.as_array().ok_or("Invalid loader arguments")?.clone());
        }
        result["arguments"][group] = Value::Array(args);
    }
    if let Some(extra) = profile.get("amberlExtra") {
        result["amberlExtra"] = extra.clone();
    }
    Ok(result)
}
pub fn display(metadata: &mut Value, name: &str, icon: &str) -> Result<()> {
    if name.trim().is_empty()
        || name.chars().count() > 64
        || name.chars().any(char::is_control)
        || !["grass", "dirt", "anvil", "pickaxe"].contains(&icon)
    {
        return Err("名称需为 1–64 个字符；请选择内置图标".into());
    }
    metadata["amberlDisplay"] = json!({"name":name.trim(),"icon":icon});
    Ok(())
}
pub fn verify_metadata(base: &Value, installed: &Value) -> Result<()> {
    let mut expected = if let Some(loader) = installed.get("amberlLoader") {
        let choice: Choice = serde_json::from_value(loader["choice"].clone())
            .map_err(|_| "Invalid installed loader choice")?;
        segment(&choice.version)?;
        let mut merged = merge(base, &loader["profile"], &choice.kind)?;
        merged["amberlLoader"] = loader.clone();
        merged
    } else {
        base.clone()
    };
    if let Some(d) = installed.get("amberlDisplay") {
        display(
            &mut expected,
            d["name"].as_str().ok_or("Invalid display name")?,
            d["icon"].as_str().ok_or("Invalid icon")?,
        )?;
    }
    if expected != *installed {
        return Err("Installed metadata differs from verified game and loader plan".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_errors_cancellation_and_checksums_do_not_commit() {
        struct Net(Vec<u8>);
        impl Transport for Net {
            fn open(&self, _: &str) -> Result<Box<dyn Read>> {
                Ok(Box::new(Cursor::new(self.0.clone())))
            }
        }
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(versions("26.3", "Fabric", &Net(b"[]".to_vec()), &cancel).is_err());
        assert!(
            versions(
                "26.3",
                "Quilt",
                &Net(br#"[{"loader":{"version":"a"}},{"loader":{"version":"a"}}]"#.to_vec()),
                &Cancellation::default()
            )
            .is_err()
        );
        assert!(origin("https://maven.fabricmc.net.evil.example/a").is_err());
        let dir = std::env::temp_dir().join(format!(
            "amberl-loader-{}",
            crate::registry::new_id().unwrap()
        ));
        std::fs::create_dir(&dir).unwrap();
        {
            let cache = Cache::open(&dir).unwrap();
            assert!(
                fetch(
                    &cache,
                    "https://maven.fabricmc.net/test.jar",
                    Some(&"0".repeat(40)),
                    &Net(b"wrong".to_vec()),
                    &Cancellation::default(),
                    1024
                )
                .is_err()
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
        assert!(version_order("0.31.0-beta.1", "0.29.2").is_gt());
        assert!(version_order("0.31.0", "0.31.0-beta.1").is_gt());
    }
    #[test]
    fn coordinates_and_display_are_bounded() {
        assert_eq!(
            maven_path("net.fabricmc:fabric-loader:0.16.14").unwrap(),
            "net/fabricmc/fabric-loader/0.16.14/fabric-loader-0.16.14.jar"
        );
        for bad in ["a:b:../../x", "a:b:v:c:extra", "a:b:v/evil", "a:b:v?x"] {
            assert!(maven_path(bad).is_err());
        }
        let mut m = json!({});
        assert!(display(&mut m, "旅途", "anvil").is_ok());
        assert!(display(&mut m, "\n", "grass").is_err());
        assert!(display(&mut m, "a", "../icon").is_err());
    }
    #[test]
    fn merge_replaces_libraries_and_rejects_identity_or_payload_changes() {
        let base = json!({"id":"1.21.1","mainClass":"net.minecraft.client.main.Main","libraries":[{"name":"org.test:asm:1"}],"arguments":{"game":["--demo"],"jvm":["-cp","${classpath}"]}});
        let p = json!({"inheritsFrom":"1.21.1","mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient","libraries":[{"name":"org.test:asm:2"}],"arguments":{"game":[],"jvm":["-Dtest=true"]}});
        let mut m = merge(&base, &p, "Fabric").unwrap();
        assert_eq!(m["libraries"].as_array().unwrap().len(), 1);
        m["amberlLoader"] = json!({"choice":{"kind":"Fabric","version":"0.16.14"},"profile":p});
        display(&mut m, "冒险", "grass").unwrap();
        verify_metadata(&base, &m).unwrap();
        m["downloads"] = json!({"client":{"url":"https://evil.example/x"}});
        assert!(verify_metadata(&base, &m).is_err());
        let mut bad = p.clone();
        bad["inheritsFrom"] = "other".into();
        assert!(merge(&base, &bad, "Fabric").is_err());
        assert!(merge(&base, &p, "Forge").is_err());
    }
}
