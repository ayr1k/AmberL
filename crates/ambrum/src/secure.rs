// SPDX-License-Identifier: MPL-2.0
//! User-scoped Windows DPAPI. Only encrypted bytes ever reach a persistent file.
//! No plaintext fallback, imported accounts, machine-wide key or logged credentials.
use crate::{
    APPLICATION_ID, Result,
    auth::{self, Authorization, Identity, Profile, Secret, Transport},
    registry::{DirectoryGuard, info, new_id, options},
    task::Cancellation,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::LocalFree, Security::Cryptography::*, Storage::FileSystem::*,
};
use zeroize::{Zeroize, Zeroizing};
const MAX_STORE: u64 = 1024 * 1024;
const FILE: &str = "accounts-v1.dpapi";
const ENTROPY: &[u8] = b"io.github.ayr1k.amberl/accounts-v1";

pub fn check_available() -> Result<()> {
    let probe = Zeroizing::new(new_id()?.into_bytes());
    let encrypted = protect(&probe, false)?;
    if *protect(&encrypted, true)? != *probe {
        return Err("Windows secure storage roundtrip failed".into());
    }
    Ok(())
}

fn protect(input: &[u8], decrypt: bool) -> Result<Zeroizing<Vec<u8>>> {
    protect_with_entropy(input, decrypt, ENTROPY)
}
pub(crate) fn protect_frp(input: &[u8], decrypt: bool) -> Result<Zeroizing<Vec<u8>>> {
    protect_with_entropy(input, decrypt, b"io.github.ayr1k.amberl/frp-v1")
}
fn protect_with_entropy(input: &[u8], decrypt: bool, entropy: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    if input.len() as u64 > MAX_STORE {
        return Err("Credential store exceeds limit".into());
    }
    let data = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr().cast_mut(),
    };
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: entropy.len() as u32,
        pbData: entropy.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let ok = unsafe {
        if decrypt {
            CryptUnprotectData(
                &data,
                null_mut(),
                &entropy,
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptProtectData(
                &data,
                null(),
                &entropy,
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err("Windows 安全存储不可用或数据损坏；原文件已保留 / Windows secure storage unavailable or damaged; preserved".into());
    }
    unsafe {
        let data = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let result = if data.len() as u64 > MAX_STORE {
            Err("Protected credential size limit".into())
        } else {
            Ok(Zeroizing::new(data.to_vec()))
        };
        data.zeroize();
        LocalFree(output.pbData.cast());
        result
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Account {
    profile: Profile,
    refresh: Secret,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    schema: u32,
    application: String,
    accounts: Vec<Account>,
}
pub struct Store {
    directory: DirectoryGuard,
    _lock: File,
    data: Data,
}
impl Store {
    pub fn open(directory: &Path) -> Result<Self> {
        let directory = DirectoryGuard::open(directory)?;
        if directory.root.join("versions").exists()
            || directory.root.join("instances").exists()
            || directory.root.join("installation.ready.json").exists()
        {
            return Err("Account data cannot be stored in a game library".into());
        }
        let lock = options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.root.join("accounts.lock"))
            .map_err(|_| "Cannot lock account store")?;
        if info(&lock)?.nNumberOfLinks != 1
            || !lock
                .metadata()
                .map_err(|_| "Cannot inspect account lock")?
                .is_file()
        {
            return Err("Linked/invalid account lock rejected".into());
        }
        lock.try_lock()
            .map_err(|_| "账户正被另一操作使用 / Account store busy")?;
        let data = match options()
            .share_mode(FILE_SHARE_READ)
            .open(directory.root.join(FILE))
        {
            Ok(file) => {
                if info(&file)?.nNumberOfLinks != 1
                    || !file
                        .metadata()
                        .map_err(|_| "Cannot inspect protected account store")?
                        .is_file()
                {
                    return Err("Invalid account store file; preserved".into());
                }
                let mut bytes = Zeroizing::new(Vec::new());
                file.take(MAX_STORE + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "Cannot read protected accounts")?;
                let plaintext = protect(&bytes, true)?;
                serde_json::from_slice::<Data>(&plaintext)
                    .map_err(|_| "Invalid account schema; preserved")?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Data {
                schema: 1,
                application: APPLICATION_ID.into(),
                accounts: vec![],
            },
            Err(_) => return Err("Account store unavailable or busy; preserved".into()),
        };
        if data.schema != 1 || data.application != APPLICATION_ID || data.accounts.len() > 16 {
            return Err("Unsupported account store schema; preserved".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for a in &data.accounts {
            a.profile.validate()?;
            Secret::new(a.refresh.expose().into())?;
            if !ids.insert(&a.profile.id) {
                return Err("Duplicate account identity; preserved".into());
            }
        }
        Ok(Self {
            directory,
            _lock: lock,
            data,
        })
    }
    pub fn profiles(&self) -> Vec<Profile> {
        self.data
            .accounts
            .iter()
            .map(|a| a.profile.clone())
            .collect()
    }
    pub fn save_authorization(&mut self, authorized: Authorization) -> Result<Profile> {
        if !authorized.identity.is_current() {
            return Err("Authentication expired before secure save".into());
        }
        let profile = authorized.identity.profile.clone();
        profile.validate()?;
        let account = Account {
            profile: profile.clone(),
            refresh: authorized.refresh,
        };
        let mut next = self.data.clone();
        if let Some(existing) = next
            .accounts
            .iter_mut()
            .find(|a| a.profile.id == profile.id)
        {
            *existing = account;
        } else {
            if next.accounts.len() >= 16 {
                return Err("Account limit reached".into());
            }
            next.accounts.push(account);
        }
        self.save(next)?;
        Ok(profile)
    }
    pub fn forget(&mut self, id: &str) -> Result<()> {
        let mut next = self.data.clone();
        let before = next.accounts.len();
        next.accounts.retain(|a| a.profile.id != id);
        if next.accounts.len() == before {
            return Err("Unknown account ID; no account was changed".into());
        }
        self.save(next)
    }
    /// Never grants offline ownership. Rotated OAuth refresh material is protected
    /// before downstream Xbox/Minecraft requests, even if those services then fail.
    pub fn authenticate(
        &mut self,
        id: &str,
        transport: &impl Transport,
        cancel: &Cancellation,
    ) -> Result<Identity> {
        cancel.check()?;
        let index = self
            .data
            .accounts
            .iter()
            .position(|a| a.profile.id == id)
            .ok_or("Selected account no longer exists; sign in explicitly")?;
        let (access, refresh) =
            auth::refresh(&self.data.accounts[index].refresh, transport, cancel)?;
        let mut next = self.data.clone();
        next.accounts[index].refresh = refresh;
        self.save(next)?;
        let identity = auth::minecraft(&access, transport, cancel)?;
        if identity.profile.id != id {
            return Err("刷新后的角色发生变化，请重新登录 / Refreshed account identity changed; sign in again".into());
        }
        let mut next = self.data.clone();
        next.accounts[index].profile = identity.profile.clone();
        self.save(next)?;
        cancel.check()?;
        Ok(identity)
    }
    fn save(&mut self, next: Data) -> Result<()> {
        let plain = Zeroizing::new(
            serde_json::to_vec(&next).map_err(|_| "Cannot encode protected account record")?,
        );
        let cipher = protect(&plain, false)?;
        let path = self
            .directory
            .root
            .join(format!("accounts-{}.encrypted-part", new_id()?));
        let file = options()
            .write(true)
            .access_mode(FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE)
            .create_new(true)
            .share_mode(FILE_SHARE_READ)
            .open(&path)
            .map_err(|_| "Cannot stage protected account record")?;
        let mut temporary = EncryptedStage {
            path,
            file: Some(file),
        };
        let file = temporary.file.as_mut().unwrap();
        file.write_all(&cipher)
            .map_err(|_| "Cannot write protected account record")?;
        file.sync_all()
            .map_err(|_| "Cannot flush protected account record")?;
        // Replacing a directory/reparse/hardlink is not a configuration recovery policy.
        let destination = self.directory.root.join(FILE);
        match options().share_mode(FILE_SHARE_READ).open(&destination) {
            Ok(existing) => {
                if info(&existing)?.nNumberOfLinks != 1
                    || !existing
                        .metadata()
                        .map_err(|_| "Cannot inspect account destination")?
                        .is_file()
                {
                    return Err("Invalid account destination; preserved".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("Account destination busy; previous record preserved".into()),
        }
        rename_file(file, &destination)?;
        self.data = next;
        Ok(())
    }
}
struct EncryptedStage {
    path: PathBuf,
    file: Option<File>,
}
impl Drop for EncryptedStage {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.path);
    }
}
fn rename_file(file: &File, path: &Path) -> Result<()> {
    let name: Vec<u16> = path.as_os_str().encode_wide().collect();
    let size = (std::mem::offset_of!(FILE_RENAME_INFO, FileName) + (name.len() + 1) * 2)
        .max(size_of::<FILE_RENAME_INFO>());
    let mut storage = vec![0usize; size.div_ceil(size_of::<usize>())];
    let ptr = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    unsafe {
        (*ptr).Anonymous.ReplaceIfExists = true;
        (*ptr).FileNameLength = (name.len() * 2) as u32;
        std::ptr::copy_nonoverlapping(name.as_ptr(), (*ptr).FileName.as_mut_ptr(), name.len());
        if SetFileInformationByHandle(
            file.as_raw_handle(),
            FileRenameInfo,
            ptr.cast(),
            size as u32,
        ) == 0
        {
            return Err("Secure save commit failed; previous record preserved".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
