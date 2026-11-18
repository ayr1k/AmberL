// SPDX-License-Identifier: MPL-2.0
//! Source metadata supplies identities only, never executable paths or arguments.
use crate::{Result, loader::Choice};
use serde_json::Value;
use std::collections::BTreeSet;

fn identity(value: &str) -> Result<String> {
    if value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b))
    {
        return Err("游戏或加载器版本标识无效".into());
    }
    crate::paths::name(value)?;
    Ok(value.to_owned())
}

pub(super) fn resolve(chain: &[Value]) -> Result<(String, String, Option<Choice>)> {
    let mut games = BTreeSet::new();
    let mut loaders = BTreeSet::new();
    let mut secondary_neoforge = false;
    for meta in chain {
        if let Some(value) = meta.get("clientVersion") {
            games.insert(identity(value.as_str().ok_or("无效的 clientVersion")?)?);
        }
        if let Some(patches) = meta.get("patches") {
            for patch in patches.as_array().ok_or("无效的版本补丁列表")? {
                if patch["id"] == "game" {
                    games.insert(identity(
                        patch["version"].as_str().ok_or("缺少游戏补丁版本")?,
                    )?);
                }
            }
        }
        if let Some(args) = meta.pointer("/arguments/game").and_then(Value::as_array) {
            for pair in args.windows(2) {
                if pair[0] == "--fml.mcVersion" {
                    games.insert(identity(pair[1].as_str().ok_or("无效的 Forge 游戏版本")?)?);
                }
                for (flag, kind) in [
                    ("--fml.forgeVersion", "Forge"),
                    ("--fml.neoForgeVersion", "NeoForge"),
                ] {
                    if pair[0] == flag {
                        loaders.insert((
                            kind.to_owned(),
                            identity(pair[1].as_str().ok_or("缺少加载器版本参数")?)?,
                        ));
                    }
                }
            }
        }
        if let Some(libs) = meta.get("libraries") {
            for lib in libs.as_array().ok_or("无效的依赖列表")? {
                let name = lib["name"].as_str().ok_or("无效的依赖坐标")?;
                if let Some(game) = name.strip_prefix("net.fabricmc:intermediary:") {
                    games.insert(identity(game)?);
                }
                secondary_neoforge |= name.starts_with("net.neoforged.fancymodloader:loader:");
                for (prefix, kind) in [
                    ("net.fabricmc:fabric-loader:", "Fabric"),
                    ("org.quiltmc:quilt-loader:", "Quilt"),
                    ("net.minecraftforge:forge:", "Forge"),
                    ("net.neoforged:neoforge:", "NeoForge"),
                ] {
                    if let Some(raw) = name.strip_prefix(prefix) {
                        let raw = raw.strip_suffix(":universal").unwrap_or(raw);
                        let version = if kind == "Forge" {
                            let (game, version) =
                                raw.split_once('-').ok_or("无法确认 Forge 游戏版本")?;
                            games.insert(identity(game)?);
                            version
                        } else {
                            raw
                        };
                        loaders.insert((kind.to_owned(), identity(version)?));
                    }
                }
            }
        }
    }
    if chain.len() > 1 {
        games.insert(identity(
            chain.last().unwrap()["id"].as_str().ok_or("缺少基础版本")?,
        )?);
    }
    if games.len() > 1 || loaders.len() > 1 {
        return Err("游戏或加载器版本证据冲突，请在原启动器核对".into());
    }
    let base = chain.last().ok_or("缺少版本元数据")?;
    let game = games
        .into_iter()
        .next()
        .unwrap_or_else(|| base["id"].as_str().unwrap_or("?").to_owned());
    let main = chain.iter().find_map(|m| m["mainClass"].as_str());
    if let Some((kind, version)) = loaders.into_iter().next() {
        let version = if kind == "Forge" {
            format!("{game}-{version}")
        } else {
            version
        };
        Ok((game, kind.clone(), Some(Choice { kind, version })))
    } else {
        let kind = if secondary_neoforge {
            "NeoForge"
        } else if main == Some("net.minecraft.client.main.Main") {
            "Vanilla"
        } else {
            "Unknown"
        };
        Ok((game, kind.into(), None))
    }
}
