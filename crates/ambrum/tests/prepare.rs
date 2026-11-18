// SPDX-License-Identifier: MPL-2.0
#![cfg(windows)]
use ambrum::{
    download::{Algorithm, Artifact, Cache, Transport},
    manifest::Platform,
    prepare::{Phase, vanilla},
    task::Cancellation,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    fs,
    io::{Cursor, Read},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
const ABC: &str = "a9993e364706816aba3e25717850c26c9cd0d89d";
const EMPTY: &str = "da39a3ee5e6b4b0d3255bfef95601890afd80709";
const INDEX_HASH: &str = "99e81adb2e8fd2e5418c61e53e8aa5f17a83ad08";
const INDEX: &[u8] =
    br#"{"objects":{"sound/a":{"hash":"a9993e364706816aba3e25717850c26c9cd0d89d","size":3}}}"#;
static SEQ: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "ambrum-prepare-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[derive(Default)]
struct Network {
    opens: RefCell<Vec<String>>,
    fail_library: Cell<bool>,
}
impl Transport for Network {
    fn open(&self, url: &str) -> ambrum::Result<Box<dyn Read>> {
        self.opens.borrow_mut().push(url.to_owned());
        let data: &[u8] = match url {
            "https://example.invalid/index" => INDEX,
            "https://example.invalid/client" => b"abc",
            "https://example.invalid/library" if self.fail_library.replace(false) => {
                return Err("injected transport failure".into());
            }
            "https://example.invalid/library" => b"",
            _ => return Err("unexpected resource request".into()),
        };
        Ok(Box::new(Cursor::new(data.to_vec())))
    }
}
fn descriptor(url: &str, hash: &str, size: u64) -> Value {
    json!({"url":format!("https://example.invalid/{url}"), "sha1":hash, "size":size})
}
fn metadata() -> Value {
    let mut index = descriptor("index", INDEX_HASH, INDEX.len() as u64);
    index["id"] = json!("test");
    let mut library = descriptor("library", EMPTY, 0);
    library["path"] = json!("test/lib.jar");
    json!({"id":"test", "mainClass":"net.minecraft.client.main.Main", "downloads":{"client":descriptor("client", ABC, 3)},
        "libraries":[{"downloads":{"artifact":library}}], "assetIndex":index, "arguments":{"jvm":[],"game":[]}})
}
fn platform() -> Platform {
    Platform::windows_x64("10.0.26100")
}

#[test]
fn index_first_deduplication_complete_progress_and_revalidated_retry() {
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    let net = Network::default();
    let mut progress = vec![];
    let result = vanilla(
        &cache,
        &metadata(),
        &platform(),
        87,
        &net,
        &Cancellation::default(),
        |p| progress.push(p),
    )
    .unwrap();
    assert_eq!(result.verified_files, 3);
    assert_eq!(result.verified_bytes, 87);
    assert_eq!(result.logical_files, 4);
    assert_eq!(result.asset_mappings, 1);
    assert_eq!(result.cache_hits, 0);
    assert_eq!(net.opens.borrow()[0], "https://example.invalid/index");
    assert_eq!(net.opens.borrow().len(), 3);
    assert_eq!(progress[0].total_files, None);
    assert_eq!(progress.last().unwrap().phase, Phase::Complete);
    assert_eq!(progress.last().unwrap().verified_files, 3);
    let again = vanilla(
        &cache,
        &metadata(),
        &platform(),
        87,
        &net,
        &Cancellation::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(again.cache_hits, 3);
    assert_eq!(net.opens.borrow().len(), 3);
}

#[test]
fn interrupted_batch_keeps_verified_files_and_retries_only_missing_resources() {
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    let net = Network::default();
    net.fail_library.set(true);
    let mut complete = false;
    assert!(
        vanilla(
            &cache,
            &metadata(),
            &platform(),
            87,
            &net,
            &Cancellation::default(),
            |p| complete |= p.phase == Phase::Complete
        )
        .is_err()
    );
    assert!(!complete);
    let retry = vanilla(
        &cache,
        &metadata(),
        &platform(),
        87,
        &net,
        &Cancellation::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(retry.cache_hits, 2);
    assert_eq!(net.opens.borrow().len(), 4);
}

#[test]
fn cancellation_before_request_and_after_index_never_reports_batch_complete() {
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    let net = Network::default();
    let cancel = Cancellation::default();
    cancel.cancel();
    assert!(vanilla(&cache, &metadata(), &platform(), 87, &net, &cancel, |_| {}).is_err());
    assert!(net.opens.borrow().is_empty());
    let cancel = Cancellation::default();
    let mut complete = false;
    assert!(
        vanilla(&cache, &metadata(), &platform(), 87, &net, &cancel, |p| {
            complete |= p.phase == Phase::Complete;
            if p.phase == Phase::AssetIndex
                && p.resource
                    .is_some_and(|r| r.phase == ambrum::download::Phase::Complete)
            {
                cancel.cancel();
            }
        })
        .is_err()
    );
    assert!(!complete);
    assert_eq!(net.opens.borrow().len(), 1);
}

#[test]
fn budgets_and_unsupported_metadata_fail_before_resource_requests() {
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    let net = Network::default();
    assert!(
        vanilla(
            &cache,
            &metadata(),
            &platform(),
            86,
            &net,
            &Cancellation::default(),
            |_| {}
        )
        .is_err()
    );
    let mut bad = metadata();
    bad["inheritsFrom"] = json!("other");
    assert!(
        vanilla(
            &cache,
            &bad,
            &platform(),
            1000,
            &net,
            &Cancellation::default(),
            |_| {}
        )
        .is_err()
    );
    bad = metadata();
    bad["assetIndex"]["size"] = json!(8 * 1024 * 1024 + 1);
    assert!(
        vanilla(
            &cache,
            &bad,
            &platform(),
            u64::MAX,
            &net,
            &Cancellation::default(),
            |_| {}
        )
        .is_err()
    );
    assert!(net.opens.borrow().is_empty());
}

#[test]
fn asset_discovery_exceeding_budget_stops_after_index() {
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    let net = Network::default();
    let mut value = metadata();
    value["downloads"]["client"] = descriptor("library", EMPTY, 0);
    assert!(
        vanilla(
            &cache,
            &value,
            &platform(),
            84,
            &net,
            &Cancellation::default(),
            |_| {}
        )
        .is_err()
    );
    assert_eq!(&*net.opens.borrow(), &["https://example.invalid/index"]);
}

#[test]
fn cached_metadata_is_reverified_and_corruption_recovered_without_losing_other_files() {
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    let net = Network::default();
    vanilla(
        &cache,
        &metadata(),
        &platform(),
        87,
        &net,
        &Cancellation::default(),
        |_| {},
    )
    .unwrap();
    let artifact = Artifact {
        url: "https://example.invalid/index".into(),
        size: 84,
        algorithm: Algorithm::Sha1,
        digest: INDEX_HASH.into(),
    };
    let path = f.0.join("downloads-v1").join(format!("sha1-{INDEX_HASH}"));
    fs::write(&path, vec![b'x'; 84]).unwrap();
    assert!(
        cache
            .read_json(&artifact, &Cancellation::default())
            .unwrap_err()
            .contains("checksum")
    );
    let retry = vanilla(
        &cache,
        &metadata(),
        &platform(),
        87,
        &net,
        &Cancellation::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(retry.cache_hits, 2);
    assert_eq!(net.opens.borrow().len(), 4);
    assert_eq!(fs::read(path).unwrap(), INDEX);
}

#[test]
fn linked_metadata_and_game_roots_are_rejected_without_mutation() {
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    let net = Network::default();
    vanilla(
        &cache,
        &metadata(),
        &platform(),
        87,
        &net,
        &Cancellation::default(),
        |_| {},
    )
    .unwrap();
    let artifact = Artifact {
        url: "https://example.invalid/index".into(),
        size: 84,
        algorithm: Algorithm::Sha1,
        digest: INDEX_HASH.into(),
    };
    let path = f.0.join("downloads-v1").join(format!("sha1-{INDEX_HASH}"));
    fs::hard_link(&path, f.0.join("alias")).unwrap();
    assert!(
        cache
            .read_json(&artifact, &Cancellation::default())
            .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), INDEX);
    let game = Fixture::new();
    fs::create_dir(game.0.join("versions")).unwrap();
    assert!(Cache::open(&game.0).is_err());
    assert!(!game.0.join("downloads-v1").exists());
}

#[test]
fn metadata_reader_refuses_active_writers_and_non_json_payloads() {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    vanilla(
        &cache,
        &metadata(),
        &platform(),
        87,
        &Network::default(),
        &Cancellation::default(),
        |_| {},
    )
    .unwrap();
    let mut artifact = Artifact {
        url: "https://example.invalid/index".into(),
        size: 84,
        algorithm: Algorithm::Sha1,
        digest: INDEX_HASH.into(),
    };
    let path = f.0.join("downloads-v1").join(format!("sha1-{INDEX_HASH}"));
    let writer = fs::OpenOptions::new()
        .write(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(path)
        .unwrap();
    assert!(
        cache
            .read_json(&artifact, &Cancellation::default())
            .is_err()
    );
    drop(writer);
    assert!(cache.read_json(&artifact, &Cancellation::default()).is_ok());
    artifact.size = 3;
    artifact.digest = ABC.into();
    assert!(
        cache
            .read_json(&artifact, &Cancellation::default())
            .unwrap_err()
            .contains("JSON")
    );
}
