// SPDX-License-Identifier: MPL-2.0
//! EasyTier no-TUN rooms. Secrets remain in memory and are never put on a command line.
use super::Address;
use crate::{
    Result,
    auth::Secret,
    download::{Algorithm, Artifact, Cache, windows::Hash},
    registry::{DirectoryGuard, info, new_id, options},
    task::Cancellation,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    os::windows::{fs::OpenOptionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom},
    Storage::FileSystem::FILE_SHARE_READ,
};
use zeroize::Zeroizing;
pub const VERSION: &str = "2.6.4";
const EXE_HASH: &str = "da7eb2d24b5416f3d3407636949e964a0750e3f9dc53a828cb6799a57ead445d";
const COMPONENTS: [(&str, &str, &str); 3] = [
    ("easytier-core.exe", "easytier-2.6.4.exe", EXE_HASH),
    (
        "Packet.dll",
        "Packet.dll",
        "c7c03a87eac7243ccbe331554624b18803010b740e311fc8cfddb573096eacac",
    ),
    (
        "wintun.dll",
        "wintun.dll",
        "e5da8447dc2c320edc0fc52fa01885c103de8c118481f683643cacc3220dafce",
    ),
];
fn now() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间无效")?
        .as_secs())
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invite {
    schema: u32,
    name: String,
    secret: Secret,
    pub port: u16,
    pub peer: String,
    expires: u64,
}
fn peer(value: &str) -> Result<String> {
    let (scheme, address) = value
        .split_once("://")
        .ok_or("节点格式应为 tcp://主机:端口")?;
    if !matches!(scheme, "tcp" | "udp" | "ws" | "wss") || !address.contains(':') {
        return Err("节点需要明确的协议和端口".into());
    }
    let address = Address::parse(address)?;
    Ok(format!("{scheme}://{}", address.text()))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
impl Invite {
    pub fn host(port: u16, node: &str) -> Result<Self> {
        if port < 1024 {
            return Err("共享端口必须为 1024–65535".into());
        }
        let mut bytes = Zeroizing::new([0u8; 32]);
        if unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                bytes.as_mut_ptr(),
                32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        } < 0
        {
            return Err("无法生成安全房间密钥".into());
        }
        Ok(Self {
            schema: 1,
            name: format!("amberl-{}", new_id()?),
            secret: Secret::new(hex(bytes.as_ref()))?,
            port,
            peer: peer(node)?,
            expires: now()? + 86400,
        })
    }
    fn validate(&self) -> Result<()> {
        let time = now()?;
        if self.schema != 1
            || self.port < 1024
            || self.expires <= time
            || self.expires > time + 86400
            || !self
                .name
                .strip_prefix("amberl-")
                .is_some_and(|n| n.len() == 32 && n.bytes().all(|b| b.is_ascii_hexdigit()))
            || self.secret.expose().len() != 64
            || !self.secret.expose().bytes().all(|b| b.is_ascii_hexdigit())
            || peer(&self.peer)? != self.peer
        {
            return Err("房间邀请无效或已过期".into());
        }
        Ok(())
    }
    pub fn code(&self) -> Result<String> {
        self.validate()?;
        let bytes =
            Zeroizing::new(serde_json::to_vec(self).map_err(|_| "Cannot encode invitation")?);
        Ok(format!("amberl-room:{}", hex(&bytes)))
    }
    pub fn parse(value: &str) -> Result<Self> {
        let value = value
            .trim()
            .strip_prefix("amberl-room:")
            .ok_or("这不是 AmberL 房间邀请")?;
        if value.len() > 4096
            || !value.len().is_multiple_of(2)
            || !value.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("房间邀请格式无效".into());
        }
        let bytes = Zeroizing::new(
            (0..value.len())
                .step_by(2)
                .map(|i| {
                    u8::from_str_radix(&value[i..i + 2], 16)
                        .map_err(|_| "Invalid invitation".to_string())
                })
                .collect::<Result<Vec<_>>>()?,
        );
        let invite: Self = serde_json::from_slice(&bytes).map_err(|_| "房间邀请格式无效")?;
        invite.validate()?;
        Ok(invite)
    }
}
fn component_path(app: &Path) -> PathBuf {
    app.join("network-components").join("easytier-2.6.4.exe")
}
pub fn installed(app: &Path) -> bool {
    COMPONENTS
        .iter()
        .all(|(_, name, _)| app.join("network-components").join(name).is_file())
}
pub(crate) fn hash(mut input: impl Read) -> Result<String> {
    let mut h = Hash::new(Algorithm::Sha256)?;
    let mut b = [0; 65536];
    loop {
        let n = input.read(&mut b).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&b[..n])?;
    }
    h.finish()
}
pub fn install(app: &Path, cancel: &Cancellation) -> Result<()> {
    let a=Artifact{url:"https://github.com/EasyTier/EasyTier/releases/download/v2.6.4/easytier-windows-x86_64-v2.6.4.zip".into(),size:32657720,algorithm:Algorithm::Sha256,digest:"27af91e270e554709b048bd32327fefd2dfce5062ae1e8701af7550c6f525f84".into()};
    install_bundle(app, "network-components", &COMPONENTS, &a, cancel)
}
pub(crate) fn install_bundle(
    app: &Path,
    folder: &str,
    components: &[(&str, &str, &str)],
    a: &Artifact,
    cancel: &Cancellation,
) -> Result<()> {
    crate::paths::name(folder)?;
    let _app = DirectoryGuard::open(app)?;
    let parent = app.join(folder);
    match fs::create_dir(&parent) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }
    let _parent = DirectoryGuard::open(&parent)?;
    for &(_, name, expected) in components {
        crate::paths::name(name)?;
        let target = parent.join(name);
        if target.exists() && hash(pin(&target)?)? != expected {
            return Err("联机组件校验失败；原文件已保留".into());
        }
    }
    if components
        .iter()
        .all(|(_, name, _)| parent.join(name).is_file())
    {
        return Ok(());
    }
    let cache = Cache::open(app)?;
    cache.fetch(a, &crate::download::windows::GithubRelease, cancel, |_| {})?;
    let f = cache.verified_file(a, cancel)?;
    let mut archive = zip::ZipArchive::new(f).map_err(|_| "Invalid component ZIP")?;
    if archive.len() > 128 {
        return Err("Component archive entry limit".into());
    }
    for &(source, name, expected) in components {
        let target = parent.join(name);
        if target.exists() {
            continue;
        }
        let mut entry = None;
        for i in 0..archive.len() {
            let e = archive.by_index(i).map_err(|_| "Invalid component entry")?;
            if e.name()
                .map_err(|_| "Invalid entry name")?
                .rsplit('/')
                .next()
                == Some(source)
            {
                if entry.is_some() || e.size() > 128 * 1024 * 1024 {
                    return Err("Ambiguous component executable".into());
                }
                entry = Some(i);
            }
        }
        let mut input = archive
            .by_index(entry.ok_or("Missing component file")?)
            .map_err(|_| "Cannot read component")?;
        let stage = parent.join(format!("{}.tmp", new_id()?));
        let mut output = options()
            .write(true)
            .create_new(true)
            .open(&stage)
            .map_err(|e| e.to_string())?;
        let mut h = Hash::new(Algorithm::Sha256)?;
        let mut buf = [0; 65536];
        let mut total = 0;
        loop {
            cancel.check()?;
            let n = input
                .read(&mut buf)
                .map_err(|_| "Component extraction failed")?;
            if n == 0 {
                break;
            }
            total += n;
            if total > 128 * 1024 * 1024 {
                return Err("Component extraction limit".into());
            }
            h.update(&buf[..n])?;
            output.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
        if h.finish()? != expected {
            return Err("Component file checksum mismatch".into());
        }
        output.sync_all().map_err(|e| e.to_string())?;
        drop(output);
        cancel.check()?;
        // rename does not replace an existing Windows destination.
        fs::rename(stage, target).map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub(crate) fn pin(path: &Path) -> Result<fs::File> {
    let f = options()
        .share_mode(FILE_SHARE_READ)
        .open(path)
        .map_err(|_| "请先下载联机组件")?;
    if info(&f)?.nNumberOfLinks != 1 || !f.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Linked component rejected".into());
    }
    Ok(f)
}
fn args(invite: &Invite, guest: Option<u16>, rpc: u16) -> Result<Vec<String>> {
    invite.validate()?;
    if guest.is_some_and(|p| p < 1024 || p == rpc) {
        return Err("本地端口无效".into());
    }
    let ip = if guest.is_some() {
        let id = new_id()?;
        format!(
            "10.144.{}.{}",
            u8::from_str_radix(&id[..2], 16).unwrap() % 254 + 1,
            u8::from_str_radix(&id[2..4], 16).unwrap() % 253 + 2
        )
    } else {
        "10.144.0.1".into()
    };
    let mut args = vec![
        "--bind-device".into(),
        "false".into(),
        "--no-tun".into(),
        "--ipv4".into(),
        ip,
        "--network-name".into(),
        invite.name.clone(),
        "--peers".into(),
        invite.peer.clone(),
        "--no-listener".into(),
        "--private-mode".into(),
        "true".into(),
        "--rpc-portal".into(),
        format!("127.0.0.1:{rpc}"),
        "--hostname".into(),
        if guest.is_some() {
            "AmberL-guest".into()
        } else {
            "AmberL-host".into()
        },
        "--console-log-level".into(),
        "off".into(),
        "--tcp-whitelist".into(),
    ];
    if guest.is_none() {
        args.push(invite.port.to_string());
    } else {
        // In 2.6.4 an empty list disables filtering. Port zero denies usable inbound
        // TCP ports while the component's stateful rules permit established replies.
        args.push("0".into());
    }
    args.push("--udp-whitelist".into());
    args.push("0".into());
    if let Some(port) = guest {
        args.extend([
            "--port-forward".into(),
            format!("tcp://127.0.0.1:{port}/10.144.0.1:{}", invite.port),
        ]);
    }
    Ok(args)
}
pub fn run(
    app: &Path,
    invite: Invite,
    guest: Option<u16>,
    cancel: &Cancellation,
    mut started: impl FnMut(u32),
) -> Result<i32> {
    invite.validate()?;
    let path = component_path(app);
    let _parent = DirectoryGuard::open(path.parent().ok_or("Missing component parent")?)?;
    let file = pin(&path)?;
    if hash(&file)? != EXE_HASH {
        return Err("联机组件校验失败；未执行".into());
    }
    let mut dependencies = Vec::new();
    for (_, name, expected) in COMPONENTS.iter().skip(1) {
        let f = pin(&app.join("network-components").join(name))?;
        if hash(&f)? != *expected {
            return Err("联机依赖校验失败；未执行".into());
        }
        dependencies.push(f);
    }
    let rpc = TcpListener::bind(("127.0.0.1", 0)).map_err(|_| "无法分配本地管理端口")?;
    let rpc_port = rpc.local_addr().map_err(|e| e.to_string())?.port();
    let guest_bind = guest
        .map(|port| {
            TcpListener::bind(("127.0.0.1", port)).map_err(|_| "本地加入端口已占用".to_string())
        })
        .transpose()?;
    let args = args(&invite, guest, rpc_port)?;
    drop(rpc);
    drop(guest_bind);
    cancel.check()?;
    let mut command = Command::new(&path);
    command
        .args(args)
        .current_dir(app)
        .env("ET_NETWORK_SECRET", invite.secret.expose())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x08000000);
    struct OwnedTunnel(std::process::Child);
    impl Drop for OwnedTunnel {
        fn drop(&mut self) {
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    let mut owned = OwnedTunnel(command.spawn().map_err(|_| "联机组件无法启动")?);
    let child = &mut owned.0;
    drop(command);
    started(child.id());
    loop {
        if let Some(status) = child.try_wait().map_err(|_| "无法观察联机进程")? {
            return Ok(status.code().unwrap_or(-1));
        }
        if cancel.check().is_err() || now()? >= invite.expires {
            child
                .kill()
                .map_err(|_| "联机进程仍在退出，不能报告已停止")?;
            child.wait().map_err(|_| "联机进程尚未退出")?;
            return Ok(0);
        }
        thread::sleep(Duration::from_millis(100));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invite_bounds_expiry_and_secret_not_in_arguments() {
        let i = Invite::host(25565, "tcp://127.0.0.1:11010").unwrap();
        let code = i.code().unwrap();
        let mut j = Invite::parse(&code).unwrap();
        let a = args(&j, Some(25567), 30000).unwrap();
        assert!(!a.iter().any(|a| a.contains(j.secret.expose())));
        assert!(a.contains(&"tcp://127.0.0.1:25567/10.144.0.1:25565".into()));
        assert!(a.windows(2).any(|v| v == ["--tcp-whitelist", "0"]));
        assert!(a.windows(2).any(|v| v == ["--udp-whitelist", "0"]));
        let host = args(&j, None, 30000).unwrap();
        assert!(host.windows(2).any(|v| v == ["--tcp-whitelist", "25565"]));
        assert!(host.windows(2).any(|v| v == ["--udp-whitelist", "0"]));
        j.expires = 0;
        assert!(j.code().is_err());
        assert!(Invite::parse("amber-room:abc").is_err());
        assert!(Invite::host(25565, "tcp://host:10/path").is_err());
        assert!(Invite::parse(&"x".repeat(8000)).is_err());
    }
}
