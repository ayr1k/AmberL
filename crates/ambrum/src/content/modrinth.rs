// SPDX-License-Identifier: MPL-2.0
use crate::{
    Result,
    catalog::bounded,
    download::{Algorithm, Artifact, Https},
    task::Cancellation,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    pub game: String,
    pub loader: String,
    pub server: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct Browse {
    pub game: bool,
    pub loader: bool,
}
impl Default for Browse {
    fn default() -> Self {
        Self {
            game: true,
            loader: true,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub description: String,
}
#[derive(Clone, Debug)]
pub struct Version {
    pub id: String,
    pub title: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct File {
    pub project: String,
    pub version: String,
    pub title: String,
    pub path: String,
    pub artifact: Artifact,
}
#[derive(Clone)]
pub struct OptionalDependency {
    pub key: String,
    pub title: String,
    pub selected: bool,
}
#[derive(Clone)]
pub struct Plan {
    pub context: Context,
    pub files: Vec<File>,
    pub optional: Vec<OptionalDependency>,
    pub incompatible: Vec<String>,
}
fn id(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 128
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err("Invalid content identity".into());
    }
    Ok(())
}
fn encoded(s: &str) -> String {
    s.as_bytes()
        .iter()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(b) {
                (*b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn get(path: &str, c: &Cancellation) -> Result<Value> {
    serde_json::from_slice(&bounded(
        &Https,
        &format!("https://api.modrinth.com/v2/{path}"),
        c,
    )?)
    .map_err(|_| "Invalid Modrinth response".into())
}
fn text(v: &Value, key: &str) -> Result<String> {
    let s = v[key].as_str().ok_or("Missing content field")?;
    if s.len() > 2048 || s.chars().any(|c| c.is_control()) {
        return Err("Invalid content text".into());
    }
    Ok(s.into())
}
fn category(kind: &str) -> Result<&'static str> {
    match kind {
        "mod" => Ok("mods"),
        "plugin" => Ok("plugins"),
        "resourcepack" => Ok("resourcepacks"),
        "shader" => Ok("shaderpacks"),
        _ => Err("Unsupported content kind".into()),
    }
}
fn filters(context: &Context, kind: &str) -> Result<Vec<Vec<String>>> {
    category(kind)?;
    crate::paths::name(&context.game)?;
    let mut f = vec![
        vec![format!("project_type:{kind}")],
        vec![format!("versions:{}", context.game)],
    ];
    if kind == "mod" {
        if !["fabric", "quilt", "forge", "neoforge"].contains(&context.loader.as_str()) {
            return Err("请先安装模组加载器".into());
        }
        f.push(vec![format!("categories:{}", context.loader)]);
    }
    if kind == "plugin" {
        if !context.server || context.loader != "paper" {
            return Err("插件需要 Paper 服务端".into());
        }
        f.push(vec![
            "categories:paper".into(),
            "categories:spigot".into(),
            "categories:bukkit".into(),
        ]);
    }
    if context.server {
        if !matches!(kind, "mod" | "plugin") {
            return Err("服务端只支持安装服务端模组或插件".into());
        }
        f.push(vec![
            "server_side:required".into(),
            "server_side:optional".into(),
        ]);
    }
    Ok(f)
}
pub fn search(
    query: &str,
    kind: &str,
    context: &Context,
    c: &Cancellation,
) -> Result<Vec<Project>> {
    search_with_filters(query, kind, context, Browse::default(), c)
}
fn browse_filters(context: &Context, kind: &str, browse: Browse) -> Result<Vec<Vec<String>>> {
    let mut rows = filters(context, kind)?;
    rows.retain(|row| {
        !row.iter().any(|value| {
            !browse.game && value.starts_with("versions:")
                || !browse.loader && value.starts_with("categories:")
        })
    });
    Ok(rows)
}
pub fn search_with_filters(
    query: &str,
    kind: &str,
    context: &Context,
    browse: Browse,
    c: &Cancellation,
) -> Result<Vec<Project>> {
    if query.chars().count() > 128 || query.chars().any(char::is_control) {
        return Err("搜索文字过长".into());
    }
    let f = serde_json::to_string(&browse_filters(context, kind, browse)?)
        .map_err(|e| e.to_string())?;
    let data = get(
        &format!(
            "search?query={}&facets={}&limit=30",
            encoded(query),
            encoded(&f)
        ),
        c,
    )?;
    let rows = data["hits"].as_array().ok_or("Missing search results")?;
    if rows.len() > 30 {
        return Err("Search result limit".into());
    }
    rows.iter()
        .map(|v| {
            let project = text(v, "project_id")?;
            id(&project)?;
            Ok(Project {
                id: project,
                title: text(v, "title")?,
                description: v["description"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(512)
                    .map(|c| if c.is_control() { ' ' } else { c })
                    .collect(),
            })
        })
        .collect()
}
fn catalog(project: &str, kind: &str, context: &Context, c: &Cancellation) -> Result<Vec<Value>> {
    browse_catalog(project, kind, context, Browse::default(), c)
}
fn browse_catalog(
    project: &str,
    kind: &str,
    context: &Context,
    browse: Browse,
    c: &Cancellation,
) -> Result<Vec<Value>> {
    id(project)?;
    filters(context, kind)?;
    let loaders = if !browse.loader {
        vec![]
    } else if kind == "plugin" {
        vec!["paper", "spigot", "bukkit"]
    } else if kind == "mod" {
        vec![context.loader.as_str()]
    } else if kind == "resourcepack" {
        vec!["minecraft"]
    } else {
        vec!["iris", "optifine"]
    };
    let mut query = vec![];
    if browse.game {
        query.push(format!(
            "game_versions={}",
            encoded(&json!([context.game]).to_string())
        ));
    }
    if browse.loader {
        query.push(format!("loaders={}", encoded(&json!(loaders).to_string())));
    }
    let query = if query.is_empty() {
        String::new()
    } else {
        format!("?{}", query.join("&"))
    };
    let value = get(&format!("project/{project}/version{query}"), c)?;
    let rows = value.as_array().ok_or("Invalid content versions")?;
    if rows.len() > 4096 {
        return Err("Content version limit".into());
    }
    let mut rows = rows.clone();
    rows.sort_by(|a, b| {
        b["date_published"]
            .as_str()
            .cmp(&a["date_published"].as_str())
    });
    Ok(rows)
}
pub fn versions(
    project: &str,
    kind: &str,
    context: &Context,
    c: &Cancellation,
) -> Result<Vec<Version>> {
    versions_with_filters(project, kind, context, Browse::default(), c)
}
pub fn versions_with_filters(
    project: &str,
    kind: &str,
    context: &Context,
    browse: Browse,
    c: &Cancellation,
) -> Result<Vec<Version>> {
    browse_catalog(project, kind, context, browse, c)?
        .iter()
        .map(|v| {
            let version = text(v, "id")?;
            id(&version)?;
            Ok(Version {
                id: version,
                title: format!(
                    "{} · {}{}",
                    text(v, "name")?,
                    text(v, "version_type")?,
                    if compatible(v, context, kind).is_ok() {
                        ""
                    } else {
                        " · 不匹配当前实例"
                    }
                ),
            })
        })
        .collect()
}
fn compatible(v: &Value, context: &Context, kind: &str) -> Result<()> {
    filters(context, kind)?;
    if !v["game_versions"]
        .as_array()
        .is_some_and(|vs| vs.iter().any(|v| v == &context.game))
    {
        return Err("内容版本不适配当前游戏".into());
    }
    let wanted = if kind == "plugin" {
        vec!["paper", "spigot", "bukkit"]
    } else if kind == "mod" {
        vec![context.loader.as_str()]
    } else if kind == "resourcepack" {
        vec!["minecraft"]
    } else {
        vec!["iris", "optifine"]
    };
    if !v["loaders"]
        .as_array()
        .is_some_and(|vs| vs.iter().any(|v| wanted.iter().any(|w| v == w)))
    {
        return Err("内容加载器不适配".into());
    }
    Ok(())
}
fn file(v: &Value, kind: &str) -> Result<File> {
    let project = text(v, "project_id")?;
    let version = text(v, "id")?;
    id(&project)?;
    id(&version)?;
    let files = v["files"].as_array().ok_or("Missing content files")?;
    let primary = files
        .iter()
        .filter(|f| f["primary"] == true)
        .collect::<Vec<_>>();
    let f = if primary.len() == 1 {
        primary[0]
    } else if files.len() == 1 {
        &files[0]
    } else {
        return Err("该版本有多个文件，无法确定主文件".into());
    };
    let name = text(f, "filename")?;
    crate::paths::name(&name)?;
    if !name
        .to_ascii_lowercase()
        .ends_with(if matches!(kind, "mod" | "plugin") {
            ".jar"
        } else {
            ".zip"
        })
    {
        return Err("Unexpected content file type".into());
    }
    let url = text(f, "url")?;
    if !url.starts_with("https://cdn.modrinth.com/data/") {
        return Err("Unexpected content download origin".into());
    }
    let a = Artifact {
        url,
        size: f["size"].as_u64().ok_or("Missing content size")?,
        algorithm: Algorithm::Sha1,
        digest: text(&f["hashes"], "sha1")?,
    };
    a.validate()?;
    if a.size > 512 * 1024 * 1024 {
        return Err("Content file exceeds 512 MiB".into());
    }
    Ok(File {
        project,
        version,
        title: text(v, "name")?,
        path: format!("{}/{name}", category(kind)?),
        artifact: a,
    })
}
pub fn plan(version: &str, kind: &str, context: &Context, c: &Cancellation) -> Result<Plan> {
    plan_with_choices(version, kind, context, &BTreeSet::new(), c)
}
fn project_kind(project: &Value, context: &Context) -> Result<String> {
    let kind = text(project, "project_type")?;
    Ok(
        if kind == "mod" && context.server && context.loader == "paper" {
            "plugin".into()
        } else {
            kind
        },
    )
}
pub fn plan_with_choices(
    version: &str,
    kind: &str,
    context: &Context,
    choices: &BTreeSet<String>,
    c: &Cancellation,
) -> Result<Plan> {
    id(version)?;
    filters(context, kind)?;
    if choices.len() > 128 {
        return Err("Dependency selection limit".into());
    }
    let mut pending = vec![(version.to_owned(), kind.to_owned())];
    let mut seen = BTreeMap::new();
    let mut paths = BTreeSet::new();
    let mut files = vec![];
    let mut optional = vec![];
    let mut incompatible = vec![];
    let mut total = 0u64;
    while let Some((id, kind)) = pending.pop() {
        if files.len() + pending.len() > 128 {
            return Err("Dependency limit".into());
        }
        let v = get(&format!("version/{id}"), c)?;
        if v["id"] != id {
            return Err("Version identity mismatch".into());
        }
        let project = get(&format!("project/{}", text(&v, "project_id")?), c)?;
        let actual_kind = project_kind(&project, context)?;
        if !kind.is_empty() && actual_kind != kind {
            return Err("Content project type changed".into());
        }
        let kind = actual_kind;
        compatible(&v, context, &kind)?;
        let mut f = file(&v, &kind)?;
        f.title = format!("{} · {}", text(&project, "title")?, f.title);
        if let Some(previous) = seen.get(&f.project) {
            if previous != &f.version {
                return Err("依赖要求不同版本，请调整选择".into());
            }
            continue;
        }
        if project["id"] != f.project || project_kind(&project, context)? != kind {
            return Err("Content project identity mismatch".into());
        }
        if context.server && project["server_side"] == "unsupported" {
            return Err("该模组不支持服务端".into());
        }
        if !context.server && project["client_side"] == "unsupported" {
            return Err("该内容不支持客户端".into());
        }
        seen.insert(f.project.clone(), f.version.clone());
        if !paths.insert(f.path.to_ascii_lowercase()) {
            return Err("Dependency filename conflict".into());
        }
        total = total
            .checked_add(f.artifact.size)
            .ok_or("Content size overflow")?;
        if total > 1024 * 1024 * 1024 {
            return Err("Content plan exceeds 1 GiB".into());
        }
        for d in v["dependencies"]
            .as_array()
            .ok_or("Missing dependency list")?
        {
            let project = d["project_id"].as_str();
            let version = d["version_id"].as_str();
            match d["dependency_type"].as_str() {
                Some("required") | Some("optional") => {
                    if d["dependency_type"] == "optional" {
                        let (key, title) = if let Some(v) = version {
                            id_check(v)?;
                            (
                                format!("version:{v}"),
                                text(&get(&format!("version/{v}"), c)?, "name")?,
                            )
                        } else if let Some(p) = project {
                            id_check(p)?;
                            (
                                format!("project:{p}"),
                                text(&get(&format!("project/{p}"), c)?, "title")?,
                            )
                        } else {
                            return Err("Optional dependency lacks an identity".into());
                        };
                        let selected = choices.contains(&key);
                        if !optional.iter().any(|o: &OptionalDependency| o.key == key) {
                            optional.push(OptionalDependency {
                                key,
                                title,
                                selected,
                            });
                        }
                        if !selected {
                            continue;
                        }
                    }
                    let next = if let Some(v) = version {
                        id_check(v)?;
                        v.to_owned()
                    } else {
                        let p = project.ok_or("Required dependency lacks an identity")?;
                        id_check(p)?;
                        let dependency = get(&format!("project/{p}"), c)?;
                        let rows = catalog(p, &project_kind(&dependency, context)?, context, c)?;
                        let best = rows
                            .iter()
                            .find(|r| r["version_type"] == "release")
                            .or(rows.first())
                            .ok_or("找不到兼容的必需依赖")?;
                        text(best, "id")?
                    };
                    pending.push((next, String::new()));
                }
                Some("incompatible") => {
                    if let Some(p) = project {
                        id_check(p)?;
                        incompatible.push(p.to_owned());
                    } else {
                        return Err(
                            "Version-only incompatibility requires manual resolution".into()
                        );
                    }
                }
                Some("embedded") => {}
                _ => return Err("Unknown dependency relationship".into()),
            }
        }
        files.push(f);
    }
    if incompatible.iter().any(|id| seen.contains_key(id)) {
        return Err("所选内容存在互斥依赖".into());
    }
    if choices
        .iter()
        .any(|key| !optional.iter().any(|o| &o.key == key))
    {
        return Err("可选依赖已变化，请重新预览".into());
    }
    Ok(Plan {
        context: context.clone(),
        files,
        optional,
        incompatible,
    })
}
fn id_check(s: &str) -> Result<()> {
    id(s)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browsing_overrides_keep_target_side_and_do_not_relax_installation() {
        let context = Context {
            game: "26.3".into(),
            loader: "fabric".into(),
            server: true,
        };
        let full = browse_filters(&context, "mod", Browse::default()).unwrap();
        assert!(full.iter().flatten().any(|f| f == "versions:26.3"));
        let broad = browse_filters(
            &context,
            "mod",
            Browse {
                game: false,
                loader: false,
            },
        )
        .unwrap();
        assert!(
            !broad
                .iter()
                .flatten()
                .any(|f| f.starts_with("versions:") || f.starts_with("categories:"))
        );
        assert!(broad.iter().flatten().any(|f| f == "server_side:required"));
        assert!(broad.iter().flatten().any(|f| f == "project_type:mod"));
        assert!(
            compatible(
                &json!({"game_versions":["1.12.2"],"loaders":["forge"]}),
                &context,
                "mod"
            )
            .is_err()
        );
    }
    #[test]
    fn plugins_require_server_loader_and_keep_a_separate_destination() {
        let mut context = Context {
            game: "26.3".into(),
            loader: "paper".into(),
            server: true,
        };
        let v = json!({"id":"version1","project_id":"project1","name":"test","game_versions":["26.3"],"loaders":["bukkit"],"files":[{"filename":"test.jar","primary":true,"size":3,"url":"https://cdn.modrinth.com/data/a/test.jar","hashes":{"sha1":"0".repeat(40)}}]});
        compatible(&v, &context, "plugin").unwrap();
        assert_eq!(file(&v, "plugin").unwrap().path, "plugins/test.jar");
        assert_eq!(
            project_kind(&json!({"project_type":"mod"}), &context).unwrap(),
            "plugin"
        );
        assert!(compatible(&v, &context, "mod").is_err());
        context.server = false;
        assert!(compatible(&v, &context, "plugin").is_err());
        context.server = true;
        context.loader = "fabric".into();
        assert!(compatible(&v, &context, "plugin").is_err());
    }
    #[test]
    fn provider_context_and_content_paths_are_not_inferred() {
        let c = Context {
            game: "26.3".into(),
            loader: "fabric".into(),
            server: true,
        };
        let mut v = json!({"id":"version1","project_id":"project1","name":"test","game_versions":["26.3"],"loaders":["fabric"],"files":[{"filename":"test.jar","primary":true,"size":3,"url":"https://cdn.modrinth.com/data/a/test.jar","hashes":{"sha1":"0".repeat(40)}}]});
        compatible(&v, &c, "mod").unwrap();
        file(&v, "mod").unwrap();
        v["files"][0]["filename"] = "../escape.jar".into();
        assert!(file(&v, "mod").is_err());
        v["game_versions"] = json!(["1.21.1"]);
        assert!(compatible(&v, &c, "mod").is_err());
        assert!(filters(&c, "shader").is_err());
        assert_eq!(encoded("你好 &"), "%E4%BD%A0%E5%A5%BD%20%26");
    }
}
