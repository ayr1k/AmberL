// SPDX-License-Identifier: MPL-2.0
//! Official version discovery over platform HTTPS; separate from authenticated APIs.
use crate::{
    Result,
    download::{Algorithm, Artifact, Cache, Transport, windows::Hash},
    paths,
    task::Cancellation,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    io::{Cursor, Read},
    path::PathBuf,
    time::{Duration, Instant},
};
pub const OFFICIAL_CATALOG: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const LIMIT: usize = 8 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct Resolved {
    pub version: String,
    pub metadata_sha1: String,
    pub metadata_path: PathBuf,
    pub cache_hit: bool,
}

pub(crate) fn bounded(
    transport: &impl Transport,
    url: &str,
    cancel: &Cancellation,
) -> Result<Vec<u8>> {
    cancel.check()?;
    let started = Instant::now();
    let mut input = transport.open(url)?;
    let mut bytes = Vec::new();
    let mut buffer = [0; 65536];
    loop {
        cancel.check()?;
        if started.elapsed() > Duration::from_secs(60) {
            return Err("Version discovery deadline exceeded".into());
        }
        let n = input
            .read(&mut buffer)
            .map_err(|_| "Version metadata download interrupted")?;
        if n == 0 {
            break;
        }
        if bytes.len() + n > LIMIT {
            return Err("Version discovery exceeds 8 MiB".into());
        }
        bytes.extend_from_slice(&buffer[..n]);
    }
    cancel.check()?;
    Ok(bytes)
}

