// SPDX-License-Identifier: MPL-2.0
//! Read-only capture of known Vanilla user data. Executable resources are rebuilt
//! from official metadata; source launcher settings and credentials never enter a copy.
use crate::{
    Result,
    library::{Instance, Isolation, LibraryId, LibraryKind},
    registry::{DirectoryGuard, Registry, info, options},
    task::Cancellation,
};
use std::{
    fs,
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

pub fn eligible(instance: &Instance) -> bool {
    instance.issue.is_none()
        && instance.loader == "Vanilla"
        && matches!(instance.isolation, Isolation::Isolated | Isolation::Shared)
        && instance.game_dir.is_some()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    library: String,
    profile: String,
    version: String,
    directory: PathBuf,
    shared: bool,
}
impl Selection {
    pub fn capture(instance: &Instance) -> Option<Self> {
        let LibraryId::Registered(library) = &instance.id.library else {
            return None;
        };
        eligible(instance).then(|| Self {
            library: library.clone(),
            profile: instance.id.profile.clone(),
            version: instance.game.clone(),
            directory: instance.game_dir.clone().unwrap(),
            shared: instance.isolation == Isolation::Shared,
        })
    }
}

pub struct Source {
    pub(crate) version: String,
    root: DirectoryGuard,
    _directories: Vec<DirectoryGuard>,
    pub(crate) files: Vec<(String, fs::File, u64)>,
}
impl Source {
    pub fn open(
        app: &Path,
        selected: &Selection,
        allow_shared: bool,
        c: &Cancellation,
    ) -> Result<Self> {
        let registry = Registry::open(app)?;
        let scan = registry.scan(&selected.library, c)?;
        if scan.library.kind != LibraryKind::Minecraft {
            return Err("当前只支持 .minecraft 版本目录的独立副本导入".into());
        }
        let instance = scan
            .instances
            .iter()
            .find(|i| i.id.profile == selected.profile)
            .ok_or("导入来源已变化，请重新扫描游戏库")?;
        if Selection::capture(instance).as_ref() != Some(selected) {
            return Err("仅能复制隔离位置已确认的原版实例；请重新扫描并核对版本".into());
        }
        let shared = instance.isolation == Isolation::Shared;
        if shared && !allow_shared {
            return Err("该版本使用共享目录，需明确确认复制其中的全部存档和设置".into());
        }
        let root = DirectoryGuard::open(&scan.library.root)?;
        let game = DirectoryGuard::open(instance.game_dir.as_ref().ok_or("缺少游戏目录")?)?;
        if !game.root.starts_with(&root.root) {
            return Err("游戏数据目录在登记库之外".into());
        }
        let app = crate::paths::ordinary(app)?;
        if root.root.starts_with(&app) || app.starts_with(&root.root) {
            return Err("导入来源不能与应用数据目录重叠".into());
        }
        let mut source = Self {
            version: instance.game.clone(),
            root,
            _directories: vec![],
            files: vec![],
        };
        let mut pending = vec![];
        for name in [
            "saves",
            "resourcepacks",
            "shaderpacks",
            "screenshots",
            "config",
            "options.txt",
            "optionsof.txt",
            "optionsshaders.txt",
            "servers.dat",
        ] {
            let path = game.root.join(name);
            match fs::symlink_metadata(&path) {
                Ok(_) => pending.push(path),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        let mut bytes = 0u64;
        while let Some(path) = pending.pop() {
            c.check()?;
            let path = crate::paths::contained(&game.root, &path)?;
            let relative = path
                .strip_prefix(&game.root)
                .map_err(|_| "数据路径超出来源")?;
            if relative.components().count() > 32
                || source.files.len() + source._directories.len() + pending.len() >= 200_000
            {
                return Err("导入文件数量或路径深度超过限制".into());
            }
            if path.is_dir() {
                let guard = DirectoryGuard::open(&path)?;
                for e in fs::read_dir(&guard.root).map_err(|e| e.to_string())? {
                    c.check()?;
                    if pending.len() + source.files.len() + source._directories.len() >= 200_000 {
                        return Err("导入文件数量超过限制".into());
                    }
                    pending.push(e.map_err(|e| e.to_string())?.path());
                }
                source._directories.push(guard);
            } else {
                let relative = relative.to_string_lossy().replace('\\', "/");
                crate::manifest::relative_path(&relative)?;
                let file = options()
                    .share_mode(FILE_SHARE_READ)
                    .open(&path)
                    .map_err(|_| "无法读取或锁定来源数据，请关闭正在使用它的游戏")?;
                let metadata = file.metadata().map_err(|e| e.to_string())?;
                if !metadata.is_file() || info(&file)?.nNumberOfLinks != 1 {
                    return Err("导入来源包含链接或非普通文件".into());
                }
                bytes = bytes.checked_add(metadata.len()).ok_or("导入大小溢出")?;
                if bytes > 16 * 1024 * 1024 * 1024 {
                    return Err("导入数据超过 16 GiB 限制".into());
                }
                source.files.push((relative, file, metadata.len()));
            }
        }
        source._directories.push(game);
        Ok(source)
    }
    pub fn check_parent(&self, parent: &Path) -> Result<()> {
        let parent = crate::paths::ordinary(parent)?;
        if parent.starts_with(&self.root.root) || self.root.root.starts_with(&parent) {
            return Err("副本位置必须与来源游戏库分离".into());
        }
        Ok(())
    }
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
    pub fn version(&self) -> &str {
        &self.version
    }
}
