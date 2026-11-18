// SPDX-License-Identifier: MPL-2.0
//! Run the verified official installer in the new server staging directory only.
use super::*;
use crate::loader::forge::{import_file, repo, write_new};
use std::{
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub(super) fn argument_file(c: &Choice) -> Result<String> {
    segment(&c.version)?;
    let path = match c.kind.as_str() {
        "Forge" => "net/minecraftforge/forge",
        "NeoForge" => "net/neoforged/neoforge",
        _ => return Err("Unknown server installer".into()),
    };
    Ok(format!("libraries/{path}/{}/win_args.txt", c.version))
}
pub(super) fn install(
    cache: &Cache,
    root: &Path,
    base: &Value,
    choice: &Choice,
    java: &Path,
    cancel: &Cancellation,
) -> Result<Runtime> {
    let game = base["id"].as_str().ok_or("Missing server identity")?;
    let java = paths::ordinary(java)?;
    if !java
        .file_name()
        .is_some_and(|s| s.eq_ignore_ascii_case("java.exe"))
    {
        return Err("请选择 java.exe".into());
    }
    let _java_parent = DirectoryGuard::open(java.parent().ok_or("Missing Java parent")?)?;
    let java_pin = options()
        .share_mode(FILE_SHARE_READ)
        .open(&java)
        .map_err(|e| e.to_string())?;
    if info(&java_pin)?.nNumberOfLinks != 1 {
        return Err("Linked Java rejected".into());
    }
    crate::java::compatible(
        &crate::java::inspect(&java, cancel)?,
        Some(base["javaVersion"]["majorVersion"].as_u64().unwrap_or(8) as u32),
    )?;
    let stem = choice.kind.to_ascii_lowercase();
    let url = format!(
        "{}/{}/{stem}-{}-installer.jar",
        repo(&choice.kind)?,
        choice.version,
        choice.version
    );
    let a = fetch(
        cache,
        &url,
        None,
        &crate::download::Https,
        cancel,
        256 * 1024 * 1024,
    )?;
    write_new(root, "installer.jar", &mut cache.verified_file(&a, cancel)?)?;
    let mut jar = options()
        .share_mode(FILE_SHARE_READ)
        .open(root.join("installer.jar"))
        .map_err(|e| e.to_string())?;
    if info(&jar)?.nNumberOfLinks != 1 {
        return Err("Linked installer rejected".into());
    }
    let mut hash = Hash::new(Algorithm::Sha1)?;
    let mut buf = [0; 65536];
    loop {
        cancel.check()?;
        let n = jar.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n])?;
    }
    if hash.finish()? != a.digest {
        return Err("Installer changed".into());
    }
    let log = options()
        .write(true)
        .create_new(true)
        .open(root.join("installer-console.log"))
        .map_err(|e| e.to_string())?;
    let mut child = Command::new(&java)
        .args([
            "-Djava.awt.headless=true",
            "-jar",
            "installer.jar",
            "--installServer",
        ])
        .current_dir(root)
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .env_remove("CLASSPATH")
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| e.to_string())?;
    let start = Instant::now();
    loop {
        if cancel.check().is_err() || start.elapsed() > Duration::from_secs(900) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("官方服务端安装器已取消或超时；暂存文件保留".into());
        }
        match child.try_wait() {
            Ok(Some(s)) if s.success() => break,
            Ok(Some(_)) => {
                return Err(format!("官方服务端安装失败；日志保留于 {}", root.display()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
    }
    let main = argument_file(choice)?;
    // Only the official installer's relative launch file is executed, never .bat scripts
    // or editable user_jvm_args. All executable inputs are recorded and pinned at launch.
    let args = paths::text(root, &root.join(&main), 65536)?;
    if args.contains(root.to_string_lossy().as_ref())
        || args.contains("@")
        || args.contains(":\\")
        || args.contains(":/")
    {
        return Err(
            "Installer generated external or nested arguments; preserved for diagnosis".into(),
        );
    }
    let mut runtime = Runtime {
        choice: choice.clone(),
        game: game.into(),
        main,
        files: vec![],
    };
    let mut pending = vec![root.join("libraries")];
    let mut guards = vec![];
    let mut total = 0u64;
    while let Some(dir) = pending.pop() {
        cancel.check()?;
        guards.push(DirectoryGuard::open(&dir)?);
        if guards.len() > 4096 {
            return Err("Installer directory limit".into());
        }
        for e in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            if e.file_type().map_err(|e| e.to_string())?.is_dir() {
                pending.push(e.path());
                continue;
            }
            if runtime.files.len() >= 4096 {
                return Err("Installer file limit".into());
            }
            let d = import_file(cache, root, &e.path(), cancel)?;
            let a = Artifact {
                url: d["url"].as_str().ok_or("Missing output URL")?.into(),
                size: d["size"].as_u64().ok_or("Missing size")?,
                algorithm: Algorithm::Sha1,
                digest: d["sha1"].as_str().ok_or("Missing hash")?.into(),
            };
            total = total.checked_add(a.size).ok_or("Installer size overflow")?;
            if total > 2 * 1024 * 1024 * 1024 {
                return Err("Installer output exceeds 2 GiB".into());
            }
            runtime.files.push(File {
                path: format!("libraries/{}", d["path"].as_str().ok_or("Missing path")?),
                artifact: a,
            });
        }
    }
    runtime.validate(game)?;
    Ok(runtime)
}
