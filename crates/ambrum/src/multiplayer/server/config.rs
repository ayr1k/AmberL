// SPDX-License-Identifier: MPL-2.0
//! Bounded server settings with stale-edit detection and durable write recovery.
use super::*;
use std::io::{Seek, SeekFrom};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub motd: String,
    pub players: u16,
    pub distance: u8,
    pub difficulty: String,
    pub mode: String,
}
#[derive(Clone)]
pub struct Snapshot {
    pub server: String,
    pub digest: String,
    pub settings: Settings,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Recovery {
    pending: bool,
    original: String,
    interrupted: Option<String>,
}
fn digest(bytes: &[u8]) -> Result<String> {
    let mut h = Hash::new(Algorithm::Sha256)?;
    h.update(bytes)?;
    h.finish()
}
fn open(root: &Path) -> Result<(fs::File, String)> {
    let mut file = options()
        .write(true)
        .share_mode(0)
        .open(root.join("server.properties"))
        .map_err(|e| e.to_string())?;
    if info(&file)?.nNumberOfLinks != 1
        || !file.metadata().map_err(|e| e.to_string())?.is_file()
        || file.metadata().map_err(|e| e.to_string())?.len() > 65536
    {
        return Err("Invalid server settings; preserved".into());
    }
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|_| "配置编码不受支持，原文件已保留")?;
    Ok((file, text))
}
fn values(text: &str) -> Result<std::collections::BTreeMap<String, String>> {
    let mut values = std::collections::BTreeMap::new();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with(['#', '!']) && !l.is_empty())
    {
        let (key, value) = line
            .split_once('=')
            .ok_or("配置格式不受支持，原文件已保留")?;
        if key.contains(['\\', ':'])
            || !values
                .insert(key.trim().into(), value.trim().into())
                .is_none()
        {
            return Err("配置含重复或转义键，原文件已保留".into());
        }
    }
    Ok(values)
}
fn validate(s: &Settings) -> Result<()> {
    if s.motd.chars().count() > 128
        || s.motd.chars().any(char::is_control)
        || s.motd.contains('\\')
        || !(1..=1000).contains(&s.players)
        || !(2..=32).contains(&s.distance)
        || !["peaceful", "easy", "normal", "hard"].contains(&s.difficulty.as_str())
        || !["survival", "creative", "adventure", "spectator"].contains(&s.mode.as_str())
    {
        return Err("请检查描述、人数、视距、难度和游戏模式".into());
    }
    Ok(())
}
fn parse(text: &str) -> Result<Settings> {
    let map = values(text)?;
    let value = |key, default| {
        map.get(key)
            .cloned()
            .unwrap_or_else(|| String::from(default))
    };
    Ok(Settings {
        motd: decode(&value("motd", "AmberL Server"))?,
        players: value("max-players", "8").parse().map_err(|_| "无效人数")?,
        distance: value("view-distance", "8")
            .parse()
            .map_err(|_| "无效视距")?,
        difficulty: value("difficulty", "easy"),
        mode: value("gamemode", "survival"),
    })
}
fn encode(text: &str) -> String {
    text.encode_utf16()
        .map(|unit| {
            if (32..=126).contains(&unit) {
                char::from_u32(unit.into()).unwrap().to_string()
            } else {
                format!("\\u{unit:04x}")
            }
        })
        .collect()
}
fn decode(text: &str) -> Result<String> {
    let mut units = Vec::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('u') => {
                    let hex = chars.by_ref().take(4).collect::<String>();
                    if hex.len() != 4 {
                        return Err("无效描述转义，原文件已保留".into());
                    }
                    units.push(u16::from_str_radix(&hex, 16).map_err(|_| "无效描述转义")?);
                }
                Some(ch @ ('=' | ':' | ' ' | '\\' | '#' | '!')) => units.push(ch as u16),
                _ => return Err("描述转义暂不受支持，原文件已保留".into()),
            }
        } else {
            units.extend(ch.encode_utf16(&mut [0; 2]).iter().copied());
        }
    }
    String::from_utf16(&units).map_err(|_| "无效描述文字".into())
}
pub(super) fn ready(root: &Path) -> Result<()> {
    if Document::<Recovery>::open(root, "amberl-properties-recovery.json")?
        .data
        .pending
    {
        return Err("上次配置写入未完成，请先恢复配置".into());
    }
    Ok(())
}
pub fn read(app: &Path, id: &str) -> Result<Snapshot> {
    let (root, _lock, _) = content_lock(app, id)?;
    ready(&root.root)?;
    let (_file, text) = open(&root.root)?;
    Ok(Snapshot {
        server: id.into(),
        digest: digest(text.as_bytes())?,
        settings: parse(&text)?,
    })
}
fn write(file: &mut fs::File, text: &str) -> Result<()> {
    file.seek(SeekFrom::Start(0))
        .and_then(|_| file.set_len(0))
        .and_then(|_| file.write_all(text.as_bytes()))
        .and_then(|_| file.sync_all())
        .map_err(|_| "配置写入中断，恢复副本已保留；请先恢复配置".into())
}
pub fn save(app: &Path, expected: &Snapshot, settings: &Settings) -> Result<Snapshot> {
    validate(settings)?;
    let (root, _lock, _) = content_lock(app, &expected.server)?;
    let mut recovery = Document::<Recovery>::open(&root.root, "amberl-properties-recovery.json")?;
    if recovery.data.pending {
        return Err("请先恢复上次未完成的配置".into());
    }
    let (mut file, text) = open(&root.root)?;
    if digest(text.as_bytes())? != expected.digest {
        return Err("配置已被其他程序修改，请重新读取；未覆盖原文件".into());
    }
    values(&text)?;
    let edits = std::collections::BTreeMap::from([
        ("motd", encode(&settings.motd)),
        ("max-players", settings.players.to_string()),
        ("view-distance", settings.distance.to_string()),
        ("difficulty", settings.difficulty.clone()),
        ("gamemode", settings.mode.clone()),
    ]);
    let mut output = text
        .lines()
        .filter(|l| {
            !l.trim()
                .split_once('=')
                .is_some_and(|(k, _)| edits.contains_key(k.trim()))
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    output.extend(edits.iter().map(|(k, v)| format!("{k}={v}")));
    let output = output.join("\n") + "\n";
    if output.len() > 65536 {
        return Err("配置文件过大".into());
    }
    recovery.data = Recovery {
        pending: true,
        original: text,
        interrupted: None,
    };
    recovery.save()?;
    write(&mut file, &output)?;
    recovery.data.pending = false;
    recovery.save()?;
    Ok(Snapshot {
        server: expected.server.clone(),
        digest: digest(output.as_bytes())?,
        settings: settings.clone(),
    })
}
pub fn recover(app: &Path, id: &str) -> Result<()> {
    let (root, _lock, _) = content_lock(app, id)?;
    let mut recovery = Document::<Recovery>::open(&root.root, "amberl-properties-recovery.json")?;
    if !recovery.data.pending {
        return Err("没有未完成的配置写入".into());
    }
    if recovery.data.original.len() > 65536 {
        return Err("恢复副本过大".into());
    }
    values(&recovery.data.original)?;
    let (mut file, current) = open(&root.root)?;
    recovery.data.interrupted = Some(current);
    recovery.save()?;
    write(&mut file, &recovery.data.original)?;
    recovery.data.pending = false;
    recovery.save()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (std::path::PathBuf, String, std::path::PathBuf) {
        let app = std::env::temp_dir().join(format!("amberl-config-{}", new_id().unwrap()));
        let id = new_id().unwrap();
        let root = app.join("servers").join(&id);
        fs::create_dir_all(&root).unwrap();
        let object = DirectoryGuard::open(&root).unwrap().id.clone();
        fs::write(app.join("servers-v1.json"),serde_json::to_vec(&serde_json::json!({"schema":1,"servers":[{"id":id,"name":"QA","version":"26.3","port":25565,"object":object}]})).unwrap()).unwrap();
        fs::write(
            root.join("server.properties"),
            "# retained\nserver-port=25565\nonline-mode=true\nunknown-setting=yes\n",
        )
        .unwrap();
        (app, id, root)
    }
    #[test]
    fn config_preserves_unknowns_and_rejects_stale_live_or_unsafe_edits() {
        let (app, id, root) = fixture();
        let before = read(&app, &id).unwrap();
        let mut settings = before.settings.clone();
        settings.players = 12;
        settings.motd = "朋友的世界 🌲".into();
        save(&app, &before, &settings).unwrap();
        assert_eq!(read(&app, &id).unwrap().settings.motd, settings.motd);
        assert!(save(&app, &before, &settings).is_err());
        let after = fs::read_to_string(root.join("server.properties")).unwrap();
        assert!(after.contains("unknown-setting=yes"));
        assert!(after.contains("online-mode=true"));
        settings.motd = "bad\nonline-mode=false".into();
        assert!(save(&app, &read(&app, &id).unwrap(), &settings).is_err());
        let (_, lock, _) = content_lock(&app, &id).unwrap();
        assert!(read(&app, &id).is_err());
        drop(lock);
        fs::remove_dir_all(app).unwrap();
    }
    #[test]
    fn interrupted_write_blocks_launch_until_original_is_recovered() {
        let (app, id, root) = fixture();
        let original = fs::read_to_string(root.join("server.properties")).unwrap();
        let mut doc = Document::<Recovery>::open(&root, "amberl-properties-recovery.json").unwrap();
        doc.data = Recovery {
            pending: true,
            original: original.clone(),
            interrupted: None,
        };
        doc.save().unwrap();
        drop(doc);
        fs::write(root.join("server.properties"), "partial").unwrap();
        assert!(ready(&root).is_err());
        recover(&app, &id).unwrap();
        ready(&root).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("server.properties")).unwrap(),
            original
        );
        assert_eq!(
            Document::<Recovery>::open(&root, "amberl-properties-recovery.json")
                .unwrap()
                .data
                .interrupted
                .as_deref(),
            Some("partial")
        );
        fs::remove_dir_all(app).unwrap();
    }
}
