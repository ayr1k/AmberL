// SPDX-License-Identifier: MPL-2.0
//! Verified app-cache downloads. No game-directory writes, archive extraction or auth.
mod windows;
use crate::{
    Result,
    registry::{DirectoryGuard, info, new_id, options},
    task::Cancellation,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use windows::Hash;
pub use windows::Https;
use windows_sys::Win32::Storage::FileSystem::{
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Algorithm {
    Sha1,
    Sha256,
}
impl Algorithm {
    fn name(self) -> &'static str {
        match self {
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
        }
    }
    fn bytes(self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Artifact {
    /// Not included in Debug/errors: provider URLs can contain signed query parameters.
    pub url: String,
    pub size: u64,
    pub algorithm: Algorithm,
    pub digest: String,
}
impl Artifact {
    pub fn validate(&self) -> Result<()> {
        self.key().map(|_| ())
    }
    fn key(&self) -> Result<String> {
        Https::validate(&self.url)?;
        if self.size > 16 * 1024 * 1024 * 1024
            || self.digest.len() != self.algorithm.bytes() * 2
            || !self.digest.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("下载大小或校验值无效 / Invalid artifact size or digest".into());
        }
        Ok(format!(
            "{}-{}",
            self.algorithm.name(),
            self.digest.to_ascii_lowercase()
        ))
    }
}

/// Injectable streaming transport. Production uses platform HTTPS; tests inject failures.
pub trait Transport {
    fn open(&self, url: &str) -> Result<Box<dyn Read>>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Phase {
    CheckingCache,
    Downloading,
    Verifying,
    Complete,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Progress {
    pub phase: Phase,
    pub received: u64,
    pub total: u64,
}
#[derive(Debug, Serialize)]
pub struct Downloaded {
    pub path: PathBuf,
    pub cache_hit: bool,
    pub size: u64,
}

pub struct Cache {
    directory: DirectoryGuard,
}
impl Cache {
    pub fn open(app_directory: &Path) -> Result<Self> {
        let app = DirectoryGuard::open(app_directory)?;
        if app.root.join("versions").exists() || app.root.join("instances").exists() {
            return Err("不能将游戏库作为应用缓存 / Game root is not app data".into());
        }
        let root = app.root.join("downloads-v1");
        match fs::create_dir(&root) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.to_string()),
        }
        Ok(Self {
            directory: DirectoryGuard::open(&root)?,
        })
    }

    /// One attempt. Retry explicitly with the same descriptor after a reported failure.
    /// Commit is the success boundary; cancellation before it never replaces a good file.
    pub fn fetch(
        &self,
        artifact: &Artifact,
        transport: &impl Transport,
        cancel: &Cancellation,
        mut progress: impl FnMut(Progress),
    ) -> Result<Downloaded> {
        cancel.check()?;
        let key = artifact.key()?;
        let destination = self.directory.root.join(&key);
        let lock = options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.directory.root.join(format!("{key}.lock")))
            .map_err(|e| e.to_string())?;
        if info(&lock)?.nNumberOfLinks != 1 {
            return Err("Linked download lock rejected".into());
        }
        lock.try_lock()
            .map_err(|_| "此文件正在下载，请稍后重试 / Artifact busy".to_string())?;
        let started = Instant::now();
        let check = || -> Result<()> {
            cancel.check()?;
            if started.elapsed() > Duration::from_secs(30 * 60) {
                return Err("下载超过本次时间限额，可以重试 / Download deadline exceeded".into());
            }
            Ok(())
        };
        progress(Progress {
            phase: Phase::CheckingCache,
            received: 0,
            total: artifact.size,
        });
        match options().open(&destination) {
            Ok(mut file) => {
                if info(&file)?.nNumberOfLinks != 1
                    || !file.metadata().map_err(|e| e.to_string())?.is_file()
                {
                    return Err("Invalid cache file; preserved".into());
                }
                if file.metadata().map_err(|e| e.to_string())?.len() == artifact.size {
                    let mut hash = Hash::new(artifact.algorithm)?;
                    let mut buf = [0; 64 * 1024];
                    let mut total = 0u64;
                    loop {
                        check()?;
                        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
                        if n == 0 {
                            break;
                        }
                        total += n as u64;
                        if total > artifact.size {
                            break;
                        }
                        hash.update(&buf[..n])?;
                    }
                    if total == artifact.size
                        && hash.finish()?.eq_ignore_ascii_case(&artifact.digest)
                    {
                        check()?;
                        progress(Progress {
                            phase: Phase::Complete,
                            received: total,
                            total,
                        });
                        return Ok(Downloaded {
                            path: destination,
                            cache_hit: true,
                            size: total,
                        });
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        check()?;
        let mut response = transport.open(&artifact.url)?;
        check()?;
        let part = self
            .directory
            .root
            .join(format!("{key}-{}.part", new_id()?));
        let mut temporary = Temporary {
            path: part.clone(),
            file: Some(
                options()
                    .write(true)
                    .create_new(true)
                    .open(&part)
                    .map_err(|e| e.to_string())?,
            ),
        };
        let mut hash = Hash::new(artifact.algorithm)?;
        let mut buffer = [0; 64 * 1024];
        let mut received = 0u64;
        let mut last = Instant::now();
        progress(Progress {
            phase: Phase::Downloading,
            received,
            total: artifact.size,
        });
        loop {
            check()?;
            let n = response.read(&mut buffer).map_err(|_| {
                "下载中断，可重试；未提交文件 / Download interrupted; retry available".to_string()
            })?;
            check()?;
            if n == 0 {
                break;
            }
            received += n as u64;
            if received > artifact.size {
                return Err("下载大于预期，未提交 / Oversized download rejected".into());
            }
            temporary
                .file
                .as_mut()
                .unwrap()
                .write_all(&buffer[..n])
                .map_err(|_| {
                    "无法写入暂存文件，请检查磁盘空间和权限 / Cache write failed".to_string()
                })?;
            hash.update(&buffer[..n])?;
            if last.elapsed() >= Duration::from_millis(100) {
                progress(Progress {
                    phase: Phase::Downloading,
                    received,
                    total: artifact.size,
                });
                last = Instant::now();
            }
        }
        progress(Progress {
            phase: Phase::Verifying,
            received,
            total: artifact.size,
        });
        check()?;
        if received != artifact.size || !hash.finish()?.eq_ignore_ascii_case(&artifact.digest) {
            return Err(
                "大小或校验不符，未提交；可更换来源后重试 / Artifact verification failed".into(),
            );
        }
        temporary
            .file
            .as_ref()
            .unwrap()
            .sync_all()
            .map_err(|e| e.to_string())?;
        check()?;
        temporary.file.take();
        let wide = |p: &Path| {
            p.as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>()
        };
        if unsafe {
            MoveFileExW(
                wide(&part).as_ptr(),
                wide(&destination).as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(
                "无法提交缓存，原文件已保留；请关闭占用后重试 / Cache commit failed".into(),
            );
        }
        progress(Progress {
            phase: Phase::Complete,
            received,
            total: artifact.size,
        });
        Ok(Downloaded {
            path: destination,
            cache_hit: false,
            size: received,
        })
    }
}

struct Temporary {
    path: PathBuf,
    file: Option<fs::File>,
}
impl Drop for Temporary {
    fn drop(&mut self) {
        self.file.take();
        // This path was generated inside a guarded app cache; never a caller-supplied target.
        // Interrupted process residue is ignored on future attempts, never resumed as valid.
        let _ = fs::remove_file(&self.path);
    }
}
