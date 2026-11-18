// SPDX-License-Identifier: MPL-2.0
//! Windows app-owned registry. Attaching/detaching never writes a game library.
//! Directory guards prevent renames/reparse replacement while held. They do not
//! lock game contents against third-party launchers or authorize game mutations.
use crate::{
    Result,
    library::{Library, LibraryId, Scan},
    paths,
    task::Cancellation,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
};
use windows_sys::Win32::{Storage::FileSystem::*, System::Com::CoCreateGuid};

const LIMIT: u64 = 2 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct ObjectId {
    volume: u32,
    index: u64,
    created: u64,
}

pub(crate) fn info(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err("不接入链接或重解析路径 / Reparse point rejected".into());
    }
    Ok(info)
}

pub(crate) fn options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
    options
}

pub(crate) struct DirectoryGuard {
    pub(crate) root: PathBuf,
    pub(crate) id: ObjectId,
    _handles: Vec<File>,
}
impl DirectoryGuard {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        // Do not canonicalize away a junction before validating the original path.
        let root = paths::ordinary(path)?;
        let mut handles = Vec::new();
        for ancestor in root.ancestors().collect::<Vec<_>>().into_iter().rev() {
            let file = options().open(ancestor).map_err(|e| e.to_string())?;
            let data = info(&file)?;
            if data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
                return Err("目标不是文件夹 / Not a directory".into());
            }
            handles.push(file);
        }
        let data = info(handles.last().ok_or("Missing directory handle")?)?;
        let id = ObjectId {
            volume: data.dwVolumeSerialNumber,
            index: (u64::from(data.nFileIndexHigh) << 32) | u64::from(data.nFileIndexLow),
            created: (u64::from(data.ftCreationTime.dwHighDateTime) << 32)
                | u64::from(data.ftCreationTime.dwLowDateTime),
        };
        if id.index == 0 {
            return Err("文件系统没有可靠目录标识 / Missing file identity".into());
        }
        Ok(Self {
            root,
            id,
            _handles: handles,
        })
    }
}

