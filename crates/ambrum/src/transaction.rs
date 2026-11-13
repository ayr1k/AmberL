// SPDX-License-Identifier: MPL-2.0
//! New-directory installation only. No replacement, recursive deletion or adoption.
use crate::{
    Result,
    download::{Algorithm, windows::Hash},
    manifest::relative_path,
    paths,
    registry::{DirectoryGuard, ObjectId, info, new_id, options},
    task::Cancellation,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::*;

const PREFIX: &str = ".amberl-install-";
const PENDING: &str = "installation.pending.json";
const READY: &str = "installation.ready.json";
const MAX_FILES: usize = 210_000;
const MAX_JOURNAL: u64 = 64 * 1024 * 1024;

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    volume: u32,
    index: u64,
    created: u64,
}
fn identity(file: &File) -> Result<Identity> {
    let i = info(file)?;
    Ok(Identity {
        volume: i.dwVolumeSerialNumber,
        index: (u64::from(i.nFileIndexHigh) << 32) | u64::from(i.nFileIndexLow),
        created: (u64::from(i.ftCreationTime.dwHighDateTime) << 32)
            | u64::from(i.ftCreationTime.dwLowDateTime),
    })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    schema: u32,
    operation: String,
    target: String,
    parent: ObjectId,
    stage: Identity,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    schema: u32,
    operation: String,
    files: Vec<Record>,
}

pub(crate) struct Destination {
    parent: DirectoryGuard,
    _lock: File,
}
impl Destination {
    pub(crate) fn open(parent: &Path) -> Result<Self> {
        let parent = DirectoryGuard::open(parent)?;
        for name in [
            "versions",
            "instances",
            "saves",
            "libraries",
            "downloads-v1",
        ] {
            if parent
                .root
                .join(name)
                .try_exists()
                .map_err(|e| e.to_string())?
            {
                return Err(
                    "Choose a dedicated installation parent, not an existing game/app root".into(),
                );
            }
        }
        let lock = options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(parent.root.join(".amberl-install.lock"))
            .map_err(|e| e.to_string())?;
        if info(&lock)?.nNumberOfLinks != 1
            || !lock.metadata().map_err(|e| e.to_string())?.is_file()
        {
            return Err("Invalid installation lock; preserved".into());
        }
        lock.try_lock().map_err(|_| "Installation parent is busy")?;
        Ok(Self {
            parent,
            _lock: lock,
        })
    }
    pub(crate) fn root(&self) -> &Path {
        &self.parent.root
    }
    pub(crate) fn check_target(&self, target: &str) -> Result<()> {
        paths::name(target)?;
        if target.starts_with('.') {
            return Err("Installation name cannot start with a dot".into());
        }
        match fs::symlink_metadata(self.parent.root.join(target)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err("Destination already exists or cannot be inspected; never overwritten".into()),
        }
    }
    pub(crate) fn begin(self, target: &str) -> Result<Stage> {
        self.check_target(target)?;
        let operation = new_id()?;
        let root = self.parent.root.join(format!("{PREFIX}{operation}"));
        fs::create_dir(&root).map_err(|e| e.to_string())?;
        let handle = stage_handle(&root)?;
        let pending = Pending {
            schema: 1,
            operation,
            target: target.into(),
            parent: self.parent.id.clone(),
            stage: identity(&handle)?,
        };
        let mut stage = Stage {
            destination: self,
            root,
            handle,
            pending,
            directories: BTreeMap::new(),
            records: vec![],
        };
        let bytes = serde_json::to_vec(&stage.pending).map_err(|e| e.to_string())?;
        let record = stage.write_inner(
            PENDING,
            &mut bytes.as_slice(),
            bytes.len() as u64,
            &Cancellation::default(),
        )?;
        stage.records.push(record);
        Ok(stage)
    }
    pub(crate) fn resume(self, operation: &str) -> Result<Stage> {
        if operation.len() != 32 || !operation.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Invalid installation operation ID".into());
        }
        let root = self.parent.root.join(format!("{PREFIX}{operation}"));
        let handle = stage_handle(&root)?;
        let pending: Pending = read_record(&root.join(PENDING), 4096)?;
        if pending.schema != 1
            || pending.operation != operation
            || pending.parent != self.parent.id
            || pending.stage != identity(&handle)?
        {
            return Err("Installation journal identity mismatch; preserved".into());
        }
        self.check_target(&pending.target)?;
        Ok(Stage {
            destination: self,
            root,
            handle,
            pending,
            directories: BTreeMap::new(),
            records: vec![],
        })
    }
}

