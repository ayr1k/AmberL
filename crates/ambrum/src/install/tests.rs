// SPDX-License-Identifier: MPL-2.0
use super::*;
use crate::download::{Algorithm, Transport, windows::Hash};
use serde_json::json;
use std::{
    fs,
    io::{Cursor, Read, Write},
    sync::atomic::{AtomicU64, Ordering},
};
static SEQ: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    app: PathBuf,
    parent: PathBuf,
    metadata: Value,
}
struct Bytes(Vec<u8>);
impl Transport for Bytes {
    fn open(&self, _: &str) -> Result<Box<dyn Read>> {
        Ok(Box::new(Cursor::new(self.0.clone())))
    }
}
fn seed(cache: &Cache, bytes: &[u8]) -> Value {
    let mut hash = Hash::new(Algorithm::Sha1).unwrap();
    hash.update(bytes).unwrap();
    let digest = hash.finish().unwrap();
    cache
        .fetch(
            &Artifact {
                url: "https://example.invalid/fixture".into(),
                size: bytes.len() as u64,
                algorithm: Algorithm::Sha1,
                digest: digest.clone(),
            },
            &Bytes(bytes.into()),
            &Cancellation::default(),
            |_| {},
        )
        .unwrap();
    json!({"url":"https://example.invalid/fixture", "sha1":digest, "size":bytes.len()})
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ambrum-install-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let app = root.join("app");
        let parent = root.join("games");
        fs::create_dir(&app).unwrap();
        fs::create_dir(&parent).unwrap();
        let cache = Cache::open(&app).unwrap();
        let client = seed(&cache, b"abc");
        let index_bytes =
            serde_json::to_vec(&json!({"objects":{"sound/a":{"hash":client["sha1"], "size":3}}}))
                .unwrap();
        let mut index = seed(&cache, &index_bytes);
        index["id"] = json!("test");
        let metadata = json!({"id":"test", "mainClass":"net.minecraft.client.main.Main", "javaVersion":{"majorVersion":21},
            "downloads":{"client":client},"libraries":[],"assetIndex":index,"arguments":{"jvm":[],"game":[]}});
        Self {
            root,
            app,
            parent,
            metadata,
        }
    }
    fn install(&self, cancel: &Cancellation, progress: impl FnMut(Progress)) -> Result<Installed> {
        vanilla(
            &Cache::open(&self.app)?,
            &self.metadata,
            &Platform::windows_x64("10.0.26100"),
            &self.parent,
            "new-game",
            1024 * 1024,
            cancel,
            progress,
        )
    }
    fn stage(&self) -> PathBuf {
        fs::read_dir(&self.parent)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with(".amberl-install-")
            })
            .unwrap()
    }
    fn interrupted_ready(&self) -> String {
        let token = Cancellation::default();
        let mut operation = String::new();
        assert!(
            self.install(&token, |p| {
                if p.phase == Phase::Revalidating {
                    operation = p.operation;
                    token.cancel();
                }
            })
            .is_err()
        );
        operation
    }
    fn native(&mut self, data: Vec<u8>) {
        let mut native = seed(&Cache::open(&self.app).unwrap(), &data);
        native["path"] = json!("org/test/native.jar");
        self.metadata["libraries"] = json!([{"natives":{"windows":"natives-windows"},
            "downloads":{"classifiers":{"natives-windows":native}},"extract":{"exclude":["META-INF/"]}}]);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn zip_files(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn materializes_new_isolated_directory_without_cache_links_and_preserves_inputs() {
    let f = Fixture::new();
    let before = f.metadata.clone();
    let mut complete = false;
    let result = f
        .install(&Cancellation::default(), |p| {
            complete |= p.phase == Phase::Complete
        })
        .unwrap();
    assert!(complete);
    assert!(!result.launch_verified);
    assert_eq!(result.materialized_files, 4);
    assert_eq!(
        fs::read(result.game_directory.join("versions/test/test.jar")).unwrap(),
        b"abc"
    );
    assert!(
        result
            .game_directory
            .join("installation.ready.json")
            .exists()
    );
    assert_eq!(
        serde_json::from_slice::<Value>(
            &fs::read(result.game_directory.join("versions/test/test.json")).unwrap()
        )
        .unwrap(),
        before
    );
    assert_eq!(f.metadata, before);
    fs::write(
        f.app
            .join("downloads-v1/sha1-a9993e364706816aba3e25717850c26c9cd0d89d"),
        b"xyz",
    )
    .unwrap();
    assert_eq!(
        fs::read(result.game_directory.join("versions/test/test.jar")).unwrap(),
        b"abc"
    );
}

#[test]
fn cancelled_before_start_and_under_budget_do_not_create_any_parent_files() {
    let f = Fixture::new();
    let token = Cancellation::default();
    token.cancel();
    assert!(f.install(&token, |_| {}).is_err());
    assert!(
        vanilla(
            &Cache::open(&f.app).unwrap(),
            &f.metadata,
            &Platform::windows_x64("10"),
            &f.parent,
            "new-game",
            1,
            &Cancellation::default(),
            |_| {}
        )
        .is_err()
    );
    assert_eq!(fs::read_dir(&f.parent).unwrap().count(), 0);
}

#[test]
fn cancellation_during_copy_keeps_incomplete_stage_and_retry_does_not_adopt_it() {
    let f = Fixture::new();
    let token = Cancellation::default();
    let mut operation = String::new();
    assert!(
        f.install(&token, |p| {
            operation = p.operation;
            if p.files_written == 1 {
                token.cancel();
            }
        })
        .is_err()
    );
    let old = f.stage();
    assert!(!old.join("installation.ready.json").exists());
    assert!(!f.parent.join("new-game").exists());
    assert!(recover(&f.parent, &operation, &Cancellation::default()).is_err());
    f.install(&Cancellation::default(), |_| {}).unwrap();
    assert!(old.exists());
}

#[test]
fn complete_stage_recovers_after_reopen_and_cannot_commit_twice() {
    let f = Fixture::new();
    let operation = f.interrupted_ready();
    assert!(!f.parent.join("new-game").exists());
    let result = recover(&f.parent, &operation, &Cancellation::default()).unwrap();
    assert_eq!(
        fs::read(result.join("versions/test/test.jar")).unwrap(),
        b"abc"
    );
    assert!(recover(&f.parent, &operation, &Cancellation::default()).is_err());
}

#[test]
fn corrupt_extra_hardlinked_or_replaced_stages_never_recover() {
    for mode in 0..5 {
        let f = Fixture::new();
        let operation = f.interrupted_ready();
        let stage = f.stage();
        let jar = stage.join("versions/test/test.jar");
        match mode {
            0 => fs::write(&jar, b"xyz").unwrap(),
            1 => fs::write(stage.join("unexpected"), b"outside plan").unwrap(),
            2 => fs::hard_link(&jar, f.root.join("alias")).unwrap(),
            3 => {
                let renamed = f.root.join("original-stage");
                fs::rename(&stage, &renamed).unwrap();
                fs::create_dir(&stage).unwrap();
                fs::copy(
                    renamed.join("installation.pending.json"),
                    stage.join("installation.pending.json"),
                )
                .unwrap();
            }
            _ => {
                let path = stage.join("installation.pending.json");
                let mut v: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                v["schema"] = json!(99);
                fs::write(path, serde_json::to_vec(&v).unwrap()).unwrap();
            }
        }
        assert!(
            recover(&f.parent, &operation, &Cancellation::default()).is_err(),
            "mode={mode}"
        );
        assert!(!f.parent.join("new-game").exists());
        assert!(stage.exists());
    }
}

#[test]
fn destination_conflicts_including_at_commit_preserve_the_existing_directory() {
    let f = Fixture::new();
    let mut operation = String::new();
    assert!(
        f.install(&Cancellation::default(), |p| {
            if p.phase == Phase::Revalidating {
                operation = p.operation;
                fs::create_dir(f.parent.join("new-game")).unwrap();
                fs::write(f.parent.join("new-game/sentinel"), b"keep me").unwrap();
            }
        })
        .is_err()
    );
    assert_eq!(
        fs::read(f.parent.join("new-game/sentinel")).unwrap(),
        b"keep me"
    );
    assert!(recover(&f.parent, &operation, &Cancellation::default()).is_err());
}

#[test]
fn shared_parent_lock_and_directory_handle_exclude_competing_operations() {
    let f = Fixture::new();
    let mut checked = false;
    f.install(&Cancellation::default(), |p| {
        if !checked && p.phase == Phase::Staging {
            checked = true;
            assert!(
                f.install(&Cancellation::default(), |_| {})
                    .unwrap_err()
                    .contains("busy")
            );
            assert!(fs::rename(&f.parent, f.root.join("moved")).is_err());
            assert!(fs::rename(f.stage(), f.root.join("moved-stage")).is_err());
        }
    })
    .unwrap();
    assert!(checked);
}

#[test]
fn cache_hardlinks_active_writers_and_game_parent_are_rejected() {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
    let f = Fixture::new();
    let jar = f
        .app
        .join("downloads-v1/sha1-a9993e364706816aba3e25717850c26c9cd0d89d");
    let writer = fs::OpenOptions::new()
        .write(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(&jar)
        .unwrap();
    assert!(f.install(&Cancellation::default(), |_| {}).is_err());
    drop(writer);
    fs::hard_link(&jar, f.root.join("alias")).unwrap();
    assert!(f.install(&Cancellation::default(), |_| {}).is_err());
    fs::remove_file(f.root.join("alias")).unwrap();
    assert_eq!(fs::read_dir(&f.parent).unwrap().count(), 0);
    fs::create_dir(f.parent.join("versions")).unwrap();
    assert!(f.install(&Cancellation::default(), |_| {}).is_err());
    assert_eq!(fs::read_dir(&f.parent).unwrap().count(), 1);
}

#[test]
fn valid_deflated_natives_extract_and_meta_inf_is_excluded() {
    let mut f = Fixture::new();
    f.native(zip_files(&[
        ("lwjgl.dll", b"native fixture"),
        ("META-INF/MANIFEST.MF", b"ignore"),
    ]));
    let result = f.install(&Cancellation::default(), |_| {}).unwrap();
    assert_eq!(result.extracted_native_files, 1);
    assert_eq!(
        fs::read(result.game_directory.join("natives/lwjgl.dll")).unwrap(),
        b"native fixture"
    );
    assert!(!result.game_directory.join("natives/META-INF").exists());
}

#[test]
fn unsafe_archive_paths_collisions_and_symlinks_fail_before_installation_writes() {
    for path in [
        "../escape.dll",
        "/absolute.dll",
        "C:/escape.dll",
        "bad\\escape.dll",
        "con.dll",
        "trailing./a",
        "a/../b",
    ] {
        let mut f = Fixture::new();
        f.native(zip_files(&[(path, b"bad")]));
        assert!(
            f.install(&Cancellation::default(), |_| {}).is_err(),
            "{path}"
        );
        assert_eq!(fs::read_dir(&f.parent).unwrap().count(), 0);
    }
    for files in [
        vec![("A.dll", &b"a"[..]), ("a.dll", &b"b"[..])],
        vec![("a", &b"a"[..]), ("a/b", &b"b"[..])],
    ] {
        let mut f = Fixture::new();
        f.native(zip_files(&files));
        assert!(f.install(&Cancellation::default(), |_| {}).is_err());
        assert_eq!(fs::read_dir(&f.parent).unwrap().count(), 0);
    }
    let mut f = Fixture::new();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.add_symlink("link", "outside", zip::write::SimpleFileOptions::default())
        .unwrap();
    f.native(zip.finish().unwrap().into_inner());
    assert!(f.install(&Cancellation::default(), |_| {}).is_err());
}

#[test]
fn expanded_native_budget_and_legacy_resource_copies_are_counted() {
    let mut f = Fixture::new();
    f.native(zip_files(&[("big.dll", &vec![0u8; 2 * 1024 * 1024])]));
    assert!(
        f.install(&Cancellation::default(), |_| {})
            .unwrap_err()
            .contains("budget")
    );
    assert_eq!(fs::read_dir(&f.parent).unwrap().count(), 0);
    let mut f = Fixture::new();
    let index = json!({"virtual":true,"map_to_resources":true,
        "objects":{"sound/a":{"hash":"a9993e364706816aba3e25717850c26c9cd0d89d","size":3}}});
    let mut descriptor = seed(
        &Cache::open(&f.app).unwrap(),
        &serde_json::to_vec(&index).unwrap(),
    );
    descriptor["id"] = json!("test");
    f.metadata["assetIndex"] = descriptor;
    let result = f.install(&Cancellation::default(), |_| {}).unwrap();
    assert_eq!(result.materialized_files, 6);
    for path in ["resources/sound/a", "assets/virtual/test/sound/a"] {
        assert_eq!(fs::read(result.game_directory.join(path)).unwrap(), b"abc");
    }
}

#[test]
fn crc_corruption_inside_a_hash_valid_native_archive_never_commits() {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        "test.dll",
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .unwrap();
    zip.write_all(b"native-crc-fixture").unwrap();
    let mut bytes = zip.finish().unwrap().into_inner();
    let offset = bytes
        .windows(b"native-crc-fixture".len())
        .position(|w| w == b"native-crc-fixture")
        .unwrap();
    bytes[offset] ^= 1;
    let mut f = Fixture::new();
    f.native(bytes);
    let mut complete = false;
    assert!(
        f.install(&Cancellation::default(), |p| complete |=
            p.phase == Phase::Complete)
            .is_err()
    );
    assert!(!complete);
    assert!(!f.parent.join("new-game").exists());
    assert!(!f.stage().join("installation.ready.json").exists());
}

#[test]
fn recovery_rejects_junctions_without_touching_the_external_target() {
    use std::{
        os::windows::process::CommandExt,
        process::{Command, Stdio},
    };
    let f = Fixture::new();
    let operation = f.interrupted_ready();
    let original = f.stage().join("versions");
    let moved = f.root.join("external-versions");
    fs::rename(&original, &moved).unwrap();
    let status = Command::new("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(&original)
        .arg(&moved)
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(
        status.success(),
        "Synthetic junction creation must succeed for this Windows test"
    );
    let outcome = recover(&f.parent, &operation, &Cancellation::default());
    // Remove only the junction itself before the disposable fixture's recursive cleanup.
    fs::remove_dir(&original).unwrap();
    assert!(outcome.is_err());
    assert_eq!(fs::read(moved.join("test/test.jar")).unwrap(), b"abc");
    assert!(!f.parent.join("new-game").exists());
}
