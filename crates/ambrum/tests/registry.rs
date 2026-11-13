// SPDX-License-Identifier: MPL-2.0
#![cfg(windows)]
use ambrum::{library::LibraryId, registry::Registry, task::Cancellation};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static SEQ: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "amberl-registry-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("app")).unwrap();
        fs::create_dir_all(root.join("game/versions/demo")).unwrap();
        fs::write(
            root.join("game/versions/demo/demo.json"),
            r#"{"id":"demo","mainClass":"net.minecraft.client.main.Main"}"#,
        )
        .unwrap();
        Self(root)
    }
    fn app(&self) -> PathBuf {
        self.0.join("app")
    }
    fn game(&self) -> PathBuf {
        self.0.join("game")
    }
    fn open(&self) -> Registry {
        Registry::open(&self.app()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn attach_deduplicates_persists_and_never_writes_game_data() {
    let f = Fixture::new();
    let original = fs::read(f.game().join("versions/demo/demo.json")).unwrap();
    let id = {
        let mut registry = f.open();
        let first = registry.attach(&f.game()).unwrap();
        assert_eq!(registry.attach(&f.game()).unwrap().id, first.id);
        assert_eq!(registry.entries().len(), 1);
        first.id
    };
    let registry = f.open();
    assert_eq!(registry.entries()[0].id, id);
    let scan = registry.scan(&id, &Cancellation::default()).unwrap();
    assert_eq!(scan.instances[0].id.library, LibraryId::Registered(id));
    assert_eq!(fs::read_dir(f.game()).unwrap().count(), 1);
    assert_eq!(
        fs::read(f.game().join("versions/demo/demo.json")).unwrap(),
        original
    );
}

#[test]
fn same_path_with_replacement_directory_requires_user_reconfirmation() {
    let f = Fixture::new();
    let mut registry = f.open();
    let entry = registry.attach(&f.game()).unwrap();
    fs::rename(f.game(), f.0.join("original-game")).unwrap();
    fs::create_dir_all(f.game().join("versions")).unwrap();
    assert!(registry.scan(&entry.id, &Cancellation::default()).is_err());
    assert!(registry.attach(&f.game()).is_err());
    assert_eq!(registry.entries()[0].id, entry.id);
    registry.detach(&entry.id).unwrap();
    assert_ne!(registry.attach(&f.game()).unwrap().id, entry.id);
}

#[test]
fn concurrent_registry_writer_fails_fast_and_lock_is_released_on_drop() {
    let f = Fixture::new();
    let first = f.open();
    assert!(Registry::open(&f.app()).is_err());
    drop(first);
    assert!(Registry::open(&f.app()).is_ok());
}

#[test]
fn corrupted_or_future_registry_is_not_reset() {
    let f = Fixture::new();
    for bytes in ["{broken", r#"{"schema":99,"entries":[]}"#] {
        fs::write(f.app().join("libraries.json"), bytes).unwrap();
        assert!(Registry::open(&f.app()).is_err());
        assert_eq!(
            fs::read_to_string(f.app().join("libraries.json")).unwrap(),
            bytes
        );
    }
}

#[test]
fn cancelled_scan_and_offline_detach_preserve_the_library() {
    let f = Fixture::new();
    let mut registry = f.open();
    let entry = registry.attach(&f.game()).unwrap();
    let token = Cancellation::default();
    token.cancel();
    assert!(registry.scan(&entry.id, &token).is_err());
    fs::rename(f.game(), f.0.join("offline-game")).unwrap();
    registry.detach(&entry.id).unwrap();
    assert!(registry.entries().is_empty());
    assert!(f.0.join("offline-game/versions/demo/demo.json").is_file());
}

#[test]
fn failed_commit_keeps_old_snapshot_and_orphan_stage_is_ignored() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = Fixture::new();
    let entry = f.open().attach(&f.game()).unwrap();
    let registry_path = f.app().join("libraries.json");
    let original = fs::read(&registry_path).unwrap();
    let blocker = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&registry_path)
        .unwrap();
    let mut registry = f.open();
    assert!(registry.detach(&entry.id).is_err());
    assert_eq!(registry.entries()[0].id, entry.id);
    assert_eq!(fs::read(&registry_path).unwrap(), original);
    drop(registry);
    drop(blocker);
    assert_eq!(f.open().entries()[0].id, entry.id);
}

#[test]
fn app_data_cannot_be_inside_attached_game_library() {
    let f = Fixture::new();
    let app = f.game().join("app-data");
    fs::create_dir(&app).unwrap();
    let mut registry = Registry::open(&app).unwrap();
    assert!(registry.attach(&f.game()).is_err());
    assert!(registry.entries().is_empty());
}

#[test]
fn registry_lock_hardlink_cannot_modify_an_external_file() {
    let f = Fixture::new();
    let original = f.0.join("original.txt");
    fs::write(&original, "preserve me").unwrap();
    fs::hard_link(&original, f.app().join("libraries.lock")).unwrap();
    assert!(Registry::open(&f.app()).is_err());
    assert_eq!(fs::read_to_string(original).unwrap(), "preserve me");
}

#[test]
fn game_root_is_rejected_before_creating_app_configuration() {
    let f = Fixture::new();
    assert!(Registry::open(&f.game()).is_err());
    assert_eq!(fs::read_dir(f.game()).unwrap().count(), 1);
}

#[test]
fn registry_directory_cannot_be_renamed_while_open() {
    let f = Fixture::new();
    let registry = f.open();
    assert!(fs::rename(f.app(), f.0.join("moved-app")).is_err());
    drop(registry);
    fs::rename(f.app(), f.0.join("moved-app")).unwrap();
}
