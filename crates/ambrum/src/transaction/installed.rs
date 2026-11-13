// SPDX-License-Identifier: MPL-2.0
use super::*;

/// Pins immutable installation resources for the whole owned game session.
/// Gameplay files may be added after installation; they never become executable inputs.
pub(crate) struct Installed {
    pub(crate) root: PathBuf,
    _root: DirectoryGuard,
    _lock: File,
    _directories: Vec<DirectoryGuard>,
    files: BTreeMap<String, File>,
}
impl Installed {
    pub(crate) fn open(path: &Path, cancel: &Cancellation) -> Result<Self> {
        let root = DirectoryGuard::open(path)?;
        let parent = DirectoryGuard::open(root.root.parent().ok_or("Missing instance parent")?)?;
        // Pin journals before decoding so replacing them cannot change the checked identity.
        let mut files = BTreeMap::new();
        for name in [PENDING, READY] {
            let file = options()
                .share_mode(FILE_SHARE_READ)
                .open(root.root.join(name))
                .map_err(|_| "A complete AmberL installation is required")?;
            if info(&file)?.nNumberOfLinks != 1
                || !file.metadata().map_err(|e| e.to_string())?.is_file()
            {
                return Err("Invalid installation receipt".into());
            }
            files.insert(name.into(), file);
        }
        let pending: Pending = read_record(&root.root.join(PENDING), 4096)?;
        let ready: Ready = read_record(&root.root.join(READY), MAX_JOURNAL)?;
        let handle = options().open(&root.root).map_err(|e| e.to_string())?;
        if pending.schema != 1
            || ready.schema != 1
            || pending.operation != ready.operation
            || pending.parent != parent.id
            || pending.stage != identity(&handle)?
            || root.root.file_name().and_then(|n| n.to_str()) != Some(&pending.target)
            || ready.files.is_empty()
            || ready.files.len() > MAX_FILES
        {
            return Err("Installed instance identity mismatch; no game started".into());
        }
        let lock = options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.root.join(".amberl-session.lock"))
            .map_err(|_| "Cannot open session lock")?;
        if info(&lock)?.nNumberOfLinks != 1
            || !lock.metadata().map_err(|e| e.to_string())?.is_file()
        {
            return Err("Invalid session lock".into());
        }
        lock.try_lock()
            .map_err(|_| "This instance already has an active AmberL session")?;
        let mut seen = BTreeSet::new();
        let mut directories = BTreeMap::new();
        for record in &ready.files {
            cancel.check()?;
            relative_path(&record.path)?;
            if record.path.eq_ignore_ascii_case(READY)
                || !seen.insert(record.path.to_lowercase())
                || record.bytes > 16 * 1024 * 1024 * 1024
                || record.sha256.len() != 64
            {
                return Err("Invalid installed resource record".into());
            }
            if let Some((parent, _)) = record.path.rsplit_once('/')
                && !directories.contains_key(parent)
            {
                directories.insert(
                    parent.to_owned(),
                    DirectoryGuard::open(&root.root.join(parent))?,
                );
            }
            let mut file = options()
                .share_mode(FILE_SHARE_READ)
                .open(root.root.join(&record.path))
                .map_err(|_| "Installed resource missing or busy")?;
            verify_file(
                &mut file,
                record.bytes,
                Algorithm::Sha256,
                &record.sha256,
                cancel,
            )?;
            files.insert(record.path.clone(), file);
        }
        if !seen.contains(PENDING) {
            return Err("Missing installation identity receipt".into());
        }
        Ok(Self {
            root: root.root.clone(),
            _root: root,
            _lock: lock,
            _directories: directories.into_values().collect(),
            files,
        })
    }
    pub(crate) fn json(&mut self, path: &str) -> Result<serde_json::Value> {
        use std::io::{Seek, SeekFrom};
        let file = self.files.get_mut(path).ok_or("Unrecorded metadata")?;
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut data = Vec::new();
        file.take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() > 8 * 1024 * 1024 {
            return Err("Installed metadata exceeds limit".into());
        }
        serde_json::from_slice(&data).map_err(|_| "Invalid installed metadata".into())
    }
    pub(crate) fn verify_official(
        &mut self,
        plan: &crate::manifest::VersionPlan,
        cancel: &Cancellation,
    ) -> Result<()> {
        for resource in &plan.files {
            let file = self
                .files
                .get_mut(&resource.path)
                .ok_or("Official resource absent from installation")?;
            verify_file(
                file,
                resource.artifact.size,
                resource.artifact.algorithm,
                &resource.artifact.digest,
                cancel,
            )?;
        }
        Ok(())
    }
    /// Check existing mutable game content for links before granting the game a cwd.
    /// This is a local integrity boundary, not a sandbox against another local writer.
    pub(crate) fn check_game_paths(&self, cancel: &Cancellation) -> Result<()> {
        let mut pending = vec![self.root.clone()];
        let mut count = 0;
        while let Some(path) = pending.pop() {
            let guard = DirectoryGuard::open(&path)?;
            for entry in fs::read_dir(&guard.root).map_err(|e| e.to_string())? {
                cancel.check()?;
                count += 1;
                if count > MAX_FILES * 8 {
                    return Err("Game directory entry limit".into());
                }
                let path = entry.map_err(|e| e.to_string())?.path();
                let file = options()
                    .open(&path)
                    .map_err(|_| "Cannot inspect game path")?;
                let data = info(&file)?;
                if data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
                    pending.push(path);
                } else if data.nNumberOfLinks != 1 {
                    return Err("Linked game content rejected".into());
                }
            }
        }
        Ok(())
    }
}
fn verify_file(
    file: &mut File,
    size: u64,
    algorithm: Algorithm,
    expected: &str,
    cancel: &Cancellation,
) -> Result<()> {
    use std::io::{Seek, SeekFrom};
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if info(file)?.nNumberOfLinks != 1 || !meta.is_file() || meta.len() != size {
        return Err("Installed resource type/size mismatch".into());
    }
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut hash = Hash::new(algorithm)?;
    let mut buffer = [0; 65536];
    loop {
        cancel.check()?;
        let n = file
            .read(&mut buffer)
            .map_err(|_| "Cannot verify installed resource")?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n])?;
    }
    if !hash.finish()?.eq_ignore_ascii_case(expected) {
        return Err("Installed resource checksum mismatch; no game started".into());
    }
    Ok(())
}
