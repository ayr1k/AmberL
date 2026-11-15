// SPDX-License-Identifier: MPL-2.0
//! Explicit installation locations. Registering never changes existing game contents.
use crate::{
    Result,
    registry::{DirectoryGuard, ObjectId, new_id},
    state::Document,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub id: String,
    pub path: PathBuf,
    object: ObjectId,
}
impl Location {
    pub fn destination(&self) -> PathBuf {
        if [
            "versions",
            "instances",
            "saves",
            "libraries",
            "installation.ready.json",
        ]
        .iter()
        .any(|name| self.path.join(name).exists())
        {
            self.path.join("AmberL")
        } else {
            self.path.clone()
        }
    }
    pub fn prepare(&self, app: &Path) -> Result<HeldLocation> {
        let root = checked(app, &self.path)?;
        if root.id != self.object {
            return Err("安装位置已变化，请重新添加此目录".into());
        }
        let path = self.destination();
        if path != root.root {
            create_child(&root, "AmberL")?;
        }
        let target = DirectoryGuard::open(&path)?;
        Ok(HeldLocation {
            path: target.root.clone(),
            _root: root,
            _target: target,
        })
    }
}
pub struct HeldLocation {
    pub path: PathBuf,
    _root: DirectoryGuard,
    _target: DirectoryGuard,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Locations {
    entries: Vec<Location>,
}
fn checked(app: &Path, path: &Path) -> Result<DirectoryGuard> {
    let root = DirectoryGuard::open(path)?;
    let app = DirectoryGuard::open(app)?;
    if root.root.starts_with(&app.root) || app.root.starts_with(&root.root) {
        return Err("安装位置与应用数据目录不能相互包含".into());
    }
    Ok(root)
}
fn create_child(root: &DirectoryGuard, name: &str) -> Result<PathBuf> {
    let path = root.root.join(name);
    match fs::create_dir(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => {
            return Err(format!(
                "无法创建默认游戏目录：{e}；可使用添加游戏库选择其他位置"
            ));
        }
    }
    Ok(DirectoryGuard::open(&path)?.root)
}
pub fn list(app: &Path) -> Result<Vec<Location>> {
    let doc = Document::<Locations>::open(app, "install-locations-v1.json")?;
    if doc.data.entries.len() > 256 {
        return Err("安装位置数量超过上限".into());
    }
    Ok(doc.data.entries)
}
pub fn add(app: &Path, path: &Path) -> Result<()> {
    let root = checked(app, path)?;
    let mut doc = Document::<Locations>::open(app, "install-locations-v1.json")?;
    if let Some(existing) = doc.data.entries.iter().find(|l| l.path == root.root) {
        if existing.object != root.id {
            return Err("目录身份已变化，原登记已保留".into());
        }
        return Ok(());
    }
    if doc.data.entries.len() >= 256 {
        return Err("安装位置数量超过上限".into());
    }
    doc.data.entries.push(Location {
        id: new_id()?,
        path: root.root,
        object: root.id,
    });
    doc.save()
}
pub fn defaults(app: &Path, launcher_root: &Path) -> Result<()> {
    let root = DirectoryGuard::open(launcher_root)?;
    // Validate separation before making even the two empty directories.
    let app_root = DirectoryGuard::open(app)?;
    for name in ["AMBER原生", ".minecraft"] {
        let child = root.root.join(name);
        if child.starts_with(&app_root.root) || app_root.root.starts_with(&child) {
            return Err("默认游戏目录与应用数据目录重叠".into());
        }
        let path = create_child(&root, name)?;
        add(app, &path)?;
    }
    Ok(())
}