fn stage_handle(root: &Path) -> Result<File> {
    let handle = options()
        .access_mode(FILE_GENERIC_READ | DELETE)
        .share_mode(FILE_SHARE_READ)
        .open(root)
        .map_err(|_| "Installation stage is missing, busy or replaced")?;
    if info(&handle)?.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err("Installation stage is not an ordinary directory".into());
    }
    Ok(handle)
}
fn read_record<T: serde::de::DeserializeOwned>(path: &Path, limit: u64) -> Result<T> {
    let file = options()
        .share_mode(FILE_SHARE_READ)
        .open(path)
        .map_err(|_| "Installation journal unavailable")?;
    if info(&file)?.nNumberOfLinks != 1 || !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Invalid installation journal type".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read installation journal")?;
    if bytes.len() as u64 > limit {
        return Err("Installation journal exceeds limit".into());
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "Invalid/incomplete installation journal; preserved".into())
}

pub(crate) struct Stage {
    destination: Destination,
    root: PathBuf,
    handle: File,
    pending: Pending,
    directories: BTreeMap<String, File>,
    records: Vec<Record>,
}
impl Stage {
    pub(crate) fn operation(&self) -> &str {
        &self.pending.operation
    }
    fn directory(&mut self, path: &str, create: bool) -> Result<()> {
        let mut relative = String::new();
        for component in path.split('/') {
            if !relative.is_empty() {
                relative.push('/');
            }
            relative.push_str(component);
            if self.directories.contains_key(&relative) {
                continue;
            }
            let full = self.root.join(&relative);
            if create {
                // Every directory is created by this operation. Existing unexplained
                // paths are conflicts, not permission to adopt their contents.
                fs::create_dir(&full).map_err(|_| "Installation directory conflict")?;
            }
            let handle = options()
                .share_mode(FILE_SHARE_READ)
                .open(full)
                .map_err(|_| "Cannot pin installation directory")?;
            if info(&handle)?.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
                return Err("Installation parent is not a directory".into());
            }
            self.directories.insert(relative.clone(), handle);
        }
        Ok(())
    }
    fn write_inner(
        &mut self,
        path: &str,
        input: &mut impl Read,
        size: u64,
        cancel: &Cancellation,
    ) -> Result<Record> {
        relative_path(path)?;
        if let Some((parent, _)) = path.rsplit_once('/') {
            self.directory(parent, true)?;
        }
        let mut file = options()
            .write(true)
            .create_new(true)
            .share_mode(FILE_SHARE_READ)
            .open(self.root.join(path))
            .map_err(|_| "Installation file conflict; nothing overwritten")?;
        if info(&file)?.nNumberOfLinks != 1 {
            return Err("Linked installation file rejected".into());
        }
        let mut hash = Hash::new(Algorithm::Sha256)?;
        let mut total = 0;
        let mut buffer = [0; 65536];
        loop {
            cancel.check()?;
            let n = input
                .read(&mut buffer)
                .map_err(|_| "Installation source read failed")?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > size {
                return Err("Installation source exceeds planned size".into());
            }
            hash.update(&buffer[..n])?;
            file.write_all(&buffer[..n])
                .map_err(|_| "Installation write failed; stage retained")?;
        }
        if total != size {
            return Err("Installation source is truncated".into());
        }
        file.sync_all()
            .map_err(|_| "Cannot flush staged installation")?;
        Ok(Record {
            path: path.into(),
            bytes: total,
            sha256: hash.finish()?,
        })
    }
    pub(crate) fn write(
        &mut self,
        path: &str,
        input: &mut impl Read,
        size: u64,
        cancel: &Cancellation,
    ) -> Result<()> {
        if path.eq_ignore_ascii_case(PENDING)
            || path.eq_ignore_ascii_case(READY)
            || self.records.len() >= MAX_FILES
        {
            return Err("Reserved installation path or file limit".into());
        }
        let record = self.write_inner(path, input, size, cancel)?;
        self.records.push(record);
        Ok(())
    }
    pub(crate) fn seal(&mut self, cancel: &Cancellation) -> Result<()> {
        cancel.check()?;
        let bytes = serde_json::to_vec(&Ready {
            schema: 1,
            operation: self.pending.operation.clone(),
            files: self.records.clone(),
        })
        .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_JOURNAL {
            return Err("Installation journal exceeds limit".into());
        }
        self.write_inner(READY, &mut bytes.as_slice(), bytes.len() as u64, cancel)?;
        Ok(())
    }
    pub(crate) fn commit(mut self, cancel: &Cancellation) -> Result<PathBuf> {
        cancel.check()?;
        let ready: Ready = read_record(&self.root.join(READY), MAX_JOURNAL)?;
        if ready.schema != 1
            || ready.operation != self.pending.operation
            || ready.files.is_empty()
            || ready.files.len() > MAX_FILES
        {
            return Err("Invalid complete-installation journal; preserved".into());
        }
        let mut expected = BTreeSet::from([READY.to_owned()]);
        let mut case_names = BTreeSet::from([READY.to_owned()]);
        for record in &ready.files {
            relative_path(&record.path)?;
            if record.bytes > 16 * 1024 * 1024 * 1024
                || record.sha256.len() != 64
                || !record.sha256.bytes().all(|b| b.is_ascii_hexdigit())
                || !expected.insert(record.path.clone())
                || !case_names.insert(record.path.to_lowercase())
            {
                return Err("Conflicting or invalid installation journal".into());
            }
            if let Some((parent, _)) = record.path.rsplit_once('/') {
                self.directory(parent, false)?;
            }
            let mut file = options()
                .share_mode(FILE_SHARE_READ)
                .open(self.root.join(&record.path))
                .map_err(|_| "Staged resource missing or busy")?;
            let meta = file.metadata().map_err(|e| e.to_string())?;
            if info(&file)?.nNumberOfLinks != 1 || !meta.is_file() || meta.len() != record.bytes {
                return Err("Staged resource type/size mismatch; preserved".into());
            }
            let mut hash = Hash::new(Algorithm::Sha256)?;
            let mut buffer = [0; 65536];
            let mut total = 0;
            loop {
                cancel.check()?;
                let n = file
                    .read(&mut buffer)
                    .map_err(|_| "Cannot verify staged resource")?;
                if n == 0 {
                    break;
                }
                total += n as u64;
                if total > record.bytes {
                    return Err("Staged resource grew".into());
                }
                hash.update(&buffer[..n])?;
            }
            if total != record.bytes || !hash.finish()?.eq_ignore_ascii_case(&record.sha256) {
                return Err("Staged resource checksum mismatch; preserved".into());
            }
        }
        if !expected.contains(PENDING) {
            return Err("Missing pending journal record".into());
        }
        // Reject additions, links and unexpected directories, including on crash recovery.
        let mut parents = vec![String::new()];
        parents.extend(self.directories.keys().cloned());
        let mut count = 0;
        for relative in parents {
            for entry in fs::read_dir(self.root.join(&relative)).map_err(|e| e.to_string())? {
                cancel.check()?;
                let entry = entry.map_err(|e| e.to_string())?;
                count += 1;
                if count > MAX_FILES * 8 {
                    return Err("Staged directory entry limit".into());
                }
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| "Non-UTF8 staged filename")?;
                let path = if relative.is_empty() {
                    name
                } else {
                    format!("{relative}/{name}")
                };
                if !expected.contains(&path) && !self.directories.contains_key(&path) {
                    return Err("Unexpected staged content; preserved for inspection".into());
                }
            }
        }
        let pending: Pending = read_record(&self.root.join(PENDING), 4096)?;
        if pending.parent != self.destination.parent.id
            || pending.stage != identity(&self.handle)?
            || pending.operation != self.pending.operation
            || pending.target != self.pending.target
            || pending.schema != 1
        {
            return Err("Installation identity changed".into());
        }
        self.destination.check_target(&self.pending.target)?;
        cancel.check()?;
        self.directories.clear();
        let destination = self.destination.parent.root.join(&self.pending.target);
        rename_handle(&self.handle, &destination)?;
        Ok(destination)
    }
}

