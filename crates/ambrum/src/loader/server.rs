// SPDX-License-Identifier: MPL-2.0
//! Server-only loader profiles. No client entrypoints or provider-supplied JVM flags.
use super::*;
use crate::registry::{DirectoryGuard, info, options};
use std::{fs, io::Write, os::windows::fs::OpenOptionsExt};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
mod installer;
mod paper;
pub fn versions(
    game: &str,
    kind: &str,
    net: &impl Transport,
    c: &Cancellation,
) -> Result<Vec<String>> {
    if kind == "Paper" {
        paper::versions(game, net, c)
    } else {
        super::versions(game, kind, net, c)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub choice: Choice,
    game: String,
    main: String,
    files: Vec<File>,
}
impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("choice", &self.choice)
            .field("game", &self.game)
            .finish_non_exhaustive()
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    path: String,
    artifact: Artifact,
}
fn entry(kind: &str) -> Result<&'static str> {
    match kind {
        "Fabric" => Ok("net.fabricmc.loader.impl.launch.knot.KnotServer"),
        "Quilt" => Ok("org.quiltmc.loader.impl.launch.knot.KnotServer"),
        _ => Err("Unsupported server loader".into()),
    }
}
impl Runtime {
    pub fn validate(&self, game: &str) -> Result<()> {
        segment(&self.choice.version)?;
        let main = if self.choice.kind == "Paper" {
            if self.files.len() != 1 || self.files[0].path != "paper.jar" {
                return Err("Invalid Paper file set".into());
            }
            "paper.jar".into()
        } else if matches!(self.choice.kind.as_str(), "Forge" | "NeoForge") {
            installer::argument_file(&self.choice)?
        } else {
            entry(&self.choice.kind)?.into()
        };
        if self.game != game
            || self.main != main
            || self.files.is_empty()
            || self.files.len() > 4096
        {
            return Err("Server loader identity mismatch".into());
        }
        let mut seen = BTreeSet::new();
        let mut bytes = 0u64;
        for f in &self.files {
            paths::name(&self.game)?;
            crate::manifest::relative_path(&f.path)?;
            if !(f.path.starts_with("libraries/")
                || self.choice.kind == "Paper" && f.path == "paper.jar")
                || f.path.contains([';', '\n', '\r'])
                || !seen.insert(f.path.to_ascii_lowercase())
            {
                return Err("Invalid server loader path".into());
            }
            if self.choice.kind == "Paper" {
                paper::validate(&f.artifact, game, &self.choice.version)?;
            } else {
                origin(&f.artifact.url)?;
            }
            f.artifact.validate()?;
            bytes = bytes
                .checked_add(f.artifact.size)
                .ok_or("Loader size overflow")?;
        }
        if self.main.starts_with("libraries/") && !seen.contains(&self.main.to_ascii_lowercase()) {
            return Err("Missing server arguments receipt".into());
        }
        if bytes > 2 * 1024 * 1024 * 1024 {
            return Err("Server loader size limit".into());
        }
        Ok(())
    }
    /// Keep every executable input open against writes/replacement until server exit.
    pub fn pin(&self, root: &Path, game: &str, cancel: &Cancellation) -> Result<Vec<fs::File>> {
        self.validate(game)?;
        self.files
            .iter()
            .map(|f| {
                let path = paths::ordinary(&root.join(&f.path))?;
                let mut handle = options()
                    .share_mode(FILE_SHARE_READ)
                    .open(path)
                    .map_err(|e| e.to_string())?;
                if info(&handle)?.nNumberOfLinks != 1
                    || !handle.metadata().map_err(|e| e.to_string())?.is_file()
                {
                    return Err("Linked server library rejected".into());
                }
                let mut hash = Hash::new(f.artifact.algorithm)?;
                let mut size = 0;
                let mut buf = [0; 65536];
                loop {
                    cancel.check()?;
                    let n = handle.read(&mut buf).map_err(|e| e.to_string())?;
                    if n == 0 {
                        break;
                    }
                    size += n as u64;
                    if size > f.artifact.size {
                        return Err("Server library size mismatch".into());
                    }
                    hash.update(&buf[..n])?;
                }
                if size != f.artifact.size || hash.finish()? != f.artifact.digest {
                    return Err("模组服务端文件校验失败；未启动".into());
                }
                Ok(handle)
            })
            .collect()
    }
    pub fn arguments(&self) -> Vec<String> {
        if self.choice.kind == "Paper" {
            return vec!["-jar".into(), "paper.jar".into()];
        }
        if matches!(self.choice.kind.as_str(), "Forge" | "NeoForge") {
            return vec![format!("@{}", self.main)];
        }
        let property = if self.choice.kind == "Fabric" {
            "fabric"
        } else {
            "loader"
        };
        vec![
            format!("-D{property}.gameJarPath=server.jar"),
            "-cp".into(),
            self.files
                .iter()
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>()
                .join(";"),
            self.main.clone(),
        ]
    }
}
pub fn install(
    cache: &Cache,
    root: &Path,
    base: &Value,
    choice: &Choice,
    java: Option<&Path>,
    cancel: &Cancellation,
) -> Result<Runtime> {
    let net = crate::download::Https;
    let game = base["id"].as_str().ok_or("Missing server game identity")?;
    if !versions(game, &choice.kind, &net, cancel)?.contains(&choice.version) {
        return Err("请选择当前游戏兼容的服务端加载器版本".into());
    }
    if choice.kind == "Paper" {
        return paper::install(cache, root, game, choice, cancel);
    }
    if matches!(choice.kind.as_str(), "Forge" | "NeoForge") {
        return installer::install(
            cache,
            root,
            base,
            choice,
            java.ok_or("请先选择兼容的 Java")?,
            cancel,
        );
    }
    entry(&choice.kind)?;
    let bytes = bounded(
        &net,
        &format!(
            "{}/versions/loader/{game}/{}/server/json",
            endpoint(&choice.kind)?,
            choice.version
        ),
        cancel,
    )?;
    let mut profile: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid server profile")?;
    if profile["inheritsFrom"] != game || profile["mainClass"] != entry(&choice.kind)? {
        return Err("Provider returned mismatched server profile".into());
    }
    normalize(cache, &mut profile, 512 * 1024 * 1024, &net, cancel)?;
    let mut runtime = Runtime {
        choice: choice.clone(),
        game: game.into(),
        main: entry(&choice.kind)?.into(),
        files: vec![],
    };
    for lib in profile["libraries"]
        .as_array()
        .ok_or("Missing server libraries")?
    {
        let d = &lib["downloads"]["artifact"];
        runtime.files.push(File {
            path: format!(
                "libraries/{}",
                d["path"].as_str().ok_or("Missing library path")?
            ),
            artifact: Artifact {
                url: d["url"].as_str().ok_or("Missing library URL")?.into(),
                size: d["size"].as_u64().ok_or("Missing library size")?,
                algorithm: Algorithm::Sha1,
                digest: d["sha1"].as_str().ok_or("Missing library digest")?.into(),
            },
        });
    }
    runtime.validate(game)?;
    for f in &runtime.files {
        cancel.check()?;
        let target = root.join(&f.path);
        let parent = target.parent().ok_or("Missing library parent")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let _guard = DirectoryGuard::open(parent)?;
        let mut out = options()
            .write(true)
            .create_new(true)
            .open(target)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut cache.verified_file(&f.artifact, cancel)?, &mut out)
            .and_then(|_| out.flush())
            .and_then(|_| out.sync_all())
            .map_err(|e| e.to_string())?;
    }
    Ok(runtime)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_client_entrypoint_and_mismatched_server_identity() {
        let mut r = Runtime {
            choice: Choice {
                kind: "Fabric".into(),
                version: "0.19.5".into(),
            },
            game: "26.3".into(),
            main: entry("Fabric").unwrap().into(),
            files: vec![File {
                path: "libraries/net/fabricmc/loader.jar".into(),
                artifact: Artifact {
                    url: "https://maven.fabricmc.net/loader.jar".into(),
                    size: 1,
                    algorithm: Algorithm::Sha1,
                    digest: "0".repeat(40),
                },
            }],
        };
        r.validate("26.3").unwrap();
        assert!(r.validate("1.21.1").is_err());
        r.main = "net.fabricmc.loader.impl.launch.knot.KnotClient".into();
        assert!(r.validate("26.3").is_err());
        r.main = entry("Fabric").unwrap().into();
        r.files[0].path = "libraries/../../escape.jar".into();
        assert!(r.validate("26.3").is_err());
    }
}
