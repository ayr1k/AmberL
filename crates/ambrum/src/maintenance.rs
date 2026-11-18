// SPDX-License-Identifier: MPL-2.0
//! Owned client snapshots and presentation state. Restore always commits a new instance.
use crate::{
    Result,
    download::{Algorithm, windows::Hash},
    registry::{DirectoryGuard, ObjectId, info, new_id, options},
    state::Document,
    task::Cancellation,
    transaction::{Destination, Installed},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub root: PathBuf,
    object: ObjectId,
    pub version: String,
    pub title: String,
    pub icon: String,
    pub archived: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    schema: u32,
    profiles: Vec<Profile>,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            schema: 1,
            profiles: vec![],
        }
    }
}
fn validate_profile(p: &Profile) -> Result<()> {
    crate::paths::name(&p.version)?;
    if !p.root.is_absolute() {
        return Err("Invalid instance location".into());
    }
    crate::loader::display(&mut serde_json::json!({}), &p.title, &p.icon)
}
pub fn profiles(app: &Path) -> Result<Vec<Profile>> {
    let doc = Document::<Data>::open(app, "instances-v1.json")?;
    validate_data(&doc.data)?;
    Ok(doc.data.profiles)
}
fn validate_data(data: &Data) -> Result<()> {
    if data.schema != 1 || data.profiles.len() > 256 {
        return Err("Invalid instance state; preserved".into());
    }
    let mut roots = BTreeSet::new();
    for p in &data.profiles {
        validate_profile(p)?;
        if !roots.insert(&p.root) {
            return Err("Duplicate instance state".into());
        }
    }
    Ok(())
}
pub fn matching<'a>(profiles: &'a [Profile], root: &Path) -> Result<Option<&'a Profile>> {
    let root = DirectoryGuard::open(root)?;
    let p = profiles.iter().find(|p| p.root == root.root);
    if p.is_some_and(|p| p.object != root.id) {
        return Err("实例目录身份已变化，原记录已保留".into());
    }
    Ok(p)
}
pub fn ready(app: &Path, root: &Path) -> Result<()> {
    if matching(&profiles(app)?, root)?.is_some_and(|p| p.archived) {
        return Err("游戏已移除，请先从备份与恢复页面恢复显示".into());
    }
    Ok(())
}
pub fn save_profile(
    app: &Path,
    root: &Path,
    version: &str,
    title: &str,
    icon: &str,
    archived: bool,
    c: &Cancellation,
) -> Result<()> {
    let mut instance = Installed::open(root, c)?;
    let metadata = instance.json(&format!("versions/{version}/{version}.json"))?;
    if metadata["id"] != version {
        return Err("Instance version changed".into());
    }
    let root = DirectoryGuard::open(&instance.root)?;
    let p = Profile {
        root: root.root.clone(),
        object: root.id.clone(),
        version: version.into(),
        title: title.trim().into(),
        icon: icon.into(),
        archived,
    };
    validate_profile(&p)?;
    let mut doc = Document::<Data>::open(app, "instances-v1.json")?;
    // The exclusive document lock protects concurrent profile edits. Never replace another identity.
    validate_data(&doc.data)?;
    if let Some(old) = doc.data.profiles.iter_mut().find(|p| p.root == root.root) {
        if old.object != root.id {
            return Err("Instance replaced; state preserved".into());
        }
        *old = p;
    } else {
        if doc.data.profiles.len() >= 256 {
            return Err("Instance state limit".into());
        }
        doc.data.profiles.push(p);
    }
    doc.save()
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
    size: u64,
    sha256: String,
    immutable: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: u32,
    version: String,
    title: String,
    icon: String,
    files: Vec<Entry>,
}
#[derive(Clone)]
pub struct Backup {
    pub id: String,
    pub version: String,
    pub title: String,
}
fn valid_id(id: &str) -> Result<()> {
    if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid snapshot ID".into());
    }
    Ok(())
}
fn folder(app: &Path) -> Result<DirectoryGuard> {
    let app = DirectoryGuard::open(app)?;
    let path = app.root.join("client-backups");
    match fs::create_dir(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }
    DirectoryGuard::open(&path)
}
fn pin(path: &Path) -> Result<fs::File> {
    let f = options()
        .share_mode(FILE_SHARE_READ)
        .open(path)
        .map_err(|e| e.to_string())?;
    if info(&f)?.nNumberOfLinks != 1 || !f.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Linked snapshot file rejected".into());
    }
    Ok(f)
}
fn hash(f: &mut fs::File, c: &Cancellation) -> Result<(u64, String)> {
    f.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut hash = Hash::new(Algorithm::Sha256)?;
    let mut size = 0u64;
    let mut b = [0; 65536];
    loop {
        c.check()?;
        let n = f.read(&mut b).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > 16 * 1024 * 1024 * 1024 {
            return Err("Snapshot file exceeds 16 GiB".into());
        }
        hash.update(&b[..n])?;
    }
    f.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    Ok((size, hash.finish()?))
}
fn excluded(path: &str) -> bool {
    matches!(
        path.to_ascii_lowercase().as_str(),
        "installation.pending.json" | "installation.ready.json" | ".amberl-session.lock"
    )
}
fn validate(r: &Receipt) -> Result<()> {
    crate::paths::name(&r.version)?;
    crate::loader::display(&mut serde_json::json!({}), &r.title, &r.icon)?;
    if r.schema != 1 || r.files.is_empty() || r.files.len() > 210000 {
        return Err("Invalid snapshot receipt".into());
    }
    let metadata = format!("versions/{0}/{0}.json", r.version);
    if !r.files.iter().any(|e| e.path == metadata && e.immutable) {
        return Err("Snapshot lacks immutable instance metadata".into());
    }
    let mut paths = BTreeSet::new();
    let mut total = 0u64;
    for e in &r.files {
        crate::manifest::relative_path(&e.path)?;
        if excluded(&e.path)
            || !paths.insert(e.path.to_lowercase())
            || e.sha256.len() != 64
            || !e.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Invalid snapshot file".into());
        }
        total = total.checked_add(e.size).ok_or("Snapshot size overflow")?;
        if total > 16 * 1024 * 1024 * 1024 {
            return Err("Snapshot exceeds 16 GiB".into());
        }
    }
    Ok(())
}
fn read(root: &Path) -> Result<(Receipt, fs::File)> {
    let mut f = pin(&root.join("snapshot.json"))?;
    let mut bytes = vec![];
    (&mut f)
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Snapshot receipt limit".into());
    }
    let r = serde_json::from_slice(&bytes).map_err(|_| "Invalid snapshot receipt")?;
    validate(&r)?;
    Ok((r, f))
}
pub fn backups(app: &Path) -> Result<Vec<Backup>> {
    let root = folder(app)?;
    let mut result = vec![];
    for e in fs::read_dir(&root.root).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let id = e.file_name().to_string_lossy().into_owned();
        if valid_id(&id).is_err() {
            continue;
        }
        if result.len() >= 256 {
            return Err("Snapshot limit".into());
        }
        let guard = DirectoryGuard::open(&e.path())?;
        let (r, _pin) = read(&guard.root)?;
        result.push(Backup {
            id,
            version: r.version,
            title: r.title,
        });
    }
    Ok(result)
}
pub fn backup(app: &Path, root: &Path, version: &str, c: &Cancellation) -> Result<String> {
    let mut installed = Installed::open(root, c)?;
    let metadata = installed.json(&format!("versions/{version}/{version}.json"))?;
    if metadata["id"] != version {
        return Err("Instance identity changed".into());
    }
    let app_guard = DirectoryGuard::open(app)?;
    if installed.root.starts_with(&app_guard.root) || app_guard.root.starts_with(&installed.root) {
        return Err("Snapshot and source directories overlap".into());
    }
    let immutable = installed.immutable_paths();
    let parent = folder(app)?;
    if backups(app)?.len() >= 256 {
        return Err("Snapshot limit".into());
    }
    let saved = profiles(app)?;
    let display = matching(&saved, &installed.root)?;
    let mut r = Receipt {
        schema: 1,
        version: version.into(),
        title: display
            .map(|p| p.title.as_str())
            .or(metadata
                .pointer("/amberlDisplay/name")
                .and_then(|v| v.as_str()))
            .unwrap_or(version)
            .into(),
        icon: display
            .map(|p| p.icon.as_str())
            .or(metadata
                .pointer("/amberlDisplay/icon")
                .and_then(|v| v.as_str()))
            .unwrap_or("grass")
            .into(),
        files: vec![],
    };
    let id = new_id()?;
    let stage = parent.root.join(format!("stage-{id}"));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let stage_guard = DirectoryGuard::open(&stage)?;
    let mut dirs = vec![installed.root.clone()];
    let mut guards = vec![];
    let mut files = vec![];
    while let Some(dir) = dirs.pop() {
        if guards.len() + dirs.len() > 210000
            || dir
                .strip_prefix(&installed.root)
                .map_err(|_| "Outside instance")?
                .components()
                .count()
                > 32
        {
            return Err("Snapshot directory limit".into());
        }
        let guard = DirectoryGuard::open(&dir)?;
        for e in fs::read_dir(&guard.root).map_err(|e| e.to_string())? {
            c.check()?;
            let path = e.map_err(|e| e.to_string())?.path();
            let path = crate::paths::contained(&installed.root, &path)?;
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(&installed.root)
                .map_err(|_| "Outside instance")?
                .to_string_lossy()
                .replace('\\', "/");
            if excluded(&rel) {
                continue;
            }
            if files.len() >= 210000 {
                return Err("Snapshot file limit".into());
            }
            files.push((rel, pin(&path)?));
        }
        guards.push(guard);
    }
    let mut total = 0u64;
    for (path, mut input) in files {
        let (size, sha256) = hash(&mut input, c)?;
        total = total.checked_add(size).ok_or("Snapshot size overflow")?;
        if total > 16 * 1024 * 1024 * 1024 {
            return Err("Snapshot exceeds 16 GiB".into());
        }
        let target = stage.join("files").join(&path);
        fs::create_dir_all(target.parent().ok_or("Invalid target")?).map_err(|e| e.to_string())?;
        let _guard = DirectoryGuard::open(target.parent().ok_or("Invalid target")?)?;
        let mut out = options()
            .write(true)
            .create_new(true)
            .open(target)
            .map_err(|e| e.to_string())?;
        let mut b = [0; 65536];
        loop {
            c.check()?;
            let n = input.read(&mut b).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            out.write_all(&b[..n]).map_err(|e| e.to_string())?;
        }
        out.sync_all().map_err(|e| e.to_string())?;
        r.files.push(Entry {
            immutable: immutable.contains(&path),
            path,
            size,
            sha256,
        });
    }
    validate(&r)?;
    let bytes = serde_json::to_vec(&r).map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Snapshot receipt limit".into());
    }
    let mut f = options()
        .write(true)
        .create_new(true)
        .open(stage.join("snapshot.json"))
        .map_err(|e| e.to_string())?;
    f.write_all(&bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())?;
    drop(f);
    c.check()?;
    drop(stage_guard);
    fs::rename(stage, parent.root.join(&id)).map_err(|e| e.to_string())?;
    Ok(id)
}
pub fn restore(
    app: &Path,
    id: &str,
    parent: &Path,
    name: &str,
    c: &Cancellation,
) -> Result<PathBuf> {
    valid_id(id)?;
    crate::install::check_destination(app, parent, name)?;
    let snapshots = folder(app)?;
    let snapshot = DirectoryGuard::open(&snapshots.root.join(id))?;
    let (r, _receipt) = read(&snapshot.root)?;
    let mut stage = Destination::open(parent)?.begin(name)?;
    for e in &r.files {
        c.check()?;
        let source =
            crate::paths::contained(&snapshot.root, &snapshot.root.join("files").join(&e.path))?;
        let _parent = DirectoryGuard::open(source.parent().ok_or("Missing snapshot parent")?)?;
        let mut input = pin(&source)?;
        let (size, digest) = hash(&mut input, c)?;
        if size != e.size || digest != e.sha256 {
            return Err("备份校验失败；原游戏未修改".into());
        }
        if e.immutable {
            stage.write(&e.path, &mut input, size, c)?;
        } else {
            stage.write_mutable(&e.path, &mut input, size, c)?;
        }
    }
    stage.seal(c)?;
    let root = stage.commit(c)?;
    let register = || -> Result<()> {
        save_profile(app, &root, &r.version, &r.title, &r.icon, false, c)?;
        crate::registry::Registry::open(app)?.attach(&root)?;
        Ok(())
    };
    register().map_err(|e| {
        format!(
            "游戏副本已恢复到 {}，登记未完成；可重新添加此目录。{e}",
            root.display()
        )
    })?;
    Ok(root)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("amberl-maintenance-{}", new_id().unwrap()));
        let app = base.join("app");
        let parent = base.join("games");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir(&parent).unwrap();
        let c = Cancellation::default();
        let mut stage = Destination::open(&parent)
            .unwrap()
            .begin("original")
            .unwrap();
        let meta = br#"{"id":"26.3","type":"release"}"#;
        stage
            .write(
                "versions/26.3/26.3.json",
                &mut meta.as_slice(),
                meta.len() as u64,
                &c,
            )
            .unwrap();
        stage
            .write("client.jar", &mut b"immutable".as_slice(), 9, &c)
            .unwrap();
        stage.seal(&c).unwrap();
        let root = stage.commit(&c).unwrap();
        fs::create_dir(root.join("saves")).unwrap();
        fs::write(root.join("saves/world.dat"), b"saved-world").unwrap();
        (app, parent, root)
    }
    #[test]
    fn snapshot_restores_new_instance_with_mutable_world_and_immutable_resources() {
        let (app, parent, root) = fixture();
        let c = Cancellation::default();
        save_profile(&app, &root, "26.3", "朋友的世界", "anvil", false, &c).unwrap();
        let id = backup(&app, &root, "26.3", &c).unwrap();
        let restored = restore(&app, &id, &parent, "restored", &c).unwrap();
        assert_eq!(
            fs::read(restored.join("saves/world.dat")).unwrap(),
            b"saved-world"
        );
        assert_eq!(
            fs::read(root.join("saves/world.dat")).unwrap(),
            b"saved-world"
        );
        fs::write(restored.join("saves/world.dat"), b"next-save").unwrap();
        Installed::open(&restored, &c).unwrap();
        fs::write(restored.join("client.jar"), b"tampered!").unwrap();
        assert!(Installed::open(&restored, &c).is_err());
        assert!(restore(&app, &id, &parent, "original", &c).is_err());
        assert_eq!(
            matching(&profiles(&app).unwrap(), &root)
                .unwrap()
                .unwrap()
                .title,
            "朋友的世界"
        );
        fs::remove_dir_all(app.parent().unwrap()).unwrap();
    }
    #[test]
    fn archive_restore_live_session_and_corrupt_snapshot_preserve_original() {
        let (app, parent, root) = fixture();
        let c = Cancellation::default();
        let session = Installed::open(&root, &c).unwrap();
        assert!(backup(&app, &root, "26.3", &c).is_err());
        assert!(save_profile(&app, &root, "26.3", "QA", "grass", true, &c).is_err());
        drop(session);
        save_profile(&app, &root, "26.3", "QA", "grass", true, &c).unwrap();
        assert!(ready(&app, &root).is_err());
        save_profile(&app, &root, "26.3", "QA", "grass", false, &c).unwrap();
        ready(&app, &root).unwrap();
        let id = backup(&app, &root, "26.3", &c).unwrap();
        fs::write(
            app.join("client-backups")
                .join(&id)
                .join("files/saves/world.dat"),
            b"bad",
        )
        .unwrap();
        assert!(restore(&app, &id, &parent, "bad-copy", &c).is_err());
        assert!(!parent.join("bad-copy").exists());
        assert_eq!(
            fs::read(root.join("saves/world.dat")).unwrap(),
            b"saved-world"
        );
        c.cancel();
        assert!(backup(&app, &root, "26.3", &c).is_err());
        fs::remove_dir_all(app.parent().unwrap()).unwrap();
    }
}
