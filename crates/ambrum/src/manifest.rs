// SPDX-License-Identifier: MPL-2.0
//! Pure plans: no installation, network, process or filesystem mutation.
use crate::{
    Result,
    download::{Algorithm, Artifact},
    paths,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub struct Platform {
    pub os: String,
    pub arch: String,
    pub version: String,
    pub features: BTreeMap<String, bool>,
}
impl Platform {
    pub fn windows_x64(version: &str) -> Self {
        Self {
            os: "windows".into(),
            arch: "amd64".into(),
            version: version.into(),
            features: BTreeMap::new(),
        }
    }
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Missing/invalid metadata field: {key}"))
}
fn object(v: &Value) -> Result<&serde_json::Map<String, Value>> {
    v.as_object()
        .ok_or_else(|| "Expected metadata object".into())
}
fn list(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or_else(|| "Expected metadata list".into())
}

/// Ordered Mojang rules: default allow without rules, default deny with rules;
/// each matching rule overrides the previous result. No guessed OS versions.
pub fn allowed(rules: Option<&Value>, platform: &Platform) -> Result<bool> {
    let Some(rules) = rules else {
        return Ok(true);
    };
    let mut allowed = false;
    for rule in list(rules)? {
        if object(rule)?
            .keys()
            .any(|k| !matches!(k.as_str(), "action" | "os" | "features"))
        {
            return Err("Unsupported rule field".into());
        }
        let action = match string(rule, "action")? {
            "allow" => true,
            "disallow" => false,
            _ => return Err("Unknown rule action".into()),
        };
        let mut matches = true;
        if let Some(os) = rule.get("os") {
            for (key, value) in object(os)? {
                let value = value.as_str().ok_or("Invalid OS rule")?;
                matches &= match key.as_str() {
                    "name" => value == platform.os,
                    "arch" => regex::RegexBuilder::new(value)
                        .size_limit(256 * 1024)
                        .build()
                        .map_err(|_| "Unsupported OS architecture expression")?
                        .is_match(&platform.arch),
                    "version" => {
                        if !matches {
                            continue;
                        }
                        if platform.version.is_empty() {
                            return Err("OS version is required by this metadata rule".into());
                        }
                        regex::RegexBuilder::new(value)
                            .size_limit(256 * 1024)
                            .build()
                            .map_err(|_| "Unsupported OS version expression")?
                            .is_match(&platform.version)
                    }
                    _ => return Err("Unsupported OS rule field".into()),
                };
            }
        }
        if let Some(features) = rule.get("features") {
            for (key, value) in object(features)? {
                matches &= platform.features.get(key).copied().unwrap_or(false)
                    == value.as_bool().ok_or("Invalid feature rule")?;
            }
        }
        if matches {
            allowed = action;
        }
    }
    Ok(allowed)
}

fn arguments(value: &Value, platform: &Platform) -> Result<Vec<String>> {
    let mut args = Vec::new();
    for arg in list(value)? {
        if let Some(text) = arg.as_str() {
            args.push(text.into());
        } else {
            if !object(arg)?.contains_key("rules")
                || object(arg)?
                    .keys()
                    .any(|k| !matches!(k.as_str(), "rules" | "value"))
            {
                return Err("Invalid conditional argument".into());
            }
            if allowed(arg.get("rules"), platform)? {
                match arg
                    .get("value")
                    .ok_or("Missing conditional argument value")?
                {
                    Value::String(s) => args.push(s.clone()),
                    Value::Array(a) => {
                        for s in a {
                            args.push(s.as_str().ok_or("Invalid argument value")?.into());
                        }
                    }
                    _ => return Err("Invalid conditional argument value".into()),
                }
            }
        }
    }
    validate_arguments(&args)?;
    Ok(args)
}

fn validate_arguments(args: &[String]) -> Result<()> {
    if args.len() > 4096 || args.iter().any(|a| a.len() > 32768 || a.contains('\0')) {
        return Err("Argument limit exceeded".into());
    }
    Ok(())
}

pub fn relative_path(path: &str) -> Result<()> {
    if path.is_empty() || path.len() > 2048 || path.contains('\\') {
        return Err("Invalid resource path".into());
    }
    for component in path.split('/') {
        paths::name(component)?;
    }
    Ok(())
}
fn artifact(value: &Value) -> Result<Artifact> {
    let a = Artifact {
        url: string(value, "url")?.into(),
        size: value
            .get("size")
            .and_then(Value::as_u64)
            .ok_or("Missing resource size")?,
        algorithm: Algorithm::Sha1,
        digest: string(value, "sha1")?.into(),
    };
    a.validate()?;
    Ok(a)
}

#[derive(Serialize)]
pub struct FilePlan {
    pub path: String,
    pub role: &'static str,
    pub artifact: Artifact,
}
#[derive(Serialize)]
pub struct VersionPlan {
    pub version: String,
    pub java_major: Option<u32>,
    pub main_class: String,
    pub files: Vec<FilePlan>,
    pub classpath: Vec<String>,
    pub native_archives: Vec<String>,
    pub game_arguments: Vec<String>,
    pub jvm_arguments: Vec<String>,
    pub logging_argument: Option<String>,
    pub asset_index: String,
}
fn add_file(
    files: &mut BTreeMap<String, FilePlan>,
    path: String,
    role: &'static str,
    descriptor: &Value,
) -> Result<()> {
    relative_path(&path)?;
    let resource = artifact(descriptor)?;
    if let Some(existing) = files.get(&path.to_lowercase()) {
        if existing.path != path
            || existing.artifact.size != resource.size
            || !existing
                .artifact
                .digest
                .eq_ignore_ascii_case(&resource.digest)
        {
            return Err("Conflicting resource destinations".into());
        }
        return Ok(());
    }
    files.insert(
        path.to_lowercase(),
        FilePlan {
            path,
            role,
            artifact: resource,
        },
    );
    Ok(())
}

impl VersionPlan {
    pub fn parse(metadata: &Value, platform: &Platform) -> Result<Self> {
        let version = string(metadata, "id")?.to_owned();
        paths::name(&version)?;
        if metadata.get("inheritsFrom").is_some() {
            return Err(
                "Inherited/loader metadata needs a separate resolver; no partial install planned"
                    .into(),
            );
        }
        let main = string(metadata, "mainClass")?;
        if main != "net.minecraft.client.main.Main" {
            return Err("This plan supports the standalone Vanilla entrypoint only".into());
        }
        let java_major = metadata
            .get("javaVersion")
            .map(|j| {
                j.get("majorVersion")
                    .and_then(Value::as_u64)
                    .filter(|n| (8..=255).contains(n))
                    .map(|n| n as u32)
                    .ok_or_else(|| "Invalid Java requirement".to_string())
            })
            .transpose()?;
        let mut files = BTreeMap::new();
        let client_path = format!("versions/{version}/{version}.jar");
        add_file(
            &mut files,
            client_path.clone(),
            "client",
            metadata
                .pointer("/downloads/client")
                .ok_or("Missing client download")?,
        )?;
        let mut classpath = Vec::new();
        let mut native_archives = Vec::new();
        let libraries = list(metadata.get("libraries").ok_or("Missing libraries")?)?;
        if libraries.len() > 4096 {
            return Err("Library count limit exceeded".into());
        }
        for library in libraries {
            if !allowed(library.get("rules"), platform)? {
                continue;
            }
            let downloads = library
                .get("downloads")
                .ok_or("Legacy Maven-only library needs resolver; not silently omitted")?;
            let mut included = false;
            if let Some(file) = downloads.get("artifact") {
                let path = format!("libraries/{}", string(file, "path")?);
                add_file(&mut files, path.clone(), "library", file)?;
                if !classpath.contains(&path) {
                    classpath.push(path);
                }
                included = true;
            }
            if let Some(natives) = library.get("natives")
                && let Some(classifier) = object(natives)?.get(&platform.os)
            {
                let bits = match platform.arch.as_str() {
                    "x86" => "32",
                    "x86_64" | "amd64" => "64",
                    _ => return Err("Native classifier architecture unverified".into()),
                };
                let classifier = classifier
                    .as_str()
                    .ok_or("Invalid native classifier")?
                    .replace("${arch}", bits);
                paths::name(&classifier)?;
                let file = downloads
                    .get("classifiers")
                    .and_then(|c| c.get(&classifier))
                    .ok_or("Missing native artifact")?;
                let path = format!("libraries/{}", string(file, "path")?);
                add_file(&mut files, path.clone(), "native_archive", file)?;
                native_archives.push(path);
                included = true;
                // Extraction must independently validate every archive entry; no extraction here.
                if let Some(extract) = library.get("extract") {
                    if object(extract)?.keys().any(|k| k != "exclude") {
                        return Err("Unknown extraction rule".into());
                    }
                    if let Some(excludes) = extract.get("exclude") {
                        for exclusion in list(excludes)? {
                            if exclusion.as_str() != Some("META-INF/") {
                                return Err("Custom native exclusion needs explicit support".into());
                            }
                        }
                    }
                }
            }
            if !included {
                return Err("Applicable library has no supported download".into());
            }
        }
        classpath.push(client_path);
        let assets = metadata
            .get("assetIndex")
            .ok_or("Missing asset index descriptor")?;
        let asset_index = string(assets, "id")?.to_owned();
        paths::name(&asset_index)?;
        add_file(
            &mut files,
            format!("assets/indexes/{asset_index}.json"),
            "asset_index",
            assets,
        )?;
        let logging_argument = if let Some(logging) = metadata.pointer("/logging/client") {
            if string(logging, "type")? != "log4j2-xml" {
                return Err("Unsupported logging configuration".into());
            }
            let file = logging.get("file").ok_or("Missing logging file")?;
            let id = string(file, "id")?;
            paths::name(id)?;
            add_file(
                &mut files,
                format!("assets/log_configs/{id}"),
                "logging",
                file,
            )?;
            let argument = string(logging, "argument")?;
            if argument != "-Dlog4j.configurationFile=${path}" {
                return Err("Unsupported logging argument".into());
            }
            Some(argument.to_owned())
        } else {
            None
        };
        let (game_arguments, jvm_arguments) = if let Some(args) = metadata.get("arguments") {
            (
                arguments(args.get("game").ok_or("Missing game arguments")?, platform)?,
                arguments(args.get("jvm").ok_or("Missing JVM arguments")?, platform)?,
            )
        } else {
            let raw = string(metadata, "minecraftArguments")?;
            if raw.contains(['\'', '"', '\0']) {
                return Err("Quoted legacy arguments require explicit parsing support".into());
            }
            (
                raw.split_whitespace().map(str::to_owned).collect(),
                vec![
                    "-Djava.library.path=${natives_directory}".into(),
                    "-cp".into(),
                    "${classpath}".into(),
                ],
            )
        };
        validate_arguments(&game_arguments)?;
        validate_arguments(&jvm_arguments)?;
        Ok(Self {
            version,
            java_major,
            main_class: main.into(),
            files: files.into_values().collect(),
            classpath,
            native_archives,
            game_arguments,
            jvm_arguments,
            logging_argument,
            asset_index,
        })
    }
}

#[derive(Serialize)]
pub struct AssetPlan {
    pub files: Vec<FilePlan>,
    pub mappings: BTreeMap<String, String>,
    pub virtual_layout: bool,
    pub map_to_resources: bool,
}
impl AssetPlan {
    pub fn parse(index: &Value) -> Result<Self> {
        let objects = object(index.get("objects").ok_or("Missing asset objects")?)?;
        if objects.len() > 100_000 {
            return Err("Asset object limit exceeded".into());
        }
        let flag = |key| {
            index
                .get(key)
                .map(|v| v.as_bool().ok_or("Invalid asset layout flag"))
                .transpose()
                .map(|x| x.unwrap_or(false))
        };
        let mut files = BTreeMap::new();
        let mut mappings = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for (name, value) in objects {
            relative_path(name)?;
            if !seen.insert(name.to_lowercase()) {
                return Err("Case-colliding asset mappings".into());
            }
            let hash = string(value, "hash")?;
            if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("Invalid asset hash".into());
            }
            let hash = hash.to_ascii_lowercase();
            let suffix = format!("{}/{hash}", &hash[..2]);
            let path = format!("assets/objects/{suffix}");
            let descriptor = serde_json::json!({"url":format!("https://resources.download.minecraft.net/{suffix}"),"sha1":hash,"size":value.get("size").ok_or("Missing asset size")?});
            add_file(&mut files, path.clone(), "asset", &descriptor)?;
            mappings.insert(name.clone(), path);
        }
        Ok(Self {
            files: files.into_values().collect(),
            mappings,
            virtual_layout: flag("virtual")?,
            map_to_resources: flag("map_to_resources")?,
        })
    }
}