#[derive(Clone, Debug, Serialize)]
pub struct Version {
    pub id: String,
    pub kind: String,
}
/// Keep provider order (newest first), reject ambiguous identities before selection.
pub fn list(transport: &impl Transport, cancel: &Cancellation) -> Result<Vec<Version>> {
    let data: Value = serde_json::from_slice(&bounded(transport, OFFICIAL_CATALOG, cancel)?)
        .map_err(|_| "Invalid official version catalog")?;
    parse_list(&data)
}
fn parse_list(data: &Value) -> Result<Vec<Version>> {
    let items = data["versions"].as_array().ok_or("Missing version list")?;
    if items.len() > 20_000 {
        return Err("Version catalog entry limit".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    items
        .iter()
        .map(|v| {
            let id = v["id"].as_str().ok_or("Invalid version identity")?;
            paths::name(id)?;
            if !seen.insert(id.to_lowercase()) {
                return Err("Duplicate version identity".into());
            }
            let kind = v["type"].as_str().ok_or("Missing version type")?;
            if !matches!(kind, "release" | "snapshot" | "old_beta" | "old_alpha") {
                return Err("Unknown version type".into());
            }
            Ok(Version {
                id: id.into(),
                kind: kind.into(),
            })
        })
        .collect()
}

pub fn resolve(
    cache: &Cache,
    version: &str,
    transport: &impl Transport,
    cancel: &Cancellation,
) -> Result<Resolved> {
    paths::name(version)?;
    let catalog: Value = serde_json::from_slice(&bounded(transport, OFFICIAL_CATALOG, cancel)?)
        .map_err(|_| "Invalid official version catalog")?;
    let versions = catalog
        .get("versions")
        .and_then(Value::as_array)
        .ok_or("Missing version list")?;
    if versions.len() > 20_000 {
        return Err("Version catalog entry limit".into());
    }
    let mut matches = versions
        .iter()
        .filter(|v| v.get("id").and_then(Value::as_str) == Some(version));
    let entry = matches
        .next()
        .ok_or("Version is not in the official catalog")?;
    if matches.next().is_some() {
        return Err("Duplicate version catalog entry".into());
    }
    let url = entry
        .get("url")
        .and_then(Value::as_str)
        .ok_or("Missing version metadata URL")?;
    if ![
        "https://piston-meta.mojang.com/v1/packages/",
        "https://launchermeta.mojang.com/v1/packages/",
    ]
    .iter()
    .any(|prefix| url.starts_with(prefix))
    {
        return Err("Unrecognized official metadata origin".into());
    }
    let digest = entry
        .get("sha1")
        .and_then(Value::as_str)
        .ok_or("Missing version metadata SHA-1")?;
    let mut artifact = Artifact {
        url: url.into(),
        algorithm: Algorithm::Sha1,
        digest: digest.into(),
        size: 0,
    };
    artifact.validate()?;
    let bytes = bounded(transport, url, cancel)?;
    let mut hash = Hash::new(Algorithm::Sha1)?;
    hash.update(&bytes)?;
    if !hash.finish()?.eq_ignore_ascii_case(digest) {
        return Err("Version metadata checksum mismatch".into());
    }
    let metadata: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid version metadata JSON")?;
    if metadata.get("id").and_then(Value::as_str) != Some(version) {
        return Err("Version metadata identity mismatch".into());
    }
    artifact.size = bytes.len() as u64;
    struct Downloaded(Vec<u8>);
    impl Transport for Downloaded {
        fn open(&self, _: &str) -> Result<Box<dyn Read>> {
            Ok(Box::new(Cursor::new(self.0.clone())))
        }
    }
    let downloaded = cache.fetch(&artifact, &Downloaded(bytes), cancel, |_| {})?;
    Ok(Resolved {
        version: version.into(),
        metadata_sha1: digest.into(),
        metadata_path: downloaded.path,
        cache_hit: downloaded.cache_hit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::new_id;
    use serde_json::json;
    use std::{cell::RefCell, fs};
    struct Network {
        catalog: Vec<u8>,
        metadata: Vec<u8>,
        opens: RefCell<Vec<String>>,
    }
    impl Transport for Network {
        fn open(&self, url: &str) -> Result<Box<dyn Read>> {
            self.opens.borrow_mut().push(url.into());
            Ok(Box::new(Cursor::new(if url == OFFICIAL_CATALOG {
                self.catalog.clone()
            } else {
                self.metadata.clone()
            })))
        }
    }
    #[test]
    fn version_list_preserves_order_and_rejects_bad_or_ambiguous_ids() {
        let rows=parse_list(&json!({"versions":[{"id":"26.3","type":"release"},{"id":"26.3-snapshot-1","type":"snapshot"}]})).unwrap();
        assert_eq!(rows[0].id, "26.3");
        assert_eq!(rows[1].kind, "snapshot");
        for data in [
            json!({"versions":[{"id":"../x","type":"release"}]}),
            json!({"versions":[{"id":"A","type":"release"},{"id":"a","type":"release"}]}),
            json!({"versions":[{"id":"a","type":"future-unknown"}]}),
        ] {
            assert!(parse_list(&data).is_err());
        }
    }
    #[test]
    fn resolves_pinned_metadata_and_reuses_verified_cache_without_storing_catalog() {
        let root = std::env::temp_dir().join(format!("ambrum-catalog-{}", new_id().unwrap()));
        fs::create_dir(&root).unwrap();
        {
            let cache = Cache::open(&root).unwrap();
            let bytes = br#"{"id":"test"}"#.to_vec();
            let mut hash = Hash::new(Algorithm::Sha1).unwrap();
            hash.update(&bytes).unwrap();
            let digest = hash.finish().unwrap();
            let net = Network { catalog: serde_json::to_vec(&json!({"versions":[{"id":"test","sha1":digest,"url":"https://piston-meta.mojang.com/v1/packages/test/test.json"}]})).unwrap(), metadata: bytes.clone(), opens: RefCell::new(vec![]) };
            let first = resolve(&cache, "test", &net, &Cancellation::default()).unwrap();
            assert!(!first.cache_hit);
            assert_eq!(fs::read(first.metadata_path).unwrap(), bytes);
            assert!(
                resolve(&cache, "test", &net, &Cancellation::default())
                    .unwrap()
                    .cache_hit
            );
            assert_eq!(net.opens.borrow().len(), 4);
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_bad_identity_digest_origin_duplicates_and_cancellation() {
        let root = std::env::temp_dir().join(format!("ambrum-catalog-{}", new_id().unwrap()));
        fs::create_dir(&root).unwrap();
        {
            let cache = Cache::open(&root).unwrap();
            for mode in 0..5 {
                let bytes = if mode == 0 {
                    br#"{"id":"wrong"}"#.to_vec()
                } else {
                    br#"{"id":"test"}"#.to_vec()
                };
                let mut hash = Hash::new(Algorithm::Sha1).unwrap();
                hash.update(&bytes).unwrap();
                let mut entry = json!({"id":"test","sha1":hash.finish().unwrap(),"url":"https://piston-meta.mojang.com/v1/packages/test/test.json"});
                if mode == 1 {
                    entry["sha1"] = json!("0".repeat(40));
                }
                if mode == 2 {
                    entry["url"] = json!("https://example.invalid/test.json");
                }
                let entries = if mode == 3 {
                    vec![entry.clone(), entry]
                } else {
                    vec![entry]
                };
                let net = Network {
                    catalog: serde_json::to_vec(&json!({"versions":entries})).unwrap(),
                    metadata: bytes,
                    opens: RefCell::new(vec![]),
                };
                let cancel = Cancellation::default();
                if mode == 4 {
                    cancel.cancel();
                }
                assert!(resolve(&cache, "test", &net, &cancel).is_err());
                if mode == 2 || mode == 3 {
                    assert_eq!(net.opens.borrow().len(), 1);
                }
                if mode == 4 {
                    assert!(net.opens.borrow().is_empty());
                }
            }
            assert_eq!(fs::read_dir(root.join("downloads-v1")).unwrap().count(), 0);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
