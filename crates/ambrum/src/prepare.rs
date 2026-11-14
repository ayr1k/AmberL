// SPDX-License-Identifier: MPL-2.0
//! Prepare verified Vanilla resources in the app cache. Never install or launch.
use crate::{
    Result,
    download::{Artifact, Cache, Progress as ResourceProgress, Transport},
    manifest::{AssetPlan, Platform, VersionPlan},
    task::Cancellation,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Phase {
    AssetIndex,
    Resources,
    Complete,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Progress {
    pub phase: Phase,
    pub verified_files: usize,
    /// Unknown until the verified asset index has been parsed and the full plan checked.
    pub total_files: Option<usize>,
    pub resource: Option<ResourceProgress>,
}

#[derive(Debug, Serialize)]
pub struct Prepared {
    pub version: String,
    pub verified_files: usize,
    pub verified_bytes: u64,
    pub cache_hits: usize,
    pub logical_files: usize,
    pub asset_mappings: usize,
}

fn resources<'a>(
    artifacts: impl Iterator<Item = &'a Artifact>,
    max_bytes: u64,
) -> Result<BTreeMap<String, Artifact>> {
    let mut unique: BTreeMap<String, Artifact> = BTreeMap::new();
    let mut bytes = 0u64;
    for artifact in artifacts {
        let key = artifact.key()?;
        if let Some(old) = unique.get(&key) {
            if old.size != artifact.size {
                return Err("Conflicting resource sizes".into());
            }
            continue;
        }
        bytes = bytes
            .checked_add(artifact.size)
            .ok_or("Resource size overflow")?;
        if bytes > max_bytes || unique.len() >= 105_000 {
            return Err("Resource plan exceeds the explicit cache preparation budget".into());
        }
        unique.insert(key, artifact.clone());
    }
    Ok(unique)
}

/// The byte budget covers unique resources, including existing cache hits. No network
/// retry is automatic. A repeated call revalidates and reuses already committed files.
pub fn vanilla(
    cache: &Cache,
    metadata: &Value,
    platform: &Platform,
    max_bytes: u64,
    transport: &impl Transport,
    cancel: &Cancellation,
    mut progress: impl FnMut(Progress),
) -> Result<Prepared> {
    cancel.check()?;
    let plan = VersionPlan::parse(metadata, platform)?;
    resources(plan.files.iter().map(|f| &f.artifact), max_bytes)?;
    let index = &plan
        .files
        .iter()
        .find(|f| f.role == "asset_index")
        .ok_or("Missing planned asset index")?
        .artifact;
    if index.size > 8 * 1024 * 1024 {
        return Err("Asset index exceeds 8 MiB".into());
    }
    let downloaded = cache.fetch(index, transport, cancel, |resource| {
        progress(Progress {
            phase: Phase::AssetIndex,
            verified_files: 0,
            total_files: None,
            resource: Some(resource),
        })
    })?;
    let assets = AssetPlan::parse(&cache.read_json(index, cancel)?)?;
    cancel.check()?;
    // Fail before downloading any remaining resources if the index introduces bad
    // mappings, conflicting descriptors or more bytes than the caller approved.
    let all = resources(
        plan.files.iter().chain(&assets.files).map(|f| &f.artifact),
        max_bytes,
    )?;
    let total = all.len();
    let index_key = index.key()?;
    let mut result = Prepared {
        version: plan.version,
        verified_files: 1,
        verified_bytes: index.size,
        cache_hits: usize::from(downloaded.cache_hit),
        logical_files: plan.files.len() + assets.files.len(),
        asset_mappings: assets.mappings.len(),
    };
    for (key, artifact) in all {
        cancel.check()?;
        if key == index_key {
            continue;
        }
        let downloaded = cache.fetch(&artifact, transport, cancel, |resource| {
            progress(Progress {
                phase: Phase::Resources,
                verified_files: result.verified_files,
                total_files: Some(total),
                resource: Some(resource),
            })
        })?;
        result.verified_files += 1;
        result.verified_bytes += downloaded.size;
        result.cache_hits += usize::from(downloaded.cache_hit);
    }
    cancel.check()?;
    progress(Progress {
        phase: Phase::Complete,
        verified_files: total,
        total_files: Some(total),
        resource: None,
    });
    Ok(result)
}
