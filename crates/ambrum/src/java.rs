// SPDX-License-Identifier: MPL-2.0
use crate::{Result, paths, task::Cancellation};
use serde::Serialize;
#[cfg(windows)]
pub mod runtime;
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Java {
    pub version: String,
    pub major: u32,
    pub architecture: String,
    pub home: String,
}

#[derive(Serialize)]
pub struct Candidate {
    pub executable: std::path::PathBuf,
    pub java: Option<Java>,
    pub issue: Option<String>,
}

/// Bounded discovery of conventional locations; never changes an existing choice.
pub fn discover(cancel: &Cancellation) -> Result<Vec<Candidate>> {
    let mut paths = Vec::new();
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        paths.push(std::path::PathBuf::from(home).join("bin/java.exe"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(
            std::env::split_paths(&path)
                .take(256)
                .map(|p| p.join("java.exe")),
        );
    }
    for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(variable) {
            for vendor in [
                "Java",
                "Microsoft",
                "Eclipse Adoptium",
                "AdoptOpenJDK",
                "Amazon Corretto",
            ] {
                if let Ok(entries) = std::fs::read_dir(std::path::Path::new(&base).join(vendor)) {
                    for entry in entries.take(64).flatten() {
                        paths.push(entry.path().join("bin/java.exe"));
                    }
                }
            }
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut candidates = Vec::new();
    for path in paths {
        cancel.check()?;
        if candidates.len() >= 32 {
            break;
        }
        let Ok(path) = paths::ordinary(&path) else {
            continue;
        };
        if !path.is_file() || !seen.insert(path.to_string_lossy().to_lowercase()) {
            continue;
        }
        let (java, issue) = match inspect(&path, cancel) {
            Ok(java) => (Some(java), None),
            Err(e) => (None, Some(e)),
        };
        candidates.push(Candidate {
            executable: path,
            java,
            issue,
        });
    }
    Ok(candidates)
}

/// Exact metadata major is the supported baseline, not an inferred minimum.
pub fn compatible(java: &Java, required: Option<u32>) -> Result<()> {
    let required = required.ok_or(
        "Java requirement unknown; this launch path requires official metadata with a Java major",
    )?;
    if java.major != required {
        return Err(format!(
            "请选择 Java {required}，当前为 Java {} / Select the required Java major",
            java.major
        ));
    }
    if !matches!(java.architecture.as_str(), "amd64" | "x86_64") {
        return Err("This Windows launch path requires x64 Java".into());
    }
    Ok(())
}

pub fn parse(output: &str) -> Result<Java> {
    let property = |key: &str| {
        output.lines().find_map(|line| {
            let (k, v) = line.trim().split_once('=')?;
            (k.trim() == key).then(|| v.trim().to_owned())
        })
    };
    let version = property("java.version").ok_or("Java 未返回版本 / Missing java.version")?;
    let significant = version.strip_prefix("1.").unwrap_or(&version);
    let major = significant
        .split(['.', '-', '+', '_'])
        .next()
        .unwrap_or("")
        .parse()
        .map_err(|_| "Java 版本无效 / Invalid Java version")?;
    if major == 0 {
        return Err("Java 版本无效 / Invalid Java version".into());
    }
    Ok(Java {
        version,
        major,
        architecture: property("os.arch").ok_or("缺失 Java 架构 / Missing architecture")?,
        home: property("java.home").ok_or("缺失 Java 路径 / Missing java.home")?,
    })
}

/// Inspect an explicitly chosen executable. Bounded time/output; only this child is killed.
pub fn inspect(executable: &Path, cancel: &Cancellation) -> Result<Java> {
    cancel.check()?;
    let path = paths::ordinary(executable)?;
    if !path.is_file() {
        return Err("请选择 Java 可执行文件 / Select a Java executable".into());
    }
    let mut command = Command::new(path);
    command
        .args(["-XshowSettings:properties", "-version"])
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Java 无法启动 / Cannot inspect Java: {e}"))?;
    fn drain(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<std::io::Result<Vec<u8>>> {
        thread::spawn(move || {
            let mut out = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let n = pipe.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                if out.len() + n > 128_000 {
                    return Err(std::io::Error::other("Java output exceeds limit"));
                }
                out.extend_from_slice(&buffer[..n]);
            }
            Ok(out)
        })
    }
    let stdout = drain(child.stdout.take().unwrap());
    let stderr = drain(child.stderr.take().unwrap());
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if cancel.check().is_err() || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            // Do not join pipe readers here: an unexpected descendant can keep a pipe open.
            return Err("Java 检测已取消或超时 / Java inspection cancelled or timed out".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
    };
    if !status.success() {
        return Err("Java 检测失败，未更改配置 / Java inspection failed".into());
    }
    // A child may exit while another process inherited a pipe. Keep the same time budget.
    while !stdout.is_finished() || !stderr.is_finished() {
        cancel.check()?;
        if Instant::now() >= deadline {
            return Err("Java 输出读取超时 / Java output timed out".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
    let mut output = stdout
        .join()
        .map_err(|_| "Java output worker failed")?
        .map_err(|e| e.to_string())?;
    output.extend(
        stderr
            .join()
            .map_err(|_| "Java output worker failed")?
            .map_err(|e| e.to_string())?,
    );
    parse(&String::from_utf8_lossy(&output))
}
