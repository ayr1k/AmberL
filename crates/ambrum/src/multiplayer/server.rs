// SPDX-License-Identifier: MPL-2.0
//! App-owned servers, verified runtime bytes and graceful stdin stop. No imported saves.
use crate::{
    Result, catalog,
    download::{Algorithm, Artifact, Cache, Https, windows::Hash},
    java,
    registry::{DirectoryGuard, ObjectId, info, new_id, options},
    state::Document,
    task::Cancellation,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    os::windows::{fs::OpenOptionsExt, process::CommandExt},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
pub mod backup;
pub mod config;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub version: String,
    pub port: u16,
    #[serde(default)]
    pub loader: Option<crate::loader::server::Runtime>,
    #[serde(default)]
    pub archived: bool,
    object: ObjectId,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    schema: u32,
    servers: Vec<Server>,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            schema: 1,
            servers: vec![],
        }
    }
}
fn validate(data: &Data) -> Result<()> {
    if data.schema != 1 || data.servers.len() > 256 {
        return Err("Unsupported server registry; preserved".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    for s in &data.servers {
        if s.id.len() != 32 || !s.id.bytes().all(|b| b.is_ascii_hexdigit()) || !ids.insert(&s.id) {
            return Err("Invalid server identity".into());
        }
        details(&s.name, &s.version, s.port)?;
        if let Some(loader) = &s.loader {
            loader.validate(&s.version)?;
        }
    }
    Ok(())
}
fn details(name: &str, version: &str, port: u16) -> Result<()> {
    crate::paths::name(version)?;
    if name.trim().is_empty()
        || name.chars().count() > 64
        || name.chars().any(char::is_control)
        || port < 1024
    {
        return Err("请选择服务端名称和 1024–65535 端口".into());
    }
    Ok(())
}
pub fn list(app: &Path) -> Result<Vec<Server>> {
    let doc = Document::<Data>::open(app, "servers-v1.json")?;
    validate(&doc.data)?;
    Ok(doc
        .data
        .servers
        .into_iter()
        .filter(|s| !s.archived)
        .collect())
}
pub fn archived(app: &Path) -> Result<Vec<Server>> {
    let doc = Document::<Data>::open(app, "servers-v1.json")?;
    validate(&doc.data)?;
    Ok(doc
        .data
        .servers
        .into_iter()
        .filter(|s| s.archived)
        .collect())
}
/// Recoverable removal: keep the owned world and identity, hide it from launch choices.
pub fn set_archived(app: &Path, id: &str, archived: bool) -> Result<()> {
    let mut doc = Document::<Data>::open(app, "servers-v1.json")?;
    validate(&doc.data)?;
    let s = doc
        .data
        .servers
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or("服务端选择已失效")?;
    let (_root, _lock) = maintenance_lock(app, s)?;
    s.archived = archived;
    doc.save()
}
fn maintenance_lock(app: &Path, s: &Server) -> Result<(DirectoryGuard, fs::File)> {
    let root = DirectoryGuard::open(&app.join("servers").join(&s.id))?;
    if root.id != s.object {
        return Err("服务端目录身份已变化；原数据已保留".into());
    }
    let lock = options()
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.root.join("amberl-session.lock"))
        .map_err(|e| e.to_string())?;
    if info(&lock)?.nNumberOfLinks != 1 {
        return Err("Linked session lock".into());
    }
    lock.try_lock().map_err(|_| "请先保存并停止该服务端")?;
    Ok((root, lock))
}
pub(crate) fn content_lock(app: &Path, id: &str) -> Result<(DirectoryGuard, fs::File, Server)> {
    let s = list(app)?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or("请选择有效服务端")?;
    let (root, lock) = maintenance_lock(app, &s)?;
    Ok((root, lock, s))
}
fn artifact(metadata: &serde_json::Value) -> Result<Artifact> {
    let a = &metadata["downloads"]["server"];
    let url = a["url"].as_str().ok_or("此版本没有官方服务端")?;
    if ![
        "https://piston-data.mojang.com/",
        "https://launcher.mojang.com/",
    ]
    .iter()
    .any(|p| url.starts_with(p))
    {
        return Err("Unexpected server download origin".into());
    }
    let out = Artifact {
        url: url.into(),
        size: a["size"].as_u64().ok_or("Missing server size")?,
        algorithm: Algorithm::Sha1,
        digest: a["sha1"].as_str().ok_or("Missing server checksum")?.into(),
    };
    out.validate()?;
    if out.size > 512 * 1024 * 1024 {
        return Err("Server download exceeds 512 MiB".into());
    }
    Ok(out)
}
fn metadata(
    app: &Path,
    version: &str,
    cancel: &Cancellation,
) -> Result<(Cache, serde_json::Value)> {
    let cache = Cache::open(app)?;
    let r = catalog::resolve(&cache, version, &Https, cancel)?;
    let data = crate::paths::json(
        r.metadata_path.parent().ok_or("Missing metadata parent")?,
        &r.metadata_path,
    )?;
    Ok((cache, data))
}
pub fn create(
    app: &Path,
    name: &str,
    version: &str,
    port: u16,
    eula: bool,
    cancel: &Cancellation,
) -> Result<Server> {
    create_with_loader(
        app,
        &Create {
            name: name.into(),
            version: version.into(),
            port,
            eula,
            loader: None,
            java: None,
        },
        cancel,
    )
}
pub struct Create {
    pub name: String,
    pub version: String,
    pub port: u16,
    pub eula: bool,
    pub loader: Option<crate::loader::Choice>,
    pub java: Option<std::path::PathBuf>,
}
pub fn create_with_loader(app: &Path, request: &Create, cancel: &Cancellation) -> Result<Server> {
    let Create {
        name,
        version,
        port,
        eula,
        loader,
        java,
    } = request;
    let (port, eula) = (*port, *eula);
    details(name, version, port)?;
    if !eula {
        return Err("请先阅读并同意 Minecraft EULA，再创建服务端".into());
    }
    let mut doc = Document::<Data>::open(app, "servers-v1.json")?;
    validate(&doc.data)?;
    if doc.data.servers.len() >= 256 {
        return Err("Server limit reached".into());
    }
    let app_guard = DirectoryGuard::open(app)?;
    let parent = app_guard.root.join("servers");
    match fs::create_dir(&parent) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }
    let _parent = DirectoryGuard::open(&parent)?;
    let (cache, metadata) = metadata(app, version, cancel)?;
    let a = artifact(&metadata)?;
    cache.fetch(&a, &Https, cancel, |_| {})?;
    let mut source = cache.verified_file(&a, cancel)?;
    let id = new_id()?;
    let stage = parent.join(format!("stage-{id}"));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let stage_guard = DirectoryGuard::open(&stage)?;
    let mut jar = options()
        .write(true)
        .create_new(true)
        .open(stage.join("server.jar"))
        .map_err(|e| e.to_string())?;
    std::io::copy(&mut source, &mut jar)
        .and_then(|_| jar.sync_all())
        .map_err(|e| e.to_string())?;
    drop(jar);
    let loader = loader
        .as_ref()
        .map(|choice| {
            crate::loader::server::install(
                &cache,
                &stage,
                &metadata,
                choice,
                java.as_deref(),
                cancel,
            )
        })
        .transpose()?;
    for (name, bytes) in [
        ("eula.txt", "eula=true\n".to_string()),
        (
            "server.properties",
            format!(
                "server-port={port}\nserver-ip=127.0.0.1\nonline-mode=true\nwhite-list=false\nenable-status=true\nenable-query=false\nenable-rcon=false\nmax-players=8\nview-distance=8\nsimulation-distance=6\nmotd=AmberL Server\n"
            ),
        ),
    ] {
        let mut f = options()
            .write(true)
            .create_new(true)
            .open(stage.join(name))
            .map_err(|e| e.to_string())?;
        f.write_all(bytes.as_bytes())
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
    }
    cancel.check()?;
    drop(stage_guard);
    let target = parent.join(&id);
    fs::rename(&stage, &target).map_err(|e| e.to_string())?;
    let root = DirectoryGuard::open(&target)?;
    let record = Server {
        id,
        name: name.trim().into(),
        version: version.into(),
        port,
        loader,
        object: root.id.clone(),
        archived: false,
    };
    doc.data.servers.push(record.clone());
    doc.save()?;
    Ok(record)
}
#[derive(Clone, Debug)]
pub enum Event {
    Started(u32),
    Ready,
    Line(String),
    Stopping,
}
struct OwnedServer {
    child: std::process::Child,
    input: std::process::ChildStdin,
}
impl Drop for OwnedServer {
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.input.write_all(b"stop\n");
            let _ = self.input.flush();
            // Preserve the directory guards until graceful exit, including error unwinding.
            let _ = self.child.wait();
        }
    }
}
pub fn console(value: &str) -> Result<String> {
    let v = value.trim();
    if v.is_empty() || v.len() > 1024 || v.chars().any(char::is_control) {
        return Err("控制台命令必须为单行，最多 1024 字节".into());
    }
    Ok(format!("{v}\n"))
}
fn pin(path: &Path) -> Result<fs::File> {
    let f = options()
        .share_mode(FILE_SHARE_READ)
        .open(path)
        .map_err(|e| e.to_string())?;
    if info(&f)?.nNumberOfLinks != 1 || !f.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Linked executable rejected".into());
    }
    Ok(f)
}
fn guard_tree(
    path: &Path,
    depth: u32,
    budget: &mut usize,
    guards: &mut Vec<DirectoryGuard>,
) -> Result<()> {
    if depth > 32 {
        return Err("Server directory depth limit".into());
    }
    guards.push(DirectoryGuard::open(path)?);
    for e in fs::read_dir(path).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        *budget += 1;
        if *budget > 100000 {
            return Err("Server file limit".into());
        }
        let p = crate::paths::ordinary(&e.path())?;
        if p.is_dir() {
            guard_tree(&p, depth + 1, budget, guards)?;
        } else {
            let f = options().open(p).map_err(|e| e.to_string())?;
            if info(&f)?.nNumberOfLinks != 1 {
                return Err("Linked server content rejected".into());
            }
        }
    }
    Ok(())
}
pub fn run(
    app: &Path,
    id: &str,
    java_path: &Path,
    memory: u32,
    cancel: &Cancellation,
    commands: &mpsc::Receiver<String>,
    mut notify: impl FnMut(Event),
) -> Result<i32> {
    if !(512..=65536).contains(&memory) {
        return Err("服务端内存必须为 512–65536 MiB".into());
    }
    let s = list(app)?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or("服务端选择已失效")?;
    let root = DirectoryGuard::open(&app.join("servers").join(&s.id))?;
    if root.id != s.object {
        return Err("服务端目录身份已变化；保留原数据并拒绝启动".into());
    }
    let lock = options()
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.root.join("amberl-session.lock"))
        .map_err(|e| e.to_string())?;
    if info(&lock)?.nNumberOfLinks != 1 {
        return Err("Linked session lock".into());
    }
    lock.try_lock().map_err(|_| "该服务端已在运行")?;
    config::ready(&root.root)?;
    let mut guards = vec![];
    guard_tree(&root.root, 0, &mut 0, &mut guards)?;
    let properties = crate::paths::text(&root.root, &root.root.join("server.properties"), 65536)?;
    for (key, expected) in [
        ("server-port", s.port.to_string()),
        ("server-ip", "127.0.0.1".into()),
        ("online-mode", "true".into()),
        ("enable-rcon", "false".into()),
    ] {
        let values = properties
            .lines()
            .filter_map(|l| l.trim().split_once('='))
            .filter(|(k, _)| k.trim() == key)
            .collect::<Vec<_>>();
        if values.len() != 1 || values[0].1.trim() != expected {
            return Err(format!(
                "服务端配置 {key} 与当前受支持配置不一致；原文件已保留"
            ));
        }
    }
    let (cache, meta) = metadata(app, &s.version, cancel)?;
    let a = artifact(&meta)?;
    let major = meta["javaVersion"]["majorVersion"].as_u64().unwrap_or(8) as u32;
    let java_path = crate::paths::ordinary(java_path)?;
    let _java_parent = DirectoryGuard::open(java_path.parent().ok_or("Choose Java")?)?;
    let _java = pin(&java_path)?;
    if !java_path
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("java.exe"))
    {
        return Err("服务端请选择 java.exe".into());
    }
    java::compatible(&java::inspect(&java_path, cancel)?, Some(major))?;
    let mut jar = pin(&root.root.join("server.jar"))?;
    let mut hash = Hash::new(Algorithm::Sha1)?;
    let mut buf = [0; 65536];
    let mut size = 0;
    loop {
        cancel.check()?;
        let n = jar.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > a.size {
            return Err("Server JAR size mismatch".into());
        }
        hash.update(&buf[..n])?;
    }
    if size != a.size || hash.finish()? != a.digest {
        return Err("服务端文件校验失败；未启动".into());
    }
    let _loader_pins = s
        .loader
        .as_ref()
        .map(|l| l.pin(&root.root, &s.version, cancel))
        .transpose()?;
    let mut args = vec![format!("-Xmx{memory}M"), "-Xms256M".into()];
    args.extend(s.loader.as_ref().map_or_else(
        || vec!["-jar".into(), "server.jar".into()],
        |l| l.arguments(),
    ));
    args.push("nogui".into());
    drop(cache);
    let port = TcpListener::bind(("127.0.0.1", s.port))
        .map_err(|_| "服务端端口已占用，请停止占用程序或创建不同端口的服务端")?;
    drop(port);
    cancel.check()?;
    let mut child = Command::new(&java_path)
        .args(args)
        .current_dir(&root.root)
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .env_remove("CLASSPATH")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| e.to_string())?;
    let (tx, rx) = mpsc::sync_channel(256);
    drain(child.stdout.take().unwrap(), tx.clone());
    drain(child.stderr.take().unwrap(), tx);
    let input = child.stdin.take().unwrap();
    let mut owned = OwnedServer { child, input };
    notify(Event::Started(owned.child.id()));
    let mut stopping = false;
    let mut ready = false;
    loop {
        if let Some(exit) = owned.child.try_wait().map_err(|e| e.to_string())? {
            while let Ok(line) = rx.try_recv() {
                notify(Event::Line(line));
            }
            return Ok(exit.code().unwrap_or(-1));
        }
        for _ in 0..128 {
            let Ok(line) = rx.try_recv() else { break };
            if !ready && !stopping && line.contains("Done (") && line.contains("For help") {
                ready = true;
                notify(Event::Ready);
            }
            notify(Event::Line(line));
        }
        if cancel.check().is_err() && !stopping {
            stopping = true;
            notify(Event::Stopping);
            let _ = owned.input.write_all(b"stop\n");
            let _ = owned.input.flush();
        }
        if !stopping {
            for cmd in commands.try_iter().take(16) {
                if let Ok(cmd) = console(&cmd) {
                    let _ = owned.input.write_all(cmd.as_bytes());
                    let _ = owned.input.flush();
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
}
pub(super) fn drain(mut input: impl Read + Send + 'static, sender: mpsc::SyncSender<String>) {
    thread::spawn(move || {
        let mut buf = [0; 4096];
        let mut line = Vec::new();
        loop {
            match input.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    for b in &buf[..n] {
                        if *b == b'\n' {
                            let value = plain_console(&String::from_utf8_lossy(&line));
                            let _ = sender.try_send(value);
                            line.clear();
                        } else if line.len() < 4096 {
                            line.push(*b);
                        }
                    }
                }
            }
        }
    });
}
fn plain_console(line: &str) -> String {
    let mut out = String::new();
    let mut escape = false;
    let mut csi = false;
    for c in line.trim_end().chars() {
        if escape {
            if !csi && c == '[' {
                csi = true;
                continue;
            }
            if !csi || ('\u{40}'..='\u{7e}').contains(&c) {
                escape = false;
                csi = false;
            }
            continue;
        }
        if c == '\u{1b}' {
            escape = true;
            continue;
        }
        if !c.is_control() || c == '\t' {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn console_strips_terminal_colors_without_damaging_chinese() {
        assert_eq!(
            plain_console("\u{1b}[32m世界已保存\u{1b}[0m\r"),
            "世界已保存"
        );
        assert_eq!(plain_console("[INFO] 你好\t世界"), "[INFO] 你好\t世界");
    }
    #[test]
    fn rejects_unaccepted_eula_bad_ports_and_multiline_commands() {
        let c = Cancellation::default();
        assert!(
            create(Path::new("missing"), "x", "26.3", 25565, false, &c)
                .unwrap_err()
                .contains("EULA")
        );
        assert!(details("x", "26.3", 0).is_err());
        assert!(console("say hi\nstop").is_err());
        assert_eq!(console(" list ").unwrap(), "list\n");
    }
    #[test]
    fn rejects_nonofficial_server_artifact() {
        let v = serde_json::json!({"downloads":{"server":{"url":"https://example.org/server.jar","sha1":"a".repeat(40),"size":12}}});
        assert!(artifact(&v).is_err());
    }
    #[test]
    fn replaced_server_directory_is_rejected_before_network_or_process_start() {
        let app =
            std::env::temp_dir().join(format!("amberl-server-identity-{}", new_id().unwrap()));
        let id = new_id().unwrap();
        let root = app.join("servers").join(&id);
        fs::create_dir_all(&root).unwrap();
        let object = DirectoryGuard::open(&root).unwrap().id.clone();
        let mut doc = Document::<Data>::open(&app, "servers-v1.json").unwrap();
        doc.data.servers.push(Server {
            id: id.clone(),
            name: "Test".into(),
            version: "26.3".into(),
            port: 25565,
            loader: None,
            archived: false,
            object,
        });
        doc.save().unwrap();
        drop(doc);
        fs::rename(&root, app.join("retained-original")).unwrap();
        fs::create_dir(&root).unwrap();
        let (_, rx) = mpsc::channel();
        assert!(
            run(
                &app,
                &id,
                Path::new("missing.exe"),
                2048,
                &Cancellation::default(),
                &rx,
                |_| {}
            )
            .unwrap_err()
            .contains("身份")
        );
        assert!(app.join("retained-original").is_dir());
    }
    #[test]
    fn linked_world_files_are_never_opened_for_a_server() {
        let app = std::env::temp_dir().join(format!("amberl-server-links-{}", new_id().unwrap()));
        fs::create_dir(&app).unwrap();
        let source = app.join("outside");
        fs::write(&source, b"retain").unwrap();
        let world = app.join("world");
        fs::create_dir(&world).unwrap();
        fs::hard_link(&source, world.join("level.dat")).unwrap();
        assert!(guard_tree(&world, 0, &mut 0, &mut vec![]).is_err());
        assert_eq!(fs::read(source).unwrap(), b"retain");
    }
}
