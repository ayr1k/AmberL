// SPDX-License-Identifier: MPL-2.0
//! Compatible content discovery and recoverable writes to explicitly owned installations.
pub mod modrinth;
#[cfg(test)]
mod tests;
use crate::{
    Result,
    download::{Cache, Https, windows::Hash},
    registry::{DirectoryGuard, ObjectId, info, new_id, options},
    state::Document,
    task::Cancellation,
    transaction::Installed,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Target {
    Client { root: PathBuf, version: String },
    Server { id: String },
}
struct Lease {
    root: DirectoryGuard,
    context: modrinth::Context,
    _client: Option<Installed>,
    _server: Option<fs::File>,
}
fn lease(app: &Path, target: &Target, c: &Cancellation) -> Result<Lease> {
    match target {
        Target::Client { root, version } => {
            crate::paths::name(version)?;
            let mut client = Installed::open(root, c)?;
            let meta = client.json(&format!("versions/{version}/{version}.json"))?;
            if meta["id"] != version.as_str() {
                return Err("Game content identity mismatch".into());
            }
            let loader = meta
                .pointer("/amberlLoader/choice/kind")
                .and_then(|v| v.as_str())
                .unwrap_or("vanilla")
                .to_ascii_lowercase();
            Ok(Lease {
                root: DirectoryGuard::open(&client.root)?,
                context: modrinth::Context {
                    game: version.clone(),
                    loader,
                    server: false,
                },
                _client: Some(client),
                _server: None,
            })
        }
        Target::Server { id } => {
            let (root, lock, s) = crate::multiplayer::server::content_lock(app, id)?;
            let loader = s
                .loader
                .as_ref()
                .map_or("vanilla", |l| l.choice.kind.as_str())
                .to_ascii_lowercase();
            Ok(Lease {
                root,
                context: modrinth::Context {
                    game: s.version,
                    loader,
                    server: true,
                },
                _client: None,
                _server: Some(lock),
            })
        }
    }
}
pub fn context(app: &Path, target: &Target, c: &Cancellation) -> Result<modrinth::Context> {
    Ok(lease(app, target, c)?.context)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: String,
    pub title: String,
    pub state: String,
    target: Target,
    object: ObjectId,
    files: Vec<modrinth::File>,
    #[serde(default)]
    incompatible: Vec<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    schema: u32,
    groups: Vec<Group>,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            schema: 1,
            groups: vec![],
        }
    }
}
fn validate(data: &Data) -> Result<()> {
    if data.schema != 1 || data.groups.len() > 256 {
        return Err("Content registry limit or schema mismatch".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    for g in &data.groups {
        if g.id.len() != 32
            || !g.id.bytes().all(|b| b.is_ascii_hexdigit())
            || !ids.insert(&g.id)
            || !["pending", "installed", "removing", "removed", "restoring"]
                .contains(&g.state.as_str())
            || g.files.is_empty()
            || g.files.len() > 128
        {
            return Err("Invalid content installation record".into());
        }
        let mut paths = std::collections::BTreeSet::new();
        for f in &g.files {
            crate::manifest::relative_path(&f.path)?;
            let (dir, name) = f.path.split_once('/').ok_or("Invalid content path")?;
            crate::paths::name(name)?;
            if !["mods", "resourcepacks", "shaderpacks"].contains(&dir)
                || !paths.insert(f.path.to_ascii_lowercase())
                || !f.artifact.url.starts_with("https://cdn.modrinth.com/data/")
            {
                return Err("Unexpected content destination or origin".into());
            }
            f.artifact.validate()?;
        }
    }
    Ok(())
}
pub fn list(app: &Path, target: &Target, c: &Cancellation) -> Result<Vec<Group>> {
    let root = lease(app, target, c)?;
    let doc = Document::<Data>::open(app, "content-v1.json")?;
    validate(&doc.data)?;
    Ok(doc
        .data
        .groups
        .into_iter()
        .filter(|g| g.object == root.root.id)
        .collect())
}
fn pinned(path: &Path, f: &modrinth::File, c: &Cancellation) -> Result<fs::File> {
    let mut file = options()
        .share_mode(FILE_SHARE_READ)
        .open(path)
        .map_err(|e| e.to_string())?;
    if info(&file)?.nNumberOfLinks != 1
        || !file.metadata().map_err(|e| e.to_string())?.is_file()
        || file.metadata().map_err(|e| e.to_string())?.len() != f.artifact.size
    {
        return Err("Content file changed; preserved".into());
    }
    let mut hash = Hash::new(f.artifact.algorithm)?;
    let mut buf = [0; 65536];
    loop {
        c.check()?;
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n])?;
    }
    if hash.finish()? != f.artifact.digest {
        return Err("内容已修改，原文件保留；未继续操作".into());
    }
    Ok(file)
}
fn directory(root: &Path, path: &str) -> Result<DirectoryGuard> {
    let parent = root
        .join(path)
        .parent()
        .ok_or("Missing content directory")?
        .to_path_buf();
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    DirectoryGuard::open(&parent)
}
fn used(data: &Data, group: &Group, file: &modrinth::File) -> bool {
    data.groups.iter().any(|g| {
        g.id != group.id
            && g.object == group.object
            && g.state != "removed"
            && g.files
                .iter()
                .any(|f| f.path.eq_ignore_ascii_case(&file.path))
    })
}
pub fn install(
    app: &Path,
    target: &Target,
    plan: &modrinth::Plan,
    c: &Cancellation,
) -> Result<String> {
    let root = lease(app, target, c)?;
    if root.context != plan.context {
        return Err("内容目标已变化，请重新预览".into());
    }
    let mut doc = Document::<Data>::open(app, "content-v1.json")?;
    validate(&doc.data)?;
    if doc.data.groups.len() >= 256 {
        return Err("Content group limit".into());
    }
    let group = Group {
        id: new_id()?,
        title: plan
            .files
            .first()
            .ok_or("Empty content plan")?
            .title
            .clone(),
        state: "pending".into(),
        target: target.clone(),
        object: root.root.id.clone(),
        files: plan.files.clone(),
        incompatible: plan.incompatible.clone(),
    };
    validate(&Data {
        schema: 1,
        groups: vec![group.clone()],
    })?;
    for g in doc
        .data
        .groups
        .iter()
        .filter(|g| g.object == root.root.id && g.state != "removed")
    {
        if g.state != "installed" {
            return Err("请先继续或移除未完成的内容任务".into());
        }
        if g.files.len() == group.files.len()
            && g.files.iter().all(|f| {
                group
                    .files
                    .iter()
                    .any(|n| n.project == f.project && n.version == f.version)
            })
        {
            return Err("此内容与依赖已安装".into());
        }
        if group
            .files
            .iter()
            .any(|f| g.incompatible.contains(&f.project))
        {
            return Err("已安装内容禁止与所选内容同时使用".into());
        }
        for f in &g.files {
            if plan.incompatible.contains(&f.project) {
                return Err("已安装内容与所选内容互斥".into());
            }
            if group
                .files
                .iter()
                .any(|n| n.project == f.project && n.version != f.version)
            {
                return Err("已存在该项目的其他版本；请先移除对应安装组".into());
            }
        }
    }
    for f in &group.files {
        let dir = directory(&root.root.root, &f.path)?;
        let path = root.root.root.join(&f.path);
        if path.try_exists().map_err(|e| e.to_string())? {
            if !used(&doc.data, &group, f) {
                return Err("目标已有未托管文件，保留原文件；请先调整选择".into());
            }
            pinned(&path, f, c)?;
        }
        drop(dir);
    }
    let cache = Cache::open(app)?;
    for f in &group.files {
        cache.fetch(&f.artifact, &Https, c, |_| {})?;
    }
    doc.data.groups.push(group.clone());
    doc.save()?;
    finish_install(&root, &cache, &mut doc, &group, c)?;
    Ok(group.id)
}
fn finish_install(
    root: &Lease,
    cache: &Cache,
    doc: &mut Document<Data>,
    group: &Group,
    c: &Cancellation,
) -> Result<()> {
    for f in &group.files {
        c.check()?;
        let _dir = directory(&root.root.root, &f.path)?;
        let target = root.root.root.join(&f.path);
        if target.try_exists().map_err(|e| e.to_string())? {
            pinned(&target, f, c)?;
            continue;
        }
        cache.fetch(&f.artifact, &Https, c, |_| {})?;
        let stage = target.with_file_name(format!(".amberl-content-{}.tmp", new_id()?));
        let mut out = options()
            .write(true)
            .create_new(true)
            .open(&stage)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut cache.verified_file(&f.artifact, c)?, &mut out)
            .and_then(|_| out.flush())
            .and_then(|_| out.sync_all())
            .map_err(|e| e.to_string())?;
        drop(out);
        c.check()?;
        fs::rename(stage, target).map_err(|e| e.to_string())?;
    }
    doc.data
        .groups
        .iter_mut()
        .find(|g| g.id == group.id)
        .ok_or("Missing content group")?
        .state = "installed".into();
    doc.save()
}
pub fn change(app: &Path, target: &Target, id: &str, action: &str, c: &Cancellation) -> Result<()> {
    let root = lease(app, target, c)?;
    let mut doc = Document::<Data>::open(app, "content-v1.json")?;
    validate(&doc.data)?;
    let group = doc
        .data
        .groups
        .iter()
        .find(|g| g.id == id && g.object == root.root.id)
        .ok_or("内容选择已失效")?
        .clone();
    if action == "resume" {
        if group.state != "pending" {
            return Err("该安装无需继续".into());
        }
        return finish_install(&root, &Cache::open(app)?, &mut doc, &group, c);
    }
    let removing = match action {
        "remove" => true,
        "restore" => false,
        _ => return Err("Invalid content action".into()),
    };
    if removing && !["installed", "pending", "removing"].contains(&group.state.as_str())
        || !removing && !["removed", "restoring"].contains(&group.state.as_str())
    {
        return Err("内容状态已变化，请刷新".into());
    }
    doc.data
        .groups
        .iter_mut()
        .find(|g| g.id == id)
        .unwrap()
        .state = if removing { "removing" } else { "restoring" }.into();
    doc.save()?;
    for f in &group.files {
        c.check()?;
        let _dir = directory(&root.root.root, &f.path)?;
        let trash_rel = format!(".amberl-content-trash/{}/{}", group.id, f.path);
        let _trash = directory(&root.root.root, &trash_rel)?;
        let live = root.root.root.join(&f.path);
        let trash = root.root.root.join(trash_rel);
        if removing && used(&doc.data, &group, f) {
            if live.try_exists().map_err(|e| e.to_string())? {
                pinned(&live, f, c)?;
            }
            continue;
        }
        let (source, destination) = if removing {
            (&live, &trash)
        } else {
            (&trash, &live)
        };
        if destination.try_exists().map_err(|e| e.to_string())? {
            pinned(destination, f, c)?;
            continue;
        }
        if !source.try_exists().map_err(|e| e.to_string())? {
            if removing {
                continue;
            }
            // A dependency may have been retained by another group and later moved
            // to that group's trash. Reuse its verified cache object without moving
            // anyone else's recovery copy or overwriting a live file.
            let cache = Cache::open(app)?;
            cache.fetch(&f.artifact, &Https, c, |_| {})?;
            let stage = live.with_file_name(format!(".amberl-restore-{}.tmp", new_id()?));
            let mut out = options()
                .write(true)
                .create_new(true)
                .open(&stage)
                .map_err(|e| e.to_string())?;
            std::io::copy(&mut cache.verified_file(&f.artifact, c)?, &mut out)
                .and_then(|_| out.sync_all())
                .map_err(|e| e.to_string())?;
            drop(out);
            c.check()?;
            fs::rename(stage, &live).map_err(|e| e.to_string())?;
            continue;
        }
        pinned(source, f, c)?;
        fs::rename(source, destination).map_err(|e| e.to_string())?;
        pinned(destination, f, c)?;
    }
    doc.data
        .groups
        .iter_mut()
        .find(|g| g.id == id)
        .unwrap()
        .state = if removing { "removed" } else { "installed" }.into();
    doc.save()
}
