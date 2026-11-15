// SPDX-License-Identifier: MPL-2.0
//! Anonymous manual release discovery. No installation, process launch or auto-update.
use crate::{Result, download::Transport, task::Cancellation};
use serde_json::Value;

pub const RELEASES: &str = "https://api.github.com/repos/ayr1k/AmberL/releases?per_page=100";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub page: String,
    pub sha256: String,
    pub bytes: u64,
}

fn version(value: &str) -> Option<[u32; 3]> {
    let parts: Vec<_> = value
        .strip_prefix('v')
        .unwrap_or(value)
        .split('.')
        .collect();
    if parts.len() != 3 {
        return None;
    }
    let mut result = [0; 3];
    for (i, part) in parts.into_iter().enumerate() {
        if part.is_empty()
            || (part.len() > 1 && part.starts_with('0'))
            || !part.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
        result[i] = part.parse().ok()?;
    }
    Some(result)
}

pub fn check(net: &impl Transport, cancel: &Cancellation) -> Result<Option<Release>> {
    let value: Value = serde_json::from_slice(&crate::catalog::bounded(net, RELEASES, cancel)?)
        .map_err(|_| "正式版本列表格式无效")?;
    parse(&value, VERSION)
}

fn parse(value: &Value, current: &str) -> Result<Option<Release>> {
    let current_base =
        version(current.split('-').next().unwrap_or(current)).ok_or("当前版本无效")?;
    let releases = value
        .as_array()
        .filter(|v| v.len() <= 100)
        .ok_or("正式版本列表无效或过长")?;
    let mut best: Option<([u32; 3], Release)> = None;
    for release in releases {
        if release["draft"] != false || release["prerelease"] != false {
            continue;
        }
        let tag = release["tag_name"].as_str().ok_or("缺少发行版本标识")?;
        let Some(numbers) = version(tag) else {
            continue;
        };
        if numbers < current_base || (numbers == current_base && !current.contains('-')) {
            continue;
        }
        let page = format!("https://github.com/ayr1k/AmberL/releases/tag/{tag}");
        if release["html_url"] != page {
            return Err("发行页面不属于 AmberL 官方仓库".into());
        }
        let title = tag.trim_start_matches('v');
        let name = format!("AmberL-{title}-windows-x64.exe");
        let assets = release["assets"]
            .as_array()
            .filter(|v| v.len() <= 100)
            .ok_or("发行文件列表无效")?;
        let matching: Vec<_> = assets.iter().filter(|a| a["name"] == name).collect();
        if matching.len() > 1 {
            return Err("发行文件名称重复".into());
        }
        let Some(asset) = matching.first() else {
            continue;
        };
        if asset["state"] != "uploaded"
            || asset["browser_download_url"]
                != format!("https://github.com/ayr1k/AmberL/releases/download/{tag}/{name}")
        {
            return Err("发行文件来源或状态无效".into());
        }
        let hash = asset["digest"]
            .as_str()
            .and_then(|s| s.strip_prefix("sha256:"))
            .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or("发行文件缺少 SHA-256 校验值")?;
        let bytes = asset["size"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 512 * 1024 * 1024)
            .ok_or("发行文件大小无效")?;
        if best.as_ref().is_none_or(|(number, _)| numbers > *number) {
            best = Some((
                numbers,
                Release {
                    version: title.to_owned(),
                    page,
                    sha256: hash.to_lowercase(),
                    bytes,
                },
            ));
        }
    }
    Ok(best.map(|(_, release)| release))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn release() -> Value {
        json!({"draft":false,"prerelease":false,"tag_name":"v0.1.0","html_url":"https://github.com/ayr1k/AmberL/releases/tag/v0.1.0","assets":[{"name":"AmberL-0.1.0-windows-x64.exe","state":"uploaded","browser_download_url":"https://github.com/ayr1k/AmberL/releases/download/v0.1.0/AmberL-0.1.0-windows-x64.exe","digest":format!("sha256:{}","a".repeat(64)),"size":12345}]})
    }
    #[test]
    fn channel_version_and_product_boundaries() {
        let mut r = release();
        assert!(parse(&json!([r]), "0.1.0-dev.1").unwrap().is_some());
        assert!(parse(&json!([r]), "0.1.0").unwrap().is_none());
        assert!(parse(&json!([r]), "0.2.0").unwrap().is_none());
        r["prerelease"] = json!(true);
        assert!(parse(&json!([r]), "0.1.0-dev.1").unwrap().is_none());
        r = release();
        r["html_url"] = json!("https://github.com/ayr1k/AMBER/releases/tag/v0.1.0");
        assert!(parse(&json!([r]), "0.1.0-dev.1").is_err());
        r = release();
        r["assets"][0]["digest"] = Value::Null;
        assert!(parse(&json!([r]), "0.1.0-dev.1").is_err());
        assert!(parse(&json!([]), "0.1.0-dev.1").unwrap().is_none());
    }
}
