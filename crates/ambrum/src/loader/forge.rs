// SPDX-License-Identifier: MPL-2.0
//! Official modern Forge/NeoForge installers in a unique app-owned work directory.
//! Never run installers against an existing library. Work is retained for diagnostics.
use super::*;
use crate::{
    manifest::{Platform, VersionPlan},
    registry::{DirectoryGuard, info, new_id, options},
};
use std::{
    fs,
    os::windows::{fs::OpenOptionsExt, process::CommandExt},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn repo(kind: &str) -> Result<&'static str> {
    match kind {
        "Forge" => Ok("https://maven.minecraftforge.net/net/minecraftforge/forge"),
        "NeoForge" => Ok("https://maven.neoforged.net/releases/net/neoforged/neoforge"),
        _ => Err("Unknown installer provider".into()),
    }
}
fn neo_game(v: &str) -> Option<String> {
    let (numbers, suffix) = v.split_once('+').unwrap_or((v, ""));
    let parts: Vec<_> = numbers.split(['.', '-']).collect();
    let major = parts.first()?.parse::<u32>().ok()?;
    let minor = parts.get(1)?.parse::<u32>().ok()?;
    if major < 26 {
        return Some(format!(
            "1.{major}{}",
            if minor == 0 {
                String::new()
            } else {
                format!(".{minor}")
            }
        ));
    }
    let patch = parts.get(2)?.parse::<u32>().ok()?;
    parts.get(3)?.parse::<u32>().ok()?;
    Some(format!(
        "{major}.{minor}{}{}",
        if patch == 0 {
            String::new()
        } else {
            format!(".{patch}")
        },
        if suffix.is_empty() {
            String::new()
        } else {
            format!("-{suffix}")
        }
    ))
}
pub(super) fn versions(
    game: &str,
    kind: &str,
    net: &impl Transport,
    cancel: &Cancellation,
) -> Result<Vec<String>> {
    let xml = String::from_utf8(bounded(
        net,
        &format!("{}/maven-metadata.xml", repo(kind)?),
        cancel,
    )?)
    .map_err(|_| "Invalid installer catalog")?;
    let mut result = Vec::new();
    for chunk in xml.split("<version>").skip(1) {
        let v = chunk
            .split_once("</version>")
            .ok_or("Invalid Maven catalog")?
            .0;
        if (kind == "Forge" && v.starts_with(&format!("{game}-")))
            || (kind == "NeoForge" && neo_game(v).as_deref() == Some(game))
        {
            segment(v)?;
            result.push(v.to_owned());
        }
    }
    if result.len() > 4096 {
        return Err("Installer catalog limit".into());
    }
    result.reverse();
    Ok(result)
}
fn write_new(root: &Path, relative: &str, bytes: &mut impl Read) -> Result<()> {
    crate::manifest::relative_path(relative)?;
    let target = root.join(relative);
    let parent = target.parent().ok_or("Missing work parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let _guard = DirectoryGuard::open(parent)?;
    let mut output = options()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|e| e.to_string())?;
    std::io::copy(bytes, &mut output).map_err(|e| e.to_string())?;
    output.sync_all().map_err(|e| e.to_string())
}
fn import_file(cache: &Cache, root: &Path, path: &Path, cancel: &Cancellation) -> Result<Value> {
    let _guard = DirectoryGuard::open(path.parent().ok_or("Missing output parent")?)?;
    let mut file = options()
        .share_mode(windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ)
        .open(path)
        .map_err(|e| e.to_string())?;
    if info(&file)?.nNumberOfLinks != 1 || !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Linked installer output rejected".into());
    }
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    if size > 256 * 1024 * 1024 {
        return Err("Installer output file exceeds 256 MiB".into());
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    let mut hash = Hash::new(Algorithm::Sha1)?;
    hash.update(&bytes)?;
    let digest = hash.finish()?;
    let relative = path
        .strip_prefix(root.join("libraries"))
        .map_err(|_| "Output outside libraries")?
        .to_str()
        .ok_or("Invalid output path")?
        .replace('\\', "/");
    crate::manifest::relative_path(&relative)?;
    // This synthetic HTTPS location is never downloaded: it denotes a verified local
    // official-processor output already imported into the content-addressed cache.
    let a = Artifact {
        url: format!("https://maven.minecraftforge.net/amberl-generated/{digest}"),
        size,
        algorithm: Algorithm::Sha1,
        digest,
    };
    cache.fetch(&a, &Bytes(bytes), cancel, |_| {})?;
    Ok(descriptor(&a, &relative))
}
#[allow(clippy::too_many_arguments)]
pub(super) fn profile(
    cache: &Cache,
    base: &Value,
    choice: &Choice,
    java: &Path,
    platform: &Platform,
    budget: u64,
    net: &impl Transport,
    cancel: &Cancellation,
) -> Result<Value> {
    let game = base["id"].as_str().ok_or("Missing game")?;
    let plan = VersionPlan::parse(base, platform)?;
    let java = paths::ordinary(java)?;
    let _java_parent = DirectoryGuard::open(java.parent().ok_or("Choose Java first")?)?;
    let _java_pin = options()
        .share_mode(windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ)
        .open(&java)
        .map_err(|_| "Cannot pin Java")?;
    if info(&_java_pin)?.nNumberOfLinks != 1 {
        return Err("Linked installer Java rejected".into());
    }
    crate::java::compatible(&crate::java::inspect(&java, cancel)?, plan.java_major)?;
    let stem = choice.kind.to_ascii_lowercase();
    let url = format!(
        "{}/{}/{stem}-{}-installer.jar",
        repo(&choice.kind)?,
        choice.version,
        choice.version
    );
    let installer = fetch(cache, &url, None, net, cancel, budget)?;
    let app = cache.root().parent().ok_or("Missing app root")?;
    let _app = DirectoryGuard::open(app)?;
    let work = app.join(format!("loader-work-{}", new_id()?));
    fs::create_dir(&work).map_err(|e| e.to_string())?;
    let _work = DirectoryGuard::open(&work)?;
    write_new(
        &work,
        "installer.jar",
        &mut cache.verified_file(&installer, cancel)?,
    )?;
    // Hold the copied executable input against replacement/writes until the installer
    // finishes, and recheck its hash after opening the pinned handle.
    let mut installer_pin = options()
        .share_mode(windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ)
        .open(work.join("installer.jar"))
        .map_err(|_| "Cannot pin official installer")?;
    if info(&installer_pin)?.nNumberOfLinks != 1
        || installer_pin.metadata().map_err(|e| e.to_string())?.len() != installer.size
    {
        return Err("Installer identity changed".into());
    }
    let mut hash = Hash::new(Algorithm::Sha1)?;
    let mut buffer = [0; 65536];
    loop {
        cancel.check()?;
        let n = installer_pin
            .read(&mut buffer)
            .map_err(|_| "Cannot verify installer")?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n])?;
    }
    if !hash.finish()?.eq_ignore_ascii_case(&installer.digest) {
        return Err("Installer checksum changed".into());
    }
    let mut bytes = installer.size;
    for f in plan
        .files
        .iter()
        .filter(|f| matches!(f.role, "client" | "library"))
    {
        bytes = bytes
            .checked_add(f.artifact.size)
            .ok_or("Installer budget overflow")?;
        if bytes > budget {
            return Err("Installer seed exceeds budget".into());
        }
        cache.fetch(&f.artifact, net, cancel, |_| {})?;
        write_new(
            &work,
            &f.path,
            &mut cache.verified_file(&f.artifact, cancel)?,
        )?;
    }
    write_new(
        &work,
        "launcher_profiles.json",
        &mut br#"{"profiles":{},"selectedProfile":""}"#.as_slice(),
    )?;
    write_new(
        &work,
        &format!("versions/{game}/{game}.json"),
        &mut serde_json::to_vec(base)
            .map_err(|e| e.to_string())?
            .as_slice(),
    )?;
    let log = options()
        .write(true)
        .create_new(true)
        .open(work.join("installer-console.log"))
        .map_err(|e| e.to_string())?;
    let mut child = Command::new(&java)
        .args([
            "-Djava.awt.headless=true",
            "-jar",
            "installer.jar",
            "--installClient",
        ])
        .arg(&work)
        .current_dir(&work)
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .env_remove("CLASSPATH")
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|_| "Cannot start official installer")?;
    let start = Instant::now();
    let status = loop {
        if cancel.check().is_err() || start.elapsed() > Duration::from_secs(900) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("安装器已取消或超时；暂存文件保留，原有游戏未修改".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Cannot wait for owned installer".into());
            }
        }
    };
    if !status.success() {
        return Err(format!(
            "官方安装器未完成（{}）；日志保留于 {}。旧版 Forge 安装器暂不支持。",
            status,
            work.display()
        ));
    }
    let mut profiles = Vec::new();
    for entry in fs::read_dir(work.join("versions")).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry
            .file_name()
            .to_str()
            .ok_or("Invalid profile name")?
            .to_owned();
        if name == game {
            continue;
        }
        paths::name(&name)?;
        profiles.push(paths::json(
            &work,
            &entry.path().join(format!("{name}.json")),
        )?);
    }
    if profiles.len() != 1 {
        return Err("Installer did not create one unambiguous client profile".into());
    }
    let mut profile = profiles.pop().unwrap();
    // Reject mismatches before adopting any generated executable input.
    merge(base, &profile, &choice.kind)?;
    let libs = profile["libraries"]
        .as_array_mut()
        .ok_or("Missing installed libraries")?;
    libs.retain(|l| l["clientreq"] != false);
    for lib in libs.iter_mut() {
        let relative = maven_path(lib["name"].as_str().ok_or("Missing library coordinate")?)?;
        let path = work.join("libraries").join(&relative);
        if !path.try_exists().map_err(|e| e.to_string())? {
            let url = lib
                .pointer("/downloads/artifact/url")
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    format!(
                        "{}/{}",
                        lib["url"]
                            .as_str()
                            .unwrap_or("https://libraries.minecraft.net")
                            .trim_end_matches('/'),
                        relative
                    )
                });
            let a = fetch(
                cache,
                &url,
                lib.pointer("/downloads/artifact/sha1")
                    .and_then(Value::as_str),
                net,
                cancel,
                budget,
            )?;
            write_new(
                &work,
                &format!("libraries/{relative}"),
                &mut cache.verified_file(&a, cancel)?,
            )?;
        }
        // If the profile publishes a checksum, compare installer-produced bytes too.
        let d = import_file(cache, &work, &path, cancel)?;
        if let Some(expected) = lib
            .pointer("/downloads/artifact/sha1")
            .and_then(Value::as_str)
            && !expected.is_empty()
            && !d["sha1"]
                .as_str()
                .unwrap_or("")
                .eq_ignore_ascii_case(expected)
        {
            return Err("Installer dependency checksum mismatch".into());
        }
        lib["downloads"] = json!({"artifact":d});
    }
    let mut pending = vec![work.join("libraries")];
    let mut extra = Vec::new();
    let mut bytes = 0u64;
    while let Some(dir) = pending.pop() {
        let guard = DirectoryGuard::open(&dir)?;
        for entry in fs::read_dir(&guard.root).map_err(|e| e.to_string())? {
            cancel.check()?;
            let path = entry.map_err(|e| e.to_string())?.path();
            paths::ordinary(&path)?;
            if path.is_dir() {
                if path.components().count() > work.components().count() + 32 {
                    return Err("Output depth limit".into());
                }
                pending.push(path);
            } else {
                let d = import_file(cache, &work, &path, cancel)?;
                bytes = bytes
                    .checked_add(d["size"].as_u64().ok_or("Missing output size")?)
                    .ok_or("Output budget overflow")?;
                if bytes > budget || extra.len() >= 4096 {
                    return Err("Installer output exceeds explicit budget".into());
                }
                extra.push(d);
            }
        }
    }
    profile["amberlExtra"] = Value::Array(extra);
    Ok(profile)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn neoforge_game_identity() {
        assert_eq!(neo_game("21.1.209").as_deref(), Some("1.21.1"));
        assert_eq!(neo_game("26.1.2.1-beta").as_deref(), Some("26.1.2"));
        assert_eq!(neo_game("bad"), None);
    }
}
