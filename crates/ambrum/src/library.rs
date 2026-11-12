// SPDX-License-Identifier: MPL-2.0
use crate::{Result, paths, task::Cancellation};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LibraryId {
    Observed(PathBuf),
    Registered(String),
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClientInstanceId {
    pub library: LibraryId,
    pub profile: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServerInstanceId {
    pub library: LibraryId,
    pub server: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Library {
    pub id: LibraryId,
    pub root: PathBuf,
    pub kind: LibraryKind,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum LibraryKind {
    AmberManaged,
    Minecraft,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Isolation {
    Isolated,
    Shared,
    Unknown,
    Conflict,
}
#[derive(Clone, Debug, Serialize)]
pub struct Instance {
    pub id: ClientInstanceId,
    pub name: String,
    pub game: String,
    pub loader: String,
    pub isolation: Isolation,
    pub game_dir: Option<PathBuf>,
    pub evidence: Vec<String>,
    pub issue: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Scan {
    pub library: Library,
    pub instances: Vec<Instance>,
    pub warnings: Vec<String>,
}

impl Library {
    pub fn open(path: &Path) -> Result<Self> {
        let root = paths::ordinary(path)?;
        if !root.is_dir() {
            return Err("请选择游戏库文件夹 / Select a library directory".into());
        }
        let managed = root
            .join("instances")
            .try_exists()
            .map_err(|e| e.to_string())?;
        let minecraft = root
            .join("versions")
            .try_exists()
            .map_err(|e| e.to_string())?;
        let kind = match (managed, minecraft) {
            (true, false) => LibraryKind::AmberManaged,
            (false, true) => LibraryKind::Minecraft,
            (true, true) => {
                return Err(
                    "同时存在 instances 与 versions，请明确选择库布局 / Ambiguous library layout"
                        .into(),
                );
            }
            _ => {
                return Err(
                    "未找到 instances 或 versions；未修改目录 / No supported library layout".into(),
                );
            }
        };
        paths::contained(
            &root,
            &root.join(if managed { "instances" } else { "versions" }),
        )?;
        Ok(Self {
            id: LibraryId::Observed(root.clone()),
            root,
            kind,
        })
    }

    pub fn scan(&self, cancel: &Cancellation) -> Result<Scan> {
        if paths::ordinary(&self.root)? != self.root {
            return Err("库路径发生变化 / Library path changed".into());
        }
        let container = self.root.join(match self.kind {
            LibraryKind::AmberManaged => "instances",
            LibraryKind::Minecraft => "versions",
        });
        paths::contained(&self.root, &container)?;
        let mut entries = Vec::new();
        for item in fs::read_dir(&container).map_err(|e| e.to_string())? {
            cancel.check()?;
            let item = item.map_err(|e| e.to_string())?;
            if !item.file_type().map_err(|e| e.to_string())?.is_file() {
                entries.push(item);
            }
            if entries.len() > 20_000 {
                return Err("版本数量超过样机扫描上限 / Probe profile limit exceeded".into());
            }
        }
        entries.sort_by_key(|e| e.file_name());
        let mut rows = Vec::new();
        for item in entries {
            cancel.check()?;
            let name = item.file_name().to_string_lossy().into_owned();
            let id = ClientInstanceId {
                library: self.id.clone(),
                profile: name.clone(),
            };
            let result = match self.kind {
                LibraryKind::AmberManaged => self.managed(&name, id.clone()),
                LibraryKind::Minecraft => self.minecraft(&name, id.clone(), cancel),
            };
            rows.push(result.unwrap_or_else(|e| Instance {
                id,
                name,
                game: "?".into(),
                loader: "?".into(),
                isolation: Isolation::Unknown,
                game_dir: None,
                evidence: vec![],
                issue: Some(e),
            }));
        }
        cancel.check()?;
        Ok(Scan {
            library: self.clone(),
            instances: rows,
            warnings: vec![
                "只读扫描；不代表已验证启动、安装或恢复 / Inspection is not launch acceptance"
                    .into(),
            ],
        })
    }

    fn managed(&self, name: &str, id: ClientInstanceId) -> Result<Instance> {
        paths::name(name)?;
        let dir = paths::contained(&self.root, &self.root.join("instances").join(name))?;
        let meta = paths::json(&self.root, &dir.join("instance.json"))?;
        if meta.get("id").and_then(Value::as_str) != Some(name) {
            return Err("实例 ID 与目录不符 / Instance ID mismatch".into());
        }
        Ok(Instance {
            id,
            name: string(&meta, "name", name),
            game: string(&meta, "game", "?"),
            loader: string(&meta, "loader", "vanilla"),
            isolation: Isolation::Isolated,
            game_dir: Some(dir),
            evidence: vec!["AMBER instance.json + instances layout".into()],
            issue: None,
        })
    }

    fn minecraft(
        &self,
        name: &str,
        id: ClientInstanceId,
        cancel: &Cancellation,
    ) -> Result<Instance> {
        paths::name(name)?;
        let dir = paths::contained(&self.root, &self.root.join("versions").join(name))?;
        let mut chain = Vec::new();
        let mut seen = BTreeSet::new();
        let mut next = name.to_owned();
        loop {
            cancel.check()?;
            paths::name(&next)?;
            if !seen.insert(next.clone()) || chain.len() >= 16 {
                return Err("版本继承循环或过深 / Cyclic or deep inheritance".into());
            }
            let meta = paths::json(
                &self.root,
                &self
                    .root
                    .join("versions")
                    .join(&next)
                    .join(format!("{next}.json")),
            )?;
            if meta.get("id").and_then(Value::as_str) != Some(&next) {
                return Err("版本 ID 与目录不符 / Profile ID mismatch".into());
            }
            let parent = meta
                .get("inheritsFrom")
                .map(|v| v.as_str().ok_or("无效的父版本 / Invalid parent"))
                .transpose()?
                .map(str::to_owned);
            chain.push(meta);
            if let Some(parent) = parent {
                next = parent;
            } else {
                break;
            }
        }
        let mut directories: Vec<(PathBuf, String)> = vec![];
        let mut add = |raw: &str, source: &str| -> Result<()> {
            if raw == "${game_directory}" {
                return Ok(());
            }
            if raw.is_empty() || raw.contains("${") || raw.contains('\0') {
                return Err("无法解析游戏目录 / Unresolved game directory".into());
            }
            let path = paths::contained(&self.root, &self.root.join(raw))?;
            if !path.is_dir() {
                return Err("游戏目录不是文件夹 / Game path is not a directory".into());
            }
            directories.push((path, source.into()));
            Ok(())
        };
        for meta in &chain {
            if let Some(p) = meta.get("gameDir") {
                add(p.as_str().ok_or("无效的 gameDir")?, "version gameDir")?;
            }
            if let Some(args) = meta.pointer("/arguments/game") {
                let args = args
                    .as_array()
                    .ok_or("无效的游戏参数 / Invalid arguments")?;
                for (i, arg) in args.iter().enumerate() {
                    if arg.as_str() == Some("--gameDir") {
                        add(
                            args.get(i + 1)
                                .and_then(Value::as_str)
                                .ok_or("缺失游戏目录参数")?,
                            "game arguments",
                        )?;
                    }
                    if let Some(p) = arg.as_str().and_then(|v| v.strip_prefix("--gameDir=")) {
                        add(p, "game arguments")?;
                    }
                    if arg.is_object() && arg.to_string().contains("gameDir") {
                        return Err(
                            "条件式游戏目录需要进一步解析 / Conditional game directory unsupported"
                                .into(),
                        );
                    }
                }
            }
            if meta
                .get("minecraftArguments")
                .and_then(Value::as_str)
                .is_some_and(|s| s.contains("gameDir"))
            {
                return Err("旧式参数含自定义游戏目录，暂不推断 / Legacy game-directory arguments need review".into());
            }
        }
        if let Some(meta) =
            paths::optional_json(&self.root, &self.root.join("launcher_profiles.json"))?
            && let Some(profiles) = meta.get("profiles")
        {
            for profile in profiles.as_object().ok_or("无效的官方启动器配置")?.values() {
                if profile.get("lastVersionId").and_then(Value::as_str) == Some(name)
                    && let Some(p) = profile.get("gameDir")
                {
                    add(p.as_str().ok_or("无效的官方游戏目录")?, "Official launcher")?;
                }
            }
        }
        if let Some(meta) = paths::optional_json(&self.root, &dir.join("hmclversion.cfg"))? {
            // Missing usesGlobal is not proof that per-version settings are authoritative.
            if meta.get("usesGlobal").and_then(Value::as_bool) == Some(false) {
                match meta.get("gameDirType").and_then(Value::as_u64) {
                    Some(0) => add(".", "HMCL per-version")?,
                    Some(1) => add(&format!("versions/{name}"), "HMCL per-version")?,
                    Some(2) => add(
                        meta.get("gameDir")
                            .and_then(Value::as_str)
                            .ok_or("缺失 HMCL 游戏目录")?,
                        "HMCL per-version",
                    )?,
                    _ => return Err("未知的 HMCL 隔离配置 / Unknown HMCL directory mode".into()),
                }
            }
        }
        let pcl_path = dir.join("PCL/Setup.ini");
        match fs::symlink_metadata(&pcl_path) {
            Ok(_) => {
                let ini = paths::text(&self.root, &pcl_path, 1_000_000)?;
                let values = |key: &str| -> Vec<&str> {
                    ini.lines()
                        .filter_map(|l| l.split_once(':'))
                        .filter(|(k, _)| k.trim() == key)
                        .map(|(_, v)| v.trim())
                        .collect()
                };
                let new = values("VersionArgumentIndieV2");
                let old = values("VersionArgumentIndie");
                if new.len() > 1 || old.len() > 1 {
                    return Err("重复的 PCL 隔离配置 / Duplicate PCL isolation fields".into());
                }
                let mode = if let Some(v) = new.first() {
                    match v.to_ascii_lowercase().as_str() {
                        "true" => Some(true),
                        "false" => Some(false),
                        _ => return Err("无效的 PCL 隔离配置".into()),
                    }
                } else {
                    match old.first().copied() {
                        Some("1") => Some(true),
                        Some("2") => Some(false),
                        _ => None,
                    }
                };
                if let Some(isolated) = mode {
                    add(
                        if isolated {
                            dir.to_str().ok_or("非 Unicode 路径")?
                        } else {
                            "."
                        },
                        "PCL per-version",
                    )?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        let distinct: BTreeSet<_> = directories.iter().map(|(p, _)| p.clone()).collect();
        let (isolation, game_dir) = if distinct.len() > 1 {
            (Isolation::Conflict, None)
        } else if let Some(path) = distinct.into_iter().next() {
            (
                if path == self.root {
                    Isolation::Shared
                } else {
                    Isolation::Isolated
                },
                Some(path),
            )
        } else {
            (Isolation::Unknown, None)
        };
        let mut loaders = BTreeSet::new();
        for meta in &chain {
            if let Some(libs) = meta.get("libraries") {
                for lib in libs.as_array().ok_or("无效的依赖列表")? {
                    let coordinate = lib
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or("无效的依赖坐标")?;
                    for (prefix, loader) in [
                        ("net.fabricmc:fabric-loader:", "Fabric"),
                        ("org.quiltmc:quilt-loader:", "Quilt"),
                        ("net.minecraftforge:forge:", "Forge"),
                        ("net.neoforged:neoforge:", "NeoForge"),
                    ] {
                        if coordinate.starts_with(prefix) {
                            loaders.insert(loader);
                        }
                    }
                }
            }
        }
        let main = chain
            .iter()
            .find_map(|m| m.get("mainClass").and_then(Value::as_str));
        let loader = if loaders.is_empty() {
            if main == Some("net.minecraft.client.main.Main") {
                "Vanilla".into()
            } else {
                "Unknown".into()
            }
        } else {
            loaders.into_iter().collect::<Vec<_>>().join(" + ")
        };
        let game = chain
            .iter()
            .find_map(|m| {
                m.get("patches")
                    .and_then(Value::as_array)
                    .and_then(|ps| {
                        ps.iter()
                            .find(|p| p.get("id").and_then(Value::as_str) == Some("game"))
                    })
                    .and_then(|p| p.get("version").and_then(Value::as_str))
            })
            .map(str::to_owned)
            .unwrap_or_else(|| string(chain.last().unwrap(), "id", "?"));
        Ok(Instance {
            id,
            name: name.into(),
            game,
            loader,
            isolation,
            game_dir,
            evidence: directories.into_iter().map(|(_, s)| s).collect(),
            issue: if isolation == Isolation::Conflict {
                Some("游戏目录配置冲突；未选择目标 / Conflicting directory evidence".into())
            } else {
                None
            },
        })
    }
}
fn string(meta: &Value, key: &str, fallback: &str) -> String {
    meta.get(key)
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .into()
}
