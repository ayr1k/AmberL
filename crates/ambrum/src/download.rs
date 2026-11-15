// SPDX-License-Identifier: MPL-2.0
//! Verified app-cache downloads. No game-directory writes, archive extraction or auth.
mod batch;
pub(crate) mod windows;
use crate::{
    Result,
    registry::{DirectoryGuard, info, new_id, options},
    task::Cancellation,
};
pub use batch::{BatchProgress, BatchResult};
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
    pub(crate) fn key(&self) -> Result<String> {
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
    /// Production anonymous downloads can provide independent worker transports.
    fn worker(&self) -> Option<Box<dyn Transport + Send>> {
        None
    }
    fn retries(&self) -> usize {
        0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Phase {
    CheckingCache,
    Retrying,
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
    pub(crate) fn root(&self) -> &Path {
        &self.directory.root
    }

    /// Pin a resource against concurrent writes/deletion and verify its actual bytes.
    /// The returned handle, not a remembered cache pathname, is the copy source.
    pub(crate) fn verified_file(
        &self,
        artifact: &Artifact,
        cancel: &Cancellation,
    ) -> Result<fs::File> {
        use std::io::{Seek, SeekFrom};
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
        cancel.check()?;
        let mut file = options()
            .share_mode(FILE_SHARE_READ)
            .open(self.directory.root.join(artifact.key()?))
            .map_err(|_| "Cache resource missing or busy; prepare it first")?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if info(&file)?.nNumberOfLinks != 1
            || !metadata.is_file()
            || metadata.len() != artifact.size
        {
            return Err("Cache resource size/type mismatch".into());
        }
        let mut hash = Hash::new(artifact.algorithm)?;
        let mut buffer = [0; 65536];
        let mut total = 0;
        loop {
            cancel.check()?;
            let n = file.read(&mut buffer).map_err(|_| "Cache read failed")?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > artifact.size {
                return Err("Cache resource size changed".into());
            }
            hash.update(&buffer[..n])?;
        }
        if total != artifact.size || !hash.finish()?.eq_ignore_ascii_case(&artifact.digest) {
            return Err("Cache checksum mismatch; prepare the resource again".into());
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|_| "Cannot rewind cache resource")?;
        Ok(file)
    }
    /// Read a small cached metadata object with a fresh digest check while denying
    /// concurrent writes/deletion. A path returned by fetch is not a trusted snapshot.
    pub fn read_json(
        &self,
        artifact: &Artifact,
        cancel: &Cancellation,
    ) -> Result<serde_json::Value> {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
        cancel.check()?;
        let key = artifact.key()?;
        if artifact.size > 8 * 1024 * 1024 {
            return Err("Cached metadata exceeds 8 MiB".into());
        }
        let mut file = options()
            .share_mode(FILE_SHARE_READ)
            .open(self.directory.root.join(key))
            .map_err(|_| "Cached metadata unavailable or busy")?;
        let metadata = file
            .metadata()
            .map_err(|_| "Cannot inspect cached metadata")?;
        if info(&file)?.nNumberOfLinks != 1
            || !metadata.is_file()
            || metadata.len() != artifact.size
        {
            return Err("Cached metadata size/type mismatch".into());
        }
        let mut bytes = Vec::with_capacity(artifact.size as usize);
        let mut hash = Hash::new(artifact.algorithm)?;
        let mut buffer = [0; 64 * 1024];
        loop {
            cancel.check()?;
            let count = file
                .read(&mut buffer)
                .map_err(|_| "Cannot read cached metadata")?;
            if count == 0 {
                break;
            }
            if bytes.len() as u64 + count as u64 > artifact.size {
                return Err("Cached metadata grew during read".into());
            }
            hash.update(&buffer[..count])?;
            bytes.extend_from_slice(&buffer[..count]);
        }
        cancel.check()?;
        if bytes.len() as u64 != artifact.size
            || !hash.finish()?.eq_ignore_ascii_case(&artifact.digest)
        {
            return Err("Cached metadata checksum mismatch".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "Cached metadata is not valid JSON".into())
    }

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

    /// Bounded retries for transient transport failures only. Verified files are reused.
    /// Commit is the success boundary; cancellation before it never replaces a good file.
    pub fn fetch(
        &self,
        artifact: &Artifact,
        transport: &(impl Transport + ?Sized),
        cancel: &Cancellation,
        mut progress: impl FnMut(Progress),
    ) -> Result<Downloaded> {
        for attempt in 0..=transport.retries().min(2) {
            match self.fetch_once(artifact, transport, cancel, &mut progress) {
                Ok(file) => return Ok(file),
                Err(error) => {
                    cancel.check()?;
                    if attempt == transport.retries().min(2) || !transient(&error) {
                        return Err(error);
                    }
                    progress(Progress {
                        phase: Phase::Retrying,
                        received: 0,
                        total: artifact.size,
                    });
                    backoff(attempt, cancel)?;
                }
            }
        }
        unreachable!()
    }
    fn fetch_once(
        &self,
        artifact: &Artifact,
        transport: &(impl Transport + ?Sized),
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
            let n = response.read(&mut buffer).map_err(|e| {
                format!(
                    "下载中断，未提交文件 / Download interrupted (Windows {})",
                    e.raw_os_error().unwrap_or(0)
                )
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
        if received < artifact.size {
            return Err("下载未完成，已完成的其他文件保留 / Download truncated".into());
        }
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

pub(crate) fn transient(error: &str) -> bool {
    error.ends_with(" / Download truncated")
        || [12002, 12007, 12029, 12030, 12031].iter().any(|code| {
            error.ends_with(&format!("HTTPS failed (Windows {code})"))
                || error.ends_with(&format!("Download interrupted (Windows {code})"))
        })
        || [408, 429, 500, 502, 503, 504]
            .iter()
            .any(|code| error.ends_with(&format!(" / HTTP {code} rejected")))
}
pub(crate) fn backoff(attempt: usize, cancel: &Cancellation) -> Result<()> {
    for _ in 0..(10 * (attempt + 1)) {
        cancel.check()?;
        std::thread::sleep(Duration::from_millis(50));
    }
    cancel.check()
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