/// Rename the pinned directory itself; never release it and rename an unchecked path.
/// ReplaceIfExists stays FALSE, so the final namespace operation also detects conflicts.
fn rename_handle(file: &File, destination: &Path) -> Result<()> {
    let name: Vec<u16> = destination.as_os_str().encode_wide().collect();
    let offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let size = (offset + (name.len() + 1) * 2).max(size_of::<FILE_RENAME_INFO>());
    let mut storage = vec![0usize; size.div_ceil(size_of::<usize>())];
    let p = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    unsafe {
        (*p).FileNameLength = (name.len() * 2) as u32;
        std::ptr::copy_nonoverlapping(name.as_ptr(), (*p).FileName.as_mut_ptr(), name.len());
        if SetFileInformationByHandle(file.as_raw_handle(), FileRenameInfo, p.cast(), size as u32)
            == 0
        {
            return Err(format!(
                "Installation commit failed; recoverable stage retained (Windows {})",
                std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
            ));
        }
    }
    Ok(())
}

// Stages intentionally survive failure/drop. Incomplete stages are never reported
// installed or automatically deleted. Only a complete, reverified stage can recover.
pub(crate) fn recover(parent: &Path, operation: &str, cancel: &Cancellation) -> Result<PathBuf> {
    Destination::open(parent)?.resume(operation)?.commit(cancel)
}

mod installed;
pub(crate) use installed::Installed;
