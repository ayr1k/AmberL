// SPDX-License-Identifier: MPL-2.0
//! Small app-owned documents: guarded directories, exclusive writer, atomic commit.
use crate::{
    Result,
    registry::{DirectoryGuard, info, new_id, options},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::File,
    io::{Read, Write},
    os::windows::ffi::OsStrExt,
    path::Path,
};
use windows_sys::Win32::Storage::FileSystem::*;

pub(crate) struct Document<T> {
    directory: DirectoryGuard,
    _lock: File,
    name: String,
    pub data: T,
}
impl<T: Serialize + DeserializeOwned + Default> Document<T> {
    pub fn open(app: &Path, name: &str) -> Result<Self> {
        crate::paths::name(name)?;
        let directory = DirectoryGuard::open(app)?;
        if directory.root.join("versions").exists() || directory.root.join("instances").exists() {
            return Err("App directory cannot be a game library".into());
        }
        Self::open_guarded(directory, name)
    }
    /// Caller has established ownership of this directory and holds its operation lock.
    /// Server runtimes may legitimately create a `versions` directory inside their root.
    pub(crate) fn open_owned(directory: &DirectoryGuard, name: &str) -> Result<Self> {
        Self::open_guarded(DirectoryGuard::open(&directory.root)?, name)
    }
    fn open_guarded(directory: DirectoryGuard, name: &str) -> Result<Self> {
        crate::paths::name(name)?;
        let lock = options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.root.join(format!("{name}.lock")))
            .map_err(|e| e.to_string())?;
        if info(&lock)?.nNumberOfLinks != 1 {
            return Err("Linked state lock rejected".into());
        }
        lock.try_lock()
            .map_err(|_| "设置正被另一操作使用，请稍后重试")?;
        let data = match options().open(directory.root.join(name)) {
            Ok(file) => {
                if info(&file)?.nNumberOfLinks != 1
                    || !file.metadata().map_err(|e| e.to_string())?.is_file()
                {
                    return Err("Invalid state file; preserved".into());
                }
                let mut bytes = vec![];
                file.take(2 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if bytes.len() > 2 * 1024 * 1024 {
                    return Err("State size limit; preserved".into());
                }
                serde_json::from_slice(&bytes).map_err(|_| "设置文件损坏，原文件已保留")?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => T::default(),
            Err(e) => return Err(e.to_string()),
        };
        Ok(Self {
            directory,
            _lock: lock,
            name: name.into(),
            data,
        })
    }
    pub fn save(&self) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&self.data).map_err(|e| e.to_string())?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("State size limit".into());
        }
        let temporary = self.directory.root.join(format!("state-{}.tmp", new_id()?));
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
        if unsafe {
            MoveFileExW(
                wide(&temporary).as_ptr(),
                wide(&self.directory.root.join(&self.name)).as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err("设置保存失败，原文件已保留".into());
        }
        Ok(())
    }
}
