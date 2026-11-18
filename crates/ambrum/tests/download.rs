// SPDX-License-Identifier: MPL-2.0
#![cfg(windows)]
use ambrum::{
    download::{Algorithm, Artifact, Cache, Https, Phase, Transport},
    task::Cancellation,
};
use std::{
    cell::Cell,
    fs,
    io::{self, Cursor, Read},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
const SHA1: &str = "a9993e364706816aba3e25717850c26c9cd0d89d";
const SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
static SEQ: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "ambrum-download-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn target(&self) -> PathBuf {
        self.0.join("downloads-v1").join(format!("sha1-{SHA1}"))
    }
    fn no_partials(&self) {
        assert!(
            fs::read_dir(self.0.join("downloads-v1")).unwrap().all(|e| e
                .unwrap()
                .path()
                .extension()
                .is_none_or(|x| x != "part"))
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
struct Bytes {
    data: Vec<u8>,
    opens: Cell<usize>,
}
impl Bytes {
    fn new(data: &[u8]) -> Self {
        Self {
            data: data.into(),
            opens: Cell::new(0),
        }
    }
}
impl Transport for Bytes {
    fn open(&self, _: &str) -> ambrum::Result<Box<dyn Read>> {
        self.opens.set(self.opens.get() + 1);
        Ok(Box::new(Cursor::new(self.data.clone())))
    }
}
fn artifact() -> Artifact {
    Artifact {
        url: "https://example.invalid/artifact?secret=do-not-log".into(),
        size: 3,
        algorithm: Algorithm::Sha1,
        digest: SHA1.into(),
    }
}

#[test]
fn sha1_sha256_match_known_vectors_and_verified_cache_avoids_network() {
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    let transport = Bytes::new(b"abc");
    for (algorithm, digest) in [(Algorithm::Sha1, SHA1), (Algorithm::Sha256, SHA256)] {
        let a = Artifact {
            algorithm,
            digest: digest.to_uppercase(),
            ..artifact()
        };
        let mut phases = vec![];
        let first = cache
            .fetch(&a, &transport, &Cancellation::default(), |p| {
                phases.push(p.phase)
            })
            .unwrap();
        assert!(!first.cache_hit);
        assert_eq!(fs::read(first.path).unwrap(), b"abc");
        assert_eq!(phases.last(), Some(&Phase::Complete));
        assert!(
            cache
                .fetch(&a, &transport, &Cancellation::default(), |_| {})
                .unwrap()
                .cache_hit
        );
    }
    assert_eq!(transport.opens.get(), 2);
    f.no_partials();
}

#[test]
fn truncated_oversized_and_same_size_corrupt_streams_never_commit() {
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    for bytes in [b"ab".as_slice(), b"abcd", b"bad"] {
        let mut completed = false;
        assert!(
            cache
                .fetch(
                    &artifact(),
                    &Bytes::new(bytes),
                    &Cancellation::default(),
                    |p| completed |= p.phase == Phase::Complete
                )
                .is_err()
        );
        assert!(!completed);
        assert!(!f.target().exists());
        f.no_partials();
    }
    assert!(
        cache
            .fetch(
                &artifact(),
                &Bytes::new(b"abc"),
                &Cancellation::default(),
                |_| {}
            )
            .is_ok()
    );
}

#[test]
fn cancellation_before_network_and_before_commit_cleans_staging() {
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    let transport = Bytes::new(b"abc");
    let token = Cancellation::default();
    token.cancel();
    assert!(
        cache
            .fetch(&artifact(), &transport, &token, |_| {})
            .is_err()
    );
    assert_eq!(transport.opens.get(), 0);
    for phase in [Phase::Downloading, Phase::Verifying] {
        let token = Cancellation::default();
        assert!(
            cache
                .fetch(&artifact(), &transport, &token, |p| {
                    if p.phase == phase {
                        token.cancel();
                    }
                })
                .is_err()
        );
        assert!(!f.target().exists());
        f.no_partials();
    }
}

struct Broken;
impl Transport for Broken {
    fn open(&self, _: &str) -> ambrum::Result<Box<dyn Read>> {
        struct Reader(bool);
        impl Read for Reader {
            fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
                if self.0 {
                    return Err(io::Error::new(
                        io::ErrorKind::ConnectionReset,
                        "sensitive transport diagnostic",
                    ));
                }
                self.0 = true;
                b[0] = b'a';
                Ok(1)
            }
        }
        Ok(Box::new(Reader(false)))
    }
}
#[test]
fn interrupted_attempt_can_retry_and_does_not_expose_url_or_reader_diagnostics() {
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    let error = cache
        .fetch(&artifact(), &Broken, &Cancellation::default(), |_| {})
        .unwrap_err();
    assert!(!error.contains("secret"));
    assert!(!error.contains("sensitive"));
    f.no_partials();
    assert!(
        cache
            .fetch(
                &artifact(),
                &Bytes::new(b"abc"),
                &Cancellation::default(),
                |_| {}
            )
            .is_ok()
    );
}

#[test]
fn corrupt_cache_is_replaced_only_after_success_and_commit_failure_preserves_it() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    fs::write(f.target(), b"bad").unwrap();
    assert!(
        cache
            .fetch(&artifact(), &Broken, &Cancellation::default(), |_| {})
            .is_err()
    );
    assert_eq!(fs::read(f.target()).unwrap(), b"bad");
    let blocker = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(f.target())
        .unwrap();
    assert!(
        cache
            .fetch(
                &artifact(),
                &Bytes::new(b"abc"),
                &Cancellation::default(),
                |_| {}
            )
            .is_err()
    );
    assert_eq!(fs::read(f.target()).unwrap(), b"bad");
    f.no_partials();
    drop(blocker);
    cache
        .fetch(
            &artifact(),
            &Bytes::new(b"abc"),
            &Cancellation::default(),
            |_| {},
        )
        .unwrap();
    assert_eq!(fs::read(f.target()).unwrap(), b"abc");
}

