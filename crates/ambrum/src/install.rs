// SPDX-License-Identifier: MPL-2.0
//! Materialize verified Vanilla resources into a NEW isolated directory only.
//! Does not authenticate, launch, update existing instances or mutate attached libraries.
use crate::{
    Result,
    download::{Artifact, Cache},
    library::{ClientInstanceId, LibraryId},
    manifest::{AssetPlan, Platform, VersionPlan, relative_path},
    task::Cancellation,
    transaction::{self, Destination},
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Phase {
    Staging,
    Revalidating,
    Complete,
}
#[derive(Clone, Debug, Serialize)]
pub struct Progress {
    pub operation: String,
    pub phase: Phase,
    pub files_written: usize,
    pub total_files: usize,
}
#[derive(Debug, Serialize)]
pub struct Installed {
    pub operation: String,
    pub instance: ClientInstanceId,
    pub game_directory: PathBuf,
    pub version: String,
    pub materialized_files: usize,
    pub materialized_bytes: u64,
    pub extracted_native_files: usize,
    pub launch_verified: bool,
}

struct Layout {
    paths: BTreeMap<String, (String, bool)>,
    bytes: u64,
    limit: u64,
    files: usize,
}
impl Layout {
    fn reserve(&mut self, path: &str, bytes: u64) -> Result<()> {
        relative_path(path)?;
        if path.split('/').count() > 32 {
            return Err("Installation path depth limit".into());
        }
        let mut prefix = String::new();
        let parts: Vec<_> = path.split('/').collect();
        for (index, part) in parts.iter().enumerate() {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let directory = index + 1 < parts.len();
            match self.paths.get(&prefix.to_lowercase()) {
                Some((old, is_dir)) if directory && *is_dir && old == &prefix => {}
                Some(_) => return Err("File/directory or case-colliding installation paths".into()),
                None => {
                    self.paths
                        .insert(prefix.to_lowercase(), (prefix.clone(), directory));
                }
            }
        }
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or("Installation byte overflow")?;
        self.files += 1;
        if self.bytes > self.limit || self.files > 209_000 || self.paths.len() > 400_000 {
            return Err("Installation exceeds explicit materialized-byte/file budget".into());
        }
        Ok(())
    }
}
struct Copy {
    path: String,
    artifact: Artifact,
}
struct Native {
    artifact: Artifact,
    entries: Vec<(usize, String, u64)>,
}

fn archive(
    cache: &Cache,
    artifact: &Artifact,
    cancel: &Cancellation,
) -> Result<zip::ZipArchive<File>> {
    // Bound compressed input before the mature ZIP parser allocates its directory.
    if artifact.size > 16 * 1024 * 1024 {
        return Err("Native archive exceeds 16 MiB prototype limit".into());
    }
    let archive = zip::ZipArchive::new(cache.verified_file(artifact, cancel)?)
        .map_err(|_| "Invalid native ZIP archive")?;
    if archive.len() > 4096 {
        return Err("Native archive entry limit exceeded".into());
    }
    Ok(archive)
}

/// Fail early before downloads; the transaction repeats these checks at commit time.
pub fn check_destination(app: &Path, parent: &Path, name: &str) -> Result<()> {
    crate::paths::name(name)?;
    let app = crate::paths::ordinary(app)?;
    let parent = crate::paths::ordinary(parent)?;
    if parent.starts_with(&app) || app.starts_with(&parent) {
        return Err("安装位置与应用数据目录重叠".into());
    }
    Destination::open(&parent)?.check_target(name)
}

/// Requires all provider resources in Cache first. The explicit budget counts every
/// materialized copy, metadata and expanded native bytes, not just unique downloads.
/// Parent and new_name are captured inputs, never a UI's current selection.
#[allow(clippy::too_many_arguments)]
pub fn vanilla(
    cache: &Cache,
    metadata: &Value,
    platform: &Platform,
    parent: &Path,
    new_name: &str,
    max_bytes: u64,
    cancel: &Cancellation,
    mut progress: impl FnMut(Progress),
) -> Result<Installed> {
    cancel.check()?;
    if max_bytes == 0 {
        return Err("Explicit positive installation byte budget required".into());
    }
    let version = VersionPlan::parse(metadata, platform)?;
    let index = &version
        .files
        .iter()
        .find(|f| f.role == "asset_index")
        .ok_or("Missing asset index")?
        .artifact;
    let assets = AssetPlan::parse(&cache.read_json(index, cancel)?)?;
    // Check containment before creating a lock or stage in the selected parent.
    let parent_path = crate::paths::ordinary(parent)?;
    let app = cache.root().parent().ok_or("Missing app root")?;
    if parent_path.starts_with(app) || app.starts_with(&parent_path) {
        return Err("Installation parent and app/cache directory overlap".into());
    }
    let mut layout = Layout {
        paths: BTreeMap::new(),
        bytes: 0,
        limit: max_bytes,
        files: 0,
    };
    // Reserved sidecars are not game payload; prevent metadata paths from targeting them.
    layout.paths.insert(
        "installation.pending.json".into(),
        ("installation.pending.json".into(), false),
    );
    layout.paths.insert(
        "installation.ready.json".into(),
        ("installation.ready.json".into(), false),
    );
    let mut copies = Vec::new();
    for file in version.files.iter().chain(&assets.files) {
        layout.reserve(&file.path, file.artifact.size)?;
        copies.push(Copy {
            path: file.path.clone(),
            artifact: file.artifact.clone(),
        });
    }
    let by_path: BTreeMap<_, _> = assets
        .files
        .iter()
        .map(|f| (f.path.as_str(), &f.artifact))
        .collect();
    for (logical, path) in &assets.mappings {
        let artifact = by_path.get(path.as_str()).ok_or("Missing asset mapping")?;
        let mut destinations = Vec::new();
        if assets.virtual_layout {
            destinations.push(format!("assets/virtual/{}/{logical}", version.asset_index));
        }
        if assets.map_to_resources {
            destinations.push(format!("resources/{logical}"));
        }
        for path in destinations {
            layout.reserve(&path, artifact.size)?;
            copies.push(Copy {
                path,
                artifact: (*artifact).clone(),
            });
        }
    }
    let metadata_bytes =
        serde_json::to_vec(metadata).map_err(|_| "Cannot encode version metadata")?;
    if metadata_bytes.len() > 8 * 1024 * 1024 {
        return Err("Version metadata exceeds 8 MiB".into());
    }
    let metadata_path = format!("versions/{0}/{0}.json", version.version);
    layout.reserve(&metadata_path, metadata_bytes.len() as u64)?;
    let mut natives = Vec::new();
    let mut native_bytes = 0u64;
    let mut native_files = 0;
    for path in &version.native_archives {
        let artifact = &version
            .files
            .iter()
            .find(|f| &f.path == path)
            .ok_or("Missing native resource")?
            .artifact;
        let mut zip = archive(cache, artifact, cancel)?;
        let mut entries = Vec::new();
        for i in 0..zip.len() {
            cancel.check()?;
            let entry = zip
                .by_index(i)
                .map_err(|_| "Unsupported/corrupt native ZIP entry")?;
            let raw = entry
                .name()
                .map_err(|_| "Invalid ZIP filename encoding")?
                .into_owned();
            relative_path(raw.strip_suffix('/').unwrap_or(&raw))?;
            if entry.enclosed_name().is_none()
                || entry.is_symlink()
                || entry.encrypted()
                || entry
                    .unix_mode()
                    .is_some_and(|m| !matches!(m & 0o170000, 0 | 0o100000 | 0o040000))
            {
                return Err("Unsafe or unsupported native ZIP entry".into());
            }
            if entry.is_dir() || raw.starts_with("META-INF/") {
                continue;
            }
            if !entry.is_file() {
                return Err("Native ZIP entry is not a regular file".into());
            }
            let path = format!("natives/{raw}");
            native_bytes = native_bytes
                .checked_add(entry.size())
                .ok_or("Native size overflow")?;
            if native_bytes > 256 * 1024 * 1024 {
                return Err("Expanded native size limit exceeded".into());
            }
            layout.reserve(&path, entry.size())?;
            entries.push((i, path, entry.size()));
            native_files += 1;
        }
        natives.push(Native {
            artifact: artifact.clone(),
            entries,
        });
    }
    // Validate every cache input before touching the installation parent, then pin
    // and reverify each resource again when its bytes are actually copied.
    for copy in &copies {
        drop(cache.verified_file(&copy.artifact, cancel)?);
    }
    let destination = Destination::open(&parent_path)?;
    destination.check_target(new_name)?;
    let final_path = destination.root().join(new_name);
    let mut stage = destination.begin(new_name)?;
    let operation = stage.operation().to_owned();
    let mut written = 0;
    let emit = |phase, files_written, progress: &mut dyn FnMut(Progress)| {
        progress(Progress {
            operation: operation.clone(),
            phase,
            files_written,
            total_files: layout.files,
        })
    };
    emit(Phase::Staging, 0, &mut progress);
    for copy in copies {
        let mut file = cache.verified_file(&copy.artifact, cancel)?;
        stage.write(&copy.path, &mut file, copy.artifact.size, cancel)?;
        written += 1;
        emit(Phase::Staging, written, &mut progress);
    }
    stage.write(
        &metadata_path,
        &mut metadata_bytes.as_slice(),
        metadata_bytes.len() as u64,
        cancel,
    )?;
    written += 1;
    for native in natives {
        let mut zip = archive(cache, &native.artifact, cancel)?;
        for (index, path, size) in native.entries {
            let mut entry = zip
                .by_index(index)
                .map_err(|_| "Cannot reopen verified native entry")?;
            stage.write(&path, &mut entry, size, cancel)?;
            written += 1;
            emit(Phase::Staging, written, &mut progress);
        }
    }
    stage.seal(cancel)?;
    emit(Phase::Revalidating, written, &mut progress);
    let committed = stage.commit(cancel)?;
    debug_assert_eq!(committed, final_path);
    emit(Phase::Complete, written, &mut progress);
    Ok(Installed {
        operation,
        instance: ClientInstanceId {
            library: LibraryId::Observed(committed.clone()),
            profile: version.version.clone(),
        },
        game_directory: committed,
        version: version.version,
        materialized_files: layout.files,
        materialized_bytes: layout.bytes,
        extracted_native_files: native_files,
        launch_verified: false,
    })
}

/// Explicitly finish an interrupted operation only if its complete journal, identity,
/// every file, and the final destination still validate. Incomplete stages stay intact.
pub fn recover(parent: &Path, operation: &str, cancel: &Cancellation) -> Result<PathBuf> {
    transaction::recover(parent, operation, cancel)
}

#[cfg(test)]
mod tests;
