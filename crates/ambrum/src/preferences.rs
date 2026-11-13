// SPDX-License-Identifier: MPL-2.0
use crate::{Result, state::Document};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Launch {
    pub java: PathBuf,
    pub account: Option<String>,
    pub memory_mib: u32,
}
impl Default for Launch {
    fn default() -> Self {
        Self {
            java: PathBuf::new(),
            account: None,
            memory_mib: 4096,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    schema: u32,
    selected: Option<String>,
    games: BTreeMap<String, Launch>,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            schema: 1,
            selected: None,
            games: BTreeMap::new(),
        }
    }
}
fn validate(data: &Data) -> Result<()> {
    if data.schema != 1 || data.games.len() > 20000 {
        return Err("Unsupported preferences; preserved".into());
    }
    for (id, p) in &data.games {
        if id.is_empty()
            || id.len() > 4096
            || !(512..=65536).contains(&p.memory_mib)
            || (!p.java.as_os_str().is_empty() && !p.java.is_absolute())
            || p.account
                .as_ref()
                .is_some_and(|a| a.len() != 32 || !a.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err("Invalid saved launch preference; preserved".into());
        }
    }
    if data
        .selected
        .as_ref()
        .is_some_and(|s| !data.games.contains_key(s))
    {
        return Err("Invalid saved selection; preserved".into());
    }
    Ok(())
}
pub fn load(app: &Path) -> Result<(Option<String>, BTreeMap<String, Launch>)> {
    let doc = Document::<Data>::open(app, "launch-preferences-v1.json")?;
    validate(&doc.data)?;
    Ok((doc.data.selected, doc.data.games))
}
pub fn save(app: &Path, id: &str, preference: Launch) -> Result<()> {
    let mut doc = Document::<Data>::open(app, "launch-preferences-v1.json")?;
    validate(&doc.data)?;
    doc.data.games.insert(id.into(), preference);
    doc.data.selected = Some(id.into());
    validate(&doc.data)?;
    doc.save()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_does_not_erase_other_games_or_accept_bad_memory() {
        let p = std::env::temp_dir().join(format!(
            "amberl-prefs-{}",
            crate::registry::new_id().unwrap()
        ));
        std::fs::create_dir(&p).unwrap();
        save(&p, "game-a", Launch::default()).unwrap();
        let b = Launch {
            java: "C:/Java/bin/java.exe".into(),
            memory_mib: 8192,
            ..Launch::default()
        };
        save(&p, "game-b", b.clone()).unwrap();
        assert_eq!(load(&p).unwrap().1["game-b"], b);
        assert_eq!(load(&p).unwrap().1.len(), 2);
        assert!(save(&p, "game-a", Launch { memory_mib: 0, ..b }).is_err());
        assert_eq!(load(&p).unwrap().1["game-a"].memory_mib, 4096);
        std::fs::write(p.join("launch-preferences-v1.json"), b"broken").unwrap();
        assert!(save(&p, "game-c", Launch::default()).is_err());
        assert_eq!(
            std::fs::read(p.join("launch-preferences-v1.json")).unwrap(),
            b"broken"
        );
    }
    #[test]
    fn linked_preferences_cannot_overwrite_external_data() {
        let p = std::env::temp_dir().join(format!(
            "amberl-prefs-links-{}",
            crate::registry::new_id().unwrap()
        ));
        std::fs::create_dir(&p).unwrap();
        let outside = p.join("keep.txt");
        std::fs::write(&outside, b"keep").unwrap();
        std::fs::hard_link(&outside, p.join("launch-preferences-v1.json")).unwrap();
        assert!(save(&p, "game", Launch::default()).is_err());
        assert_eq!(std::fs::read(&outside).unwrap(), b"keep");
    }
}