pub(crate) fn new_id() -> Result<String> {
    let mut guid = windows_sys::core::GUID::default();
    if unsafe { CoCreateGuid(&mut guid) } < 0 {
        return Err("Cannot create library ID".into());
    }
    Ok(format!(
        "{:08x}{:04x}{:04x}{}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub label: String,
    pub path: PathBuf,
    object: ObjectId,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    schema: u32,
    entries: Vec<Entry>,
}

pub struct Registry {
    directory: DirectoryGuard,
    _lock: File,
    data: Data,
}

/// Existing parent is required. Creates only AmberL's own child directory.
pub fn default_directory() -> Result<PathBuf> {
    let parent = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA unavailable")?;
    let guard = DirectoryGuard::open(Path::new(&parent))?;
    let directory = guard.root.join(crate::APPLICATION_ID);
    match fs::create_dir(&directory) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }
    DirectoryGuard::open(&directory)?;
    Ok(directory)
}

impl Registry {
    /// Explicit existing app directory, useful for portable tests. Never pass a game root.
    pub fn open(directory: &Path) -> Result<Self> {
        let directory = DirectoryGuard::open(directory)?;
        if directory.root.join("versions").exists() || directory.root.join("instances").exists() {
            return Err("配置目录不能是游戏库 / App directory looks like a game library".into());
        }
        // No delete sharing: another process cannot replace the lock file while held.
        let lock = options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.root.join("libraries.lock"))
            .map_err(|e| e.to_string())?;
        if info(&lock)?.nNumberOfLinks != 1 {
            return Err("Linked registry lock rejected".into());
        }
        lock.try_lock()
            .map_err(|_| "库列表正被另一操作使用，请稍后重试 / Registry busy".to_string())?;
        let path = directory.root.join("libraries.json");
        let data = match options().open(&path) {
            Ok(file) => {
                let metadata = info(&file)?;
                if metadata.nNumberOfLinks != 1
                    || metadata.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0
                {
                    return Err("Invalid registry file".into());
                }
                let mut bytes = Vec::new();
                file.take(LIMIT + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if bytes.len() as u64 > LIMIT {
                    return Err("Registry too large".into());
                }
                let data: Data = serde_json::from_slice(&bytes).map_err(|_| {
                    "库列表损坏，已保留原文件；请从备份恢复 / Invalid registry; preserved"
                        .to_string()
                })?;
                if data.schema != 1 || data.entries.len() > 256 {
                    return Err("Unsupported registry schema or size; preserved".into());
                }
                let mut ids = std::collections::BTreeSet::new();
                for entry in &data.entries {
                    if entry.id.len() != 32
                        || !entry.id.bytes().all(|b| b.is_ascii_hexdigit())
                        || !ids.insert(&entry.id)
                        || !entry.path.is_absolute()
                        || entry.label.is_empty()
                        || entry.label.len() > 512
                    {
                        return Err("Invalid registry entry; preserved".into());
                    }
                }
                data
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Data {
                schema: 1,
                entries: vec![],
            },
            Err(e) => return Err(e.to_string()),
        };
        Ok(Self {
            directory,
            _lock: lock,
            data,
        })
    }

    pub fn entries(&self) -> &[Entry] {
        &self.data.entries
    }

    pub fn attach(&mut self, path: &Path) -> Result<Entry> {
        let guard = DirectoryGuard::open(path)?;
        Library::open(&guard.root)?;
        if guard.root.starts_with(&self.directory.root)
            || self.directory.root.starts_with(&guard.root)
        {
            return Err("应用配置与游戏库不能相互包含 / App data and game library overlap".into());
        }
        if let Some(entry) = self.data.entries.iter().find(|e| e.object == guard.id) {
            if entry.path != guard.root {
                return Err("库位置已改变；请移除旧登记后重新添加 / Library moved".into());
            }
            return Ok(entry.clone());
        }
        if self.data.entries.iter().any(|e| e.path == guard.root) {
            return Err(
                "此路径的目录身份已改变；原登记已保留 / Directory replaced; registration preserved"
                    .into(),
            );
        }
        if self.data.entries.len() >= 256 {
            return Err("Library registry limit reached".into());
        }
        let entry = Entry {
            id: new_id()?,
            label: guard
                .root
                .file_name()
                .unwrap_or(std::ffi::OsStr::new("Game library"))
                .to_string_lossy()
                .chars()
                .take(120)
                .collect(),
            path: guard.root.clone(),
            object: guard.id.clone(),
        };
        let mut next = self.data.clone();
        next.entries.push(entry.clone());
        self.save(next)?;
        Ok(entry)
    }

    /// Detach only the app's registration. Never delete game files, including offline libraries.
    pub fn detach(&mut self, id: &str) -> Result<()> {
        let mut next = self.data.clone();
        let index = next
            .entries
            .iter()
            .position(|e| e.id == id)
            .ok_or("Unknown library ID")?;
        next.entries.remove(index);
        self.save(next)
    }

    /// The registry ID, not the current UI path text, fixes the operation's target.
    pub fn scan(&self, id: &str, cancel: &Cancellation) -> Result<Scan> {
        cancel.check()?;
        let entry = self
            .data
            .entries
            .iter()
            .find(|e| e.id == id)
            .ok_or("Unknown library ID")?;
        let guard = DirectoryGuard::open(&entry.path)?;
        if guard.id != entry.object {
            return Err("目录已被替换；请重新确认库位置 / Directory identity changed".into());
        }
        let mut library = Library::open(&guard.root)?;
        library.id = LibraryId::Registered(entry.id.clone());
        library.scan(cancel)
    }

    fn save(&mut self, next: Data) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&next).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > LIMIT {
            return Err("Registry too large".into());
        }
        let temporary = self
            .directory
            .root
            .join(format!("libraries-{}.tmp", new_id()?));
        let mut file = options()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        let wide = |p: &Path| {
            p.as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>()
        };
        let source = wide(&temporary);
        let target = wide(&self.directory.root.join("libraries.json"));
        // Same-directory replacement. Failure leaves the previous registry untouched;
        // uncommitted .tmp files are ignored, never mistaken for committed state.
        if unsafe {
            MoveFileExW(
                source.as_ptr(),
                target.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(format!(
                "保存失败，原列表保留 / Registry not committed: {}",
                std::io::Error::last_os_error()
            ));
        }
        self.data = next;
        Ok(())
    }
}
