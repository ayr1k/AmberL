// SPDX-License-Identifier: MPL-2.0
//! Mojang's Windows x64 Java runtimes, installed independently without changing PATH.
use crate::{
    Result,
    catalog::bounded,
    download::{Algorithm, Artifact, Cache, Https, windows::Hash},
    registry::{DirectoryGuard, ObjectId, info, new_id, options},
    state::Document,
    task::Cancellation,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
const CATALOG: &str = "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    runtimes: Vec<Receipt>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    id: String,
    major: u32,
    object: ObjectId,
    files: Vec<Resource>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resource {
    path: String,
    artifact: Artifact,
}
fn artifact(v: &Value, metadata: bool) -> Result<Artifact> {
    let url = v["url"].as_str().ok_or("Missing runtime URL")?;
    let origins = if metadata {
        &[
            "https://piston-meta.mojang.com/",
            "https://launchermeta.mojang.com/",
        ][..]
    } else {
        &[
            "https://piston-data.mojang.com/",
            "https://launcher.mojang.com/",
        ][..]
    };
    if !origins.iter().any(|o| url.starts_with(o)) {
        return Err("Unexpected Java download origin".into());
    }
    let a = Artifact {
        url: url.into(),
        size: v["size"].as_u64().ok_or("Missing Java size")?,
        algorithm: Algorithm::Sha1,
        digest: v["sha1"].as_str().ok_or("Missing Java checksum")?.into(),
    };
    a.validate()?;
    if a.size > 512 * 1024 * 1024 || (metadata && a.size > 2 * 1024 * 1024) {
        return Err("Java resource size limit".into());
    }
    Ok(a)
}
fn resources(manifest: &Value) -> Result<Vec<Resource>> {
    let files = manifest["files"]
        .as_object()
        .ok_or("Invalid Java manifest")?;
    if files.len() > 4096 {
        return Err("Java file count limit".into());
    }
    let mut result = vec![];
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    for (path, file) in files {
        crate::manifest::relative_path(path)?;
        if !seen.insert(path.to_ascii_lowercase()) {
            return Err("Aliased runtime entries".into());
        }
        match file["type"].as_str() {
            Some("directory") => {}
            Some("file") => {
                let a = artifact(&file["downloads"]["raw"], false)?;
                total = total.checked_add(a.size).ok_or("Java size overflow")?;
                if total > 1024 * 1024 * 1024 {
                    return Err("Java exceeds 1 GiB".into());
                }
                result.push(Resource {
                    path: path.clone(),
                    artifact: a,
                });
            }
            _ => return Err("Linked or unsupported Java entry rejected".into()),
        }
    }
    if !result.iter().any(|f| f.path == "bin/java.exe") {
        return Err("Java executable missing".into());
    }
    Ok(result)
}
fn component(major: u32) -> Result<&'static str> {
    match major {
        8 => Ok("jre-legacy"),
        16 => Ok("java-runtime-alpha"),
        17 => Ok("java-runtime-gamma"),
        21 => Ok("java-runtime-delta"),
        25 => Ok("java-runtime-epsilon"),
        _ => Err("请选择 Java 8、16、17、21 或 25".into()),
    }
}
pub fn install(
    app: &Path,
    major: u32,
    cancel: &Cancellation,
    mut progress: impl FnMut(usize, usize),
) -> Result<PathBuf> {
    let key = component(major)?;
    let mut doc = Document::<Data>::open(app, "java-runtimes-v1.json")?;
    if doc.data.runtimes.len() >= 32 {
        return Err("Java runtime limit reached".into());
    }
    let catalog: Value = serde_json::from_slice(&bounded(&Https, CATALOG, cancel)?)
        .map_err(|_| "Invalid Java catalog")?;
    let candidates = catalog["windows-x64"][key]
        .as_array()
        .ok_or("Missing Windows x64 runtime")?;
    let entry = candidates.first().ok_or("此 Java 版本暂无官方下载")?;
    let meta = artifact(&entry["manifest"], true)?;
    let cache = Cache::open(app)?;
    cache.fetch(&meta, &Https, cancel, |_| {})?;
    let mut bytes = vec![];
    cache
        .verified_file(&meta, cancel)?
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let files = resources(
        &serde_json::from_slice::<Value>(&bytes).map_err(|_| "Invalid runtime manifest")?,
    )?;
    let parent = app.join("java-runtimes");
    match fs::create_dir(&parent) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }
    let _parent = DirectoryGuard::open(&parent)?;
    let id = format!("java-{major}-{}", new_id()?);
    let stage = parent.join(format!("stage-{id}"));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let stage_guard = DirectoryGuard::open(&stage)?;
    for (index, f) in files.iter().enumerate() {
        cancel.check()?;
        cache.fetch(&f.artifact, &Https, cancel, |_| {})?;
        let target = stage.join(&f.path);
        let parent = target.parent().ok_or("Missing Java parent")?;
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
        progress(index + 1, files.len());
    }
    cancel.check()?;
    drop(stage_guard);
    let target = parent.join(&id);
    fs::rename(stage, &target).map_err(|e| e.to_string())?;
    let guard = DirectoryGuard::open(&target)?;
    let record = Receipt {
        id,
        major,
        object: guard.id.clone(),
        files,
    };
    let java = verify(&parent, &record, cancel)?;
    super::compatible(
        &java.java.ok_or("Downloaded Java failed inspection")?,
        Some(major),
    )?;
    let executable = java.executable;
    doc.data.runtimes.push(record);
    doc.save()?;
    Ok(executable)
}
fn verify(parent: &Path, r: &Receipt, cancel: &Cancellation) -> Result<super::Candidate> {
    crate::paths::name(&r.id)?;
    component(r.major)?;
    let root = DirectoryGuard::open(&parent.join(&r.id))?;
    if root.id != r.object || r.files.is_empty() || r.files.len() > 4096 {
        return Err("Java runtime identity changed".into());
    }
    let mut pins = vec![];
    let mut guards = vec![];
    for f in &r.files {
        cancel.check()?;
        crate::manifest::relative_path(&f.path)?;
        let path = root.root.join(&f.path);
        guards.push(DirectoryGuard::open(
            path.parent().ok_or("Missing Java parent")?,
        )?);
        let mut file = options()
            .share_mode(FILE_SHARE_READ)
            .open(path)
            .map_err(|e| e.to_string())?;
        if info(&file)?.nNumberOfLinks != 1
            || file.metadata().map_err(|e| e.to_string())?.len() != f.artifact.size
        {
            return Err("Java runtime file changed".into());
        }
        let mut hash = Hash::new(f.artifact.algorithm)?;
        let mut buf = [0; 65536];
        loop {
            cancel.check()?;
            let n = file.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n])?;
        }
        if hash.finish()? != f.artifact.digest {
            return Err("Java runtime checksum mismatch".into());
        }
        pins.push(file);
    }
    let executable = root.root.join("bin/java.exe");
    let java = super::inspect(&executable, cancel)?;
    super::compatible(&java, Some(r.major))?;
    Ok(super::Candidate {
        executable,
        java: Some(java),
        issue: None,
    })
}
pub fn discover(app: &Path, cancel: &Cancellation) -> Result<Vec<super::Candidate>> {
    let doc = Document::<Data>::open(app, "java-runtimes-v1.json")?;
    if doc.data.runtimes.len() > 32 {
        return Err("Java registry limit".into());
    }
    let mut result = vec![];
    for r in &doc.data.runtimes {
        cancel.check()?;
        crate::paths::name(&r.id)?;
        match verify(&app.join("java-runtimes"), r, cancel) {
            Ok(c) => result.push(c),
            Err(e) => result.push(super::Candidate {
                executable: app.join("java-runtimes").join(&r.id).join("bin/java.exe"),
                java: None,
                issue: Some(e),
            }),
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_manifest_rejects_links_aliases_and_untrusted_downloads() {
        let good = serde_json::json!({"files":{"bin/java.exe":{"type":"file","downloads":{"raw":{"url":"https://piston-data.mojang.com/runtime/java.exe","size":1,"sha1":"0".repeat(40)}}}}});
        assert_eq!(resources(&good).unwrap().len(), 1);
        let mut bad = good.clone();
        bad["files"]["bin/escape"] = serde_json::json!({"type":"link","target":"../../other"});
        assert!(resources(&bad).is_err());
        let mut bad = good.clone();
        bad["files"]["BIN/JAVA.EXE"] = good["files"]["bin/java.exe"].clone();
        assert!(resources(&bad).is_err());
        let mut bad = good;
        bad["files"]["bin/java.exe"]["downloads"]["raw"]["url"] =
            "https://piston-data.mojang.com.evil/java.exe".into();
        assert!(resources(&bad).is_err());
    }
}
