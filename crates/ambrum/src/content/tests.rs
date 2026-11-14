// SPDX-License-Identifier: MPL-2.0
use super::*;
use crate::download::{Algorithm, Artifact, Transport};
use std::io::Cursor;
struct Bytes(Vec<u8>);
impl Transport for Bytes {
    fn open(&self, _: &str) -> Result<Box<dyn Read>> {
        Ok(Box::new(Cursor::new(self.0.clone())))
    }
}
fn fixture() -> (PathBuf, Target, PathBuf) {
    let app = std::env::temp_dir().join(format!("amberl-content-{}", new_id().unwrap()));
    let id = new_id().unwrap();
    let root = app.join("servers").join(&id);
    fs::create_dir_all(&root).unwrap();
    let object = DirectoryGuard::open(&root).unwrap().id.clone();
    let data = serde_json::json!({"schema":1,"servers":[{"id":id,"name":"Content QA","version":"26.3","port":25565,"object":object,"archived":false,"loader":null}]});
    fs::write(
        app.join("servers-v1.json"),
        serde_json::to_vec(&data).unwrap(),
    )
    .unwrap();
    (app, Target::Server { id }, root)
}
fn resource(app: &Path, name: &str) -> modrinth::File {
    let bytes = name.as_bytes().to_vec();
    let mut hash = Hash::new(Algorithm::Sha1).unwrap();
    hash.update(&bytes).unwrap();
    let digest = hash.finish().unwrap();
    let artifact = Artifact {
        url: format!("https://cdn.modrinth.com/data/fixture/{name}.jar"),
        size: bytes.len() as u64,
        algorithm: Algorithm::Sha1,
        digest,
    };
    Cache::open(app)
        .unwrap()
        .fetch(&artifact, &Bytes(bytes), &Cancellation::default(), |_| {})
        .unwrap();
    modrinth::File {
        project: name.into(),
        version: "one".into(),
        title: name.into(),
        path: format!("mods/{name}.jar"),
        artifact,
    }
}
fn plan(app: &Path, t: &Target, names: &[&str]) -> modrinth::Plan {
    modrinth::Plan {
        context: context(app, t, &Cancellation::default()).unwrap(),
        files: names.iter().map(|n| resource(app, n)).collect(),
        optional: vec![],
        incompatible: vec![],
    }
}
#[test]
fn shared_dependencies_survive_removal_and_both_groups_can_be_restored() {
    let (app, t, root) = fixture();
    let c = Cancellation::default();
    let a = install(&app, &t, &plan(&app, &t, &["a", "shared"]), &c).unwrap();
    let b = install(&app, &t, &plan(&app, &t, &["b", "shared"]), &c).unwrap();
    change(&app, &t, &a, "remove", &c).unwrap();
    assert!(!root.join("mods/a.jar").exists());
    assert!(root.join("mods/shared.jar").exists());
    change(&app, &t, &b, "remove", &c).unwrap();
    assert!(!root.join("mods/shared.jar").exists());
    change(&app, &t, &a, "restore", &c).unwrap();
    change(&app, &t, &b, "restore", &c).unwrap();
    assert_eq!(fs::read(root.join("mods/shared.jar")).unwrap(), b"shared");
    assert!(
        list(&app, &t, &c)
            .unwrap()
            .iter()
            .all(|g| g.state == "installed")
    );
    fs::remove_dir_all(app).unwrap();
}
#[test]
fn unmanaged_or_changed_content_and_live_sessions_are_preserved() {
    let (app, t, root) = fixture();
    let c = Cancellation::default();
    let p = plan(&app, &t, &["a"]);
    fs::create_dir(root.join("mods")).unwrap();
    fs::write(root.join("mods/a.jar"), b"manual").unwrap();
    assert!(install(&app, &t, &p, &c).is_err());
    assert_eq!(fs::read(root.join("mods/a.jar")).unwrap(), b"manual");
    fs::rename(root.join("mods/a.jar"), root.join("manual-preserved.jar")).unwrap();
    let id = install(&app, &t, &p, &c).unwrap();
    fs::write(root.join("mods/a.jar"), b"changed").unwrap();
    assert!(change(&app, &t, &id, "remove", &c).is_err());
    assert_eq!(fs::read(root.join("mods/a.jar")).unwrap(), b"changed");
    let lock = options()
        .write(true)
        .open(root.join("amberl-session.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(context(&app, &t, &c).is_err());
    drop(lock);
    fs::remove_dir_all(app).unwrap();
}
#[test]
fn interrupted_install_resumes_from_journal_without_overwriting_files() {
    let (app, t, root) = fixture();
    let c = Cancellation::default();
    let p = plan(&app, &t, &["a", "b"]);
    let object = DirectoryGuard::open(&root).unwrap().id.clone();
    let g = Group {
        id: new_id().unwrap(),
        title: "Interrupted".into(),
        state: "pending".into(),
        target: t.clone(),
        object,
        files: p.files,
        incompatible: vec![],
    };
    let mut doc = Document::<Data>::open(&app, "content-v1.json").unwrap();
    doc.data.groups.push(g.clone());
    doc.save().unwrap();
    drop(doc);
    fs::create_dir(root.join("mods")).unwrap();
    fs::write(root.join("mods/a.jar"), b"a").unwrap();
    change(&app, &t, &g.id, "resume", &c).unwrap();
    assert_eq!(fs::read(root.join("mods/b.jar")).unwrap(), b"b");
    assert_eq!(list(&app, &t, &c).unwrap()[0].state, "installed");
    fs::remove_dir_all(app).unwrap();
}
