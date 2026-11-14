// SPDX-License-Identifier: MPL-2.0
use crate::Result;
use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

pub const MAX_METADATA: u64 = 8 * 1024 * 1024;

/// Read-only milestone: reject reparse points on every ancestor, including junctions.
/// This is NOT a race-free write capability. Future mutations require handle-based checks.
pub fn ordinary(path: &Path) -> Result<PathBuf> {
    let full = std::path::absolute(path).map_err(|e| e.to_string())?;
    if full.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("路径包含上级跳转 / Parent traversal".into());
    }
    for ancestor in full.ancestors() {
        let m = fs::symlink_metadata(ancestor)
            .map_err(|e| format!("无法读取路径 / Cannot inspect path: {e}"))?;
        if m.file_type().is_symlink() {
            return Err("暂不接入链接目录 / Linked path not supported".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if m.file_attributes() & 0x400 != 0 {
                return Err("暂不接入重解析目录 / Reparse point not supported".into());
            }
        }
    }
    fs::canonicalize(full).map_err(|e| e.to_string())
}

pub fn contained(root: &Path, path: &Path) -> Result<PathBuf> {
    let actual = ordinary(path)?;
    if !actual.starts_with(root) {
        return Err("路径超出当前库 / Path outside library".into());
    }
    Ok(actual)
}

pub fn name(value: &str) -> Result<()> {
    let stem = value.split('.').next().unwrap_or("").to_ascii_lowercase();
    let reserved = matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || ["com", "lpt"].iter().any(|p| {
            stem.strip_prefix(p)
                .is_some_and(|s| s.len() == 1 && matches!(s.as_bytes()[0], b'1'..=b'9'))
        });
    if value.is_empty()
        || value.len() > 240
        || value == "."
        || value == ".."
        || value.ends_with(['.', ' '])
        || value
            .chars()
            .any(|c| c.is_control() || "\\/:*?\"<>|".contains(c))
        || reserved
    {
        Err("无效的版本标识 / Invalid version identifier".into())
    } else {
        Ok(())
    }
}

pub fn text(root: &Path, path: &Path, limit: u64) -> Result<String> {
    let actual = contained(root, path)?;
    let file = fs::File::open(actual).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err("配置文件类型或大小无效 / Invalid metadata file".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("配置文件过大 / Metadata too large".into());
    }
    String::from_utf8(bytes).map_err(|_| "配置不是 UTF-8 / Metadata is not UTF-8".into())
}

pub fn json(root: &Path, path: &Path) -> Result<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(&text(root, path, MAX_METADATA)?)
        .map_err(|_| "配置 JSON 无效 / Invalid JSON".to_string())?;
    if !value.is_object() {
        return Err("配置必须是对象 / Expected metadata object".into());
    }
    Ok(value)
}

pub fn optional_json(root: &Path, path: &Path) -> Result<Option<serde_json::Value>> {
    match fs::symlink_metadata(path) {
        Ok(_) => json(root, path).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
