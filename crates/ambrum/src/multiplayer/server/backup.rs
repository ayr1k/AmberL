// SPDX-License-Identifier: MPL-2.0
//! Verified snapshots of stopped, owned servers. Restoration always creates a new server.
use super::*;
use std::collections::BTreeSet;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
    size: u64,
    sha256: String,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: u32,
    server: Option<Server>,
    files: Vec<Entry>,
}
#[derive(Clone)]
pub struct Backup {
    pub id: String,
    pub name: String,
    pub version: String,
}
fn folder(app: &Path) -> Result<DirectoryGuard> {
    let _app = DirectoryGuard::open(app)?;
    let path = app.join("server-backups");
    match fs::create_dir(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }
    DirectoryGuard::open(&path)
}
fn valid_id(id: &str) -> Result<()> {
    if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid backup identity".into());
    }
    Ok(())
}
fn validate_receipt(r: &Receipt) -> Result<&Server> {
    let s = r.server.as_ref().ok_or("Missing backup server")?;
    validate(&Data {
        schema: 1,
        servers: vec![s.clone()],
    })?;
    if r.schema != 1 || r.files.is_empty() || r.files.len() > 4096 {
        return Err("Invalid backup receipt".into());
    }
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    for e in &r.files {
        crate::manifest::relative_path(&e.path)?;
        if !seen.insert(e.path.to_ascii_lowercase())
            || e.path.eq_ignore_ascii_case("amberl-session.lock")
            || e.sha256.len() != 64
            || !e.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Invalid backup file receipt".into());
        }
        total = total.checked_add(e.size).ok_or("Backup size overflow")?;
        if total > 16 * 1024 * 1024 * 1024 {
            return Err("Backup exceeds 16 GiB".into());
        }
    }
    Ok(s)
}
pub fn list(app: &Path) -> Result<Vec<Backup>> {
    let root = folder(app)?;
    let mut result = vec![];
    for e in fs::read_dir(&root.root).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let id = e.file_name().to_string_lossy().into_owned();
        if valid_id(&id).is_err() {
            continue;
        }
        if result.len() >= 256 {
            return Err("Backup count limit".into());
        }
        let doc = Document::<Receipt>::open(&e.path(), "snapshot.json")?;
        let s = validate_receipt(&doc.data)?;
        result.push(Backup {
            id,
            name: s.name.clone(),
            version: s.version.clone(),
        });
    }
    Ok(result)
}
fn copy(
    source: &mut fs::File,
    target: &Path,
    expected: Option<&Entry>,
    cancel: &Cancellation,
    budget: &mut u64,
) -> Result<(u64, String)> {
    let parent = target.parent().ok_or("Missing snapshot parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let _guard = DirectoryGuard::open(parent)?;
    let mut out = options()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|e| e.to_string())?;
    let mut hash = Hash::new(Algorithm::Sha256)?;
    let mut size = 0u64;
    let mut buf = [0; 65536];
    loop {
        cancel.check()?;
        let n = source.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        *budget = budget
            .checked_sub(n as u64)
            .ok_or("Backup exceeds 16 GiB")?;
        size += n as u64;
        if expected.is_some_and(|e| size > e.size) {
            return Err("Backup size changed; original preserved".into());
        }
        hash.update(&buf[..n])?;
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
    }
    let digest = hash.finish()?;
    if expected.is_some_and(|e| e.size != size || e.sha256 != digest) {
        return Err("备份校验失败；原世界未修改".into());
    }
    out.sync_all().map_err(|e| e.to_string())?;
    Ok((size, digest))
}
pub fn create(app: &Path, id: &str, cancel: &Cancellation) -> Result<String> {
    let s = super::list(app)?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or("请选择服务端")?;
    let (root, _lock) = maintenance_lock(app, &s)?;
    let mut guards = vec![];
    guard_tree(&root.root, 0, &mut 0, &mut guards)?;
    let parent = folder(app)?;
    if list(app)?.len() >= 256 {
        return Err("Backup count limit".into());
    }
    let id = new_id()?;
    let stage = parent.root.join(format!("stage-{id}"));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let stage_guard = DirectoryGuard::open(&stage)?;
    let mut pending = vec![root.root.clone()];
    let mut pinned = vec![];
    // Pin the complete source set before copying: no partially changed snapshot.
    while let Some(dir) = pending.pop() {
        cancel.check()?;
        for e in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            if e.file_type().map_err(|e| e.to_string())?.is_dir() {
                pending.push(e.path());
                continue;
            }
            let path = e.path();
            let relative = path
                .strip_prefix(&root.root)
                .map_err(|_| "Outside server")?
                .to_string_lossy()
                .replace('\\', "/");
            if relative == "amberl-session.lock" {
                continue;
            }
            if pinned.len() >= 4096 {
                return Err("Backup file limit (4096)".into());
            }
            pinned.push((relative, pin(&path)?));
        }
    }
    let mut receipt = Receipt {
        schema: 1,
        server: Some(s),
        files: vec![],
    };
    let mut budget = 16 * 1024 * 1024 * 1024;
    for (path, mut input) in pinned {
        let (size, sha256) = copy(
            &mut input,
            &stage.join("files").join(&path),
            None,
            cancel,
            &mut budget,
        )?;
        receipt.files.push(Entry { path, size, sha256 });
    }
    validate_receipt(&receipt)?;
    let mut doc = Document::<Receipt>::open(&stage, "snapshot.json")?;
    doc.data = receipt;
    doc.save()?;
    drop(doc);
    cancel.check()?;
    drop(stage_guard);
    fs::rename(&stage, parent.root.join(&id)).map_err(|e| e.to_string())?;
    Ok(id)
}
pub fn restore(
    app: &Path,
    id: &str,
    name: &str,
    port: u16,
    cancel: &Cancellation,
) -> Result<Server> {
    valid_id(id)?;
    let parent = folder(app)?;
    let snapshot = parent.root.join(id);
    let doc = Document::<Receipt>::open(&snapshot, "snapshot.json")?;
    let mut s = validate_receipt(&doc.data)?.clone();
    details(name, &s.version, port)?;
    let mut registry = Document::<Data>::open(app, "servers-v1.json")?;
    validate(&registry.data)?;
    if registry.data.servers.len() >= 256 {
        return Err("Server limit reached".into());
    }
    let source = DirectoryGuard::open(&snapshot.join("files"))?;
    let mut source_guards = vec![];
    guard_tree(&source.root, 0, &mut 0, &mut source_guards)?;
    let servers = DirectoryGuard::open(&app.join("servers"))?;
    let id = new_id()?;
    let stage = servers.root.join(format!("stage-{id}"));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let stage_guard = DirectoryGuard::open(&stage)?;
    let mut budget = 16 * 1024 * 1024 * 1024;
    for e in &doc.data.files {
        copy(
            &mut pin(&source.root.join(&e.path))?,
            &stage.join(&e.path),
            Some(e),
            cancel,
            &mut budget,
        )?;
    }
    // Rewrite only the listener port in the new copy. The source server and snapshot stay intact.
    let properties = crate::paths::text(&stage, &stage.join("server.properties"), 65536)?;
    let mut found = 0;
    let mut lines = vec![];
    for line in properties.lines() {
        if line
            .split_once('=')
            .is_some_and(|(k, _)| k.trim() == "server-port")
        {
            found += 1;
            lines.push(format!("server-port={port}"));
        } else {
            lines.push(line.to_owned());
        }
    }
    if found != 1 {
        return Err("Invalid saved server port".into());
    }
    let mut f = options()
        .write(true)
        .truncate(true)
        .open(stage.join("server.properties"))
        .map_err(|e| e.to_string())?;
    f.write_all(format!("{}\n", lines.join("\n")).as_bytes())
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())?;
    drop(f);
    cancel.check()?;
    drop(stage_guard);
    let target = servers.root.join(&id);
    fs::rename(stage, &target).map_err(|e| e.to_string())?;
    s.id = id;
    s.name = name.trim().into();
    s.port = port;
    s.archived = false;
    s.object = DirectoryGuard::open(&target)?.id.clone();
    registry.data.servers.push(s.clone());
    registry.save()?;
    Ok(s)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (std::path::PathBuf, Server) {
        let app = std::env::temp_dir().join(format!("amberl-backup-{}", new_id().unwrap()));
        let id = new_id().unwrap();
        let root = app.join("servers").join(&id);
        fs::create_dir_all(root.join("world")).unwrap();
        fs::write(root.join("world/level.dat"), b"original world").unwrap();
        fs::write(root.join("server.jar"), b"synthetic jar").unwrap();
        fs::write(
            root.join("server.properties"),
            b"server-port=25565\nserver-ip=127.0.0.1\nonline-mode=true\nenable-rcon=false\n",
        )
        .unwrap();
        let s = Server {
            id,
            name: "Fixture".into(),
            version: "26.3".into(),
            port: 25565,
            object: DirectoryGuard::open(&root).unwrap().id.clone(),
            loader: None,
            archived: false,
        };
        let mut doc = Document::<Data>::open(&app, "servers-v1.json").unwrap();
        doc.data.servers.push(s.clone());
        doc.save().unwrap();
        drop(doc);
        (app, s)
    }
    #[test]
    fn verified_copy_restore_preserves_original_and_removal_is_reversible() {
        let (app, s) = fixture();
        let c = Cancellation::default();
        let id = create(&app, &s.id, &c).unwrap();
        let original = app.join("servers").join(&s.id).join("world/level.dat");
        fs::write(&original, b"later world").unwrap();
        let copy = restore(&app, &id, "Restored", 25568, &c).unwrap();
        assert_eq!(
            fs::read(app.join("servers").join(&copy.id).join("world/level.dat")).unwrap(),
            b"original world"
        );
        assert_eq!(fs::read(&original).unwrap(), b"later world");
        set_archived(&app, &s.id, true).unwrap();
        assert_eq!(super::super::list(&app).unwrap().len(), 1);
        assert_eq!(archived(&app).unwrap().len(), 1);
        set_archived(&app, &s.id, false).unwrap();
        assert_eq!(super::super::list(&app).unwrap().len(), 2);
        fs::remove_dir_all(app).unwrap();
    }
    #[test]
    fn active_session_corruption_and_cancellation_never_replace_a_world() {
        let (app, s) = fixture();
        let c = Cancellation::default();
        let lock = maintenance_lock(&app, &s).unwrap();
        assert!(create(&app, &s.id, &c).is_err());
        assert!(set_archived(&app, &s.id, true).is_err());
        drop(lock);
        let id = create(&app, &s.id, &c).unwrap();
        fs::write(
            app.join("server-backups")
                .join(&id)
                .join("files/world/level.dat"),
            b"corrupt",
        )
        .unwrap();
        assert!(restore(&app, &id, "Restored", 25568, &c).is_err());
        assert_eq!(super::super::list(&app).unwrap().len(), 1);
        c.cancel();
        assert!(create(&app, &s.id, &c).is_err());
        assert_eq!(
            fs::read(app.join("servers").join(&s.id).join("world/level.dat")).unwrap(),
            b"original world"
        );
        fs::remove_dir_all(app).unwrap();
    }
}