#[test]
fn identical_concurrent_artifact_is_busy_without_starting_another_transport() {
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    let transport = Bytes::new(b"abc");
    cache
        .fetch(&artifact(), &transport, &Cancellation::default(), |p| {
            if p.phase == Phase::Downloading {
                let other = Cache::open(&f.0).unwrap();
                assert!(
                    other
                        .fetch(&artifact(), &transport, &Cancellation::default(), |_| {})
                        .unwrap_err()
                        .contains("busy")
                );
            }
        })
        .unwrap();
    assert_eq!(transport.opens.get(), 1);
}

#[test]
fn invalid_descriptor_or_credential_url_is_rejected_before_network() {
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    let transport = Bytes::new(b"abc");
    for url in [
        "http://example.invalid/file",
        "file:///C:/test",
        "https://user:password@example.invalid/file",
        "https://@example.invalid/file",
        "https://example.invalid/file#fragment",
        "https://example.invalid/\r\nheader",
        "https://example.invalid\\other/file",
    ] {
        let a = Artifact {
            url: url.into(),
            ..artifact()
        };
        assert!(
            cache
                .fetch(&a, &transport, &Cancellation::default(), |_| {})
                .is_err()
        );
    }
    for digest in ["../escape", "a9993e364706816aba3e25717850c26c9cd0d89z"] {
        let a = Artifact {
            digest: digest.into(),
            ..artifact()
        };
        assert!(
            cache
                .fetch(&a, &transport, &Cancellation::default(), |_| {})
                .is_err()
        );
    }
    assert_eq!(transport.opens.get(), 0);
    assert!(Https::validate("https://piston-data.mojang.com/v1/objects/test/client.jar").is_ok());
}

#[test]
fn app_cache_cannot_be_game_root_and_hardlinked_targets_are_not_overwritten() {
    let f = Fixture::new();
    let cache = Cache::open(&f.0).unwrap();
    let original = f.0.join("original");
    fs::write(&original, b"original").unwrap();
    fs::hard_link(&original, f.target()).unwrap();
    assert!(
        cache
            .fetch(
                &artifact(),
                &Bytes::new(b"abc"),
                &Cancellation::default(),
                |_| {}
            )
            .is_err()
    );
    assert_eq!(fs::read(original).unwrap(), b"original");
    fs::create_dir(f.0.join("versions")).unwrap();
    assert!(Cache::open(&f.0).is_err());
}
