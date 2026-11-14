// SPDX-License-Identifier: MPL-2.0
//! Independent, pinned frpc. Tokens use DPAPI and an environment-rendered template.
use super::{Address, room};
use crate::{
    Result,
    auth::Secret,
    download::{Algorithm, Artifact},
    registry::{DirectoryGuard, new_id, options},
    state::Document,
    task::Cancellation,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
use zeroize::Zeroizing;
pub const VERSION: &str = "0.72.0";
const EXE_HASH: &str = "b8580b916ecb2e27ecc348bc67bc8b93b0e65266f85fd5d703e2f2f69862b089";
const FOLDER: &str = "frp-components";
pub fn installed(app: &Path) -> bool {
    app.join(FOLDER).join("frpc-0.72.0.exe").is_file()
}
pub fn install(app: &Path, c: &Cancellation) -> Result<()> {
    room::install_bundle(app,FOLDER,&[("frpc.exe","frpc-0.72.0.exe",EXE_HASH),
        ("LICENSE","LICENSE.txt","c6596eb7be8581c18be736c846fb9173b69eccf6ef94c5135893ec56bd92ba08")],
        &Artifact { url:"https://github.com/fatedier/frp/releases/download/v0.72.0/frp_0.72.0_windows_amd64.zip".into(),
            size:14472216,algorithm:Algorithm::Sha256,digest:"5197c1b69564089cdc229fc1260a0e25db1e722b1b7db45fe1542c3c43edf9ae".into() },c)
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub server: Address,
    pub local_port: u16,
    pub remote_port: u16,
    pub certificate: PathBuf,
}
impl Config {
    fn validate(&self) -> Result<()> {
        if Address::parse(&self.server.text())? != self.server
            || self.local_port < 1024
            || self.remote_port < 1024
            || self.certificate.as_os_str().is_empty()
        {
            return Err("请填写 FRP 节点、1024–65535 端口及节点 CA 证书".into());
        }
        Ok(())
    }
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    schema: u32,
    config: Option<Config>,
    certificate: Vec<u8>,
    token: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Credential {
    server: Address,
    certificate_sha256: String,
    token: Secret,
}
fn credential(record: &Stored) -> Result<Secret> {
    let config = validate(record)?;
    let plaintext = crate::secure::protect_frp(&record.token, true)?;
    let secret: Credential = serde_json::from_slice(&plaintext).map_err(|_| "FRP 凭据损坏")?;
    if secret.server != config.server
        || secret.certificate_sha256 != room::hash(std::io::Cursor::new(&record.certificate))?
    {
        return Err("FRP 凭据与节点或证书不匹配，请重新输入令牌".into());
    }
    Ok(secret.token)
}
fn validate(record: &Stored) -> Result<&Config> {
    let config = record.config.as_ref().ok_or("请先保存 FRP 配置")?;
    config.validate()?;
    if record.schema != 1
        || record.certificate.len() > 65536
        || !record
            .certificate
            .starts_with(b"-----BEGIN CERTIFICATE-----")
        || record.token.is_empty()
        || record.token.len() > 65536
    {
        return Err("FRP 配置损坏，原文件已保留".into());
    }
    Ok(config)
}
pub fn load(app: &Path) -> Result<Option<Config>> {
    let doc = Document::<Stored>::open(app, "frp-v1.json")?;
    if doc.data.config.is_none() && doc.data.schema == 0 {
        return Ok(None);
    }
    Ok(Some(validate(&doc.data)?.clone()))
}
pub fn save(app: &Path, config: &Config, token: String) -> Result<()> {
    let token = Zeroizing::new(token);
    config.validate()?;
    let certificate = crate::paths::ordinary(&config.certificate)?;
    let _parent = DirectoryGuard::open(certificate.parent().ok_or("请选择证书")?)?;
    let mut file = room::pin(&certificate)?;
    let mut bytes = vec![];
    (&mut file)
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取证书")?;
    if bytes.len() > 65536 || !bytes.starts_with(b"-----BEGIN CERTIFICATE-----") {
        return Err("请选择 PEM 格式的节点 CA 证书".into());
    }
    let mut doc = Document::<Stored>::open(app, "frp-v1.json")?;
    let protected = if token.is_empty() {
        let old = validate(&doc.data)?;
        if old.server != config.server || doc.data.certificate != bytes {
            return Err("节点或证书已变化，请重新输入令牌".into());
        }
        credential(&doc.data)?;
        doc.data.token.clone()
    } else {
        let secret = Credential {
            server: config.server.clone(),
            certificate_sha256: room::hash(std::io::Cursor::new(&bytes))?,
            token: Secret::new(token.to_string())?,
        };
        let encoded = Zeroizing::new(serde_json::to_vec(&secret).map_err(|_| "无法编码 FRP 凭据")?);
        crate::secure::protect_frp(&encoded, false)?.to_vec()
    };
    doc.data = Stored {
        schema: 1,
        config: Some(config.clone()),
        certificate: bytes,
        token: protected,
    };
    doc.save()
}
fn template(config: &Config, name: &str, certificate: &Path) -> Result<String> {
    config.validate()?;
    let data = serde_json::json!({"serverAddr":config.server.host,"serverPort":config.server.port,
        "loginFailExit":false,"auth":{"method":"token","token":"{{ .Envs.AMBERL_FRP_TOKEN_JSON }}"},
        "transport":{"tls":{"enable":true,"trustedCaFile":certificate,"serverName":config.server.host}},
        "log":{"to":"console","level":"error"},
        "proxies":[{"name":name,"type":"tcp","localIP":"127.0.0.1","localPort":config.local_port,"remotePort":config.remote_port}]});
    Ok(serde_json::to_string_pretty(&data)
        .map_err(|_| "无法生成 FRP 配置")?
        .replace(
            "\"{{ .Envs.AMBERL_FRP_TOKEN_JSON }}\"",
            "{{ .Envs.AMBERL_FRP_TOKEN_JSON }}",
        ))
}
pub fn run(
    app: &Path,
    expected: &Config,
    c: &Cancellation,
    mut started: impl FnMut(u32),
) -> Result<i32> {
    let doc = Document::<Stored>::open(app, "frp-v1.json")?;
    if validate(&doc.data)? != expected {
        return Err("FRP 配置已变化，请重新读取".into());
    }
    let token = credential(&doc.data)?;
    let env_token =
        Zeroizing::new(serde_json::to_string(token.expose()).map_err(|_| "无法编码 FRP 凭据")?);
    let parent = DirectoryGuard::open(&app.join(FOLDER))?;
    let exe = parent.root.join("frpc-0.72.0.exe");
    let pinned = room::pin(&exe)?;
    if room::hash(&pinned)? != EXE_HASH {
        return Err("FRP 组件校验失败，未执行".into());
    }
    let session = parent.root.join(format!("session-{}", new_id()?));
    fs::create_dir(&session).map_err(|e| e.to_string())?;
    let session_guard = DirectoryGuard::open(&session)?;
    let write = |name: &str, bytes: &[u8]| -> Result<()> {
        let mut f = options()
            .write(true)
            .create_new(true)
            .open(session.join(name))
            .map_err(|e| e.to_string())?;
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())
    };
    write("ca.pem", &doc.data.certificate)?;
    write(
        "frpc.json",
        template(
            expected,
            &format!("amberl-{}", new_id()?),
            &session_guard.root.join("ca.pem"),
        )?
        .as_bytes(),
    )?;
    let _ca = room::pin(&session.join("ca.pem"))?;
    let _config = room::pin(&session.join("frpc.json"))?;
    drop(doc);
    c.check()?;
    let mut command = Command::new(exe);
    command
        .args(["-c"])
        .arg(session_guard.root.join("frpc.json"))
        .current_dir(&session_guard.root)
        .env("AMBERL_FRP_TOKEN_JSON", env_token.as_str())
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x08000000);
    struct Owned(std::process::Child);
    impl Drop for Owned {
        fn drop(&mut self) {
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    let mut owned = Owned(command.spawn().map_err(|_| "FRP 组件无法启动")?);
    drop(command);
    drop(env_token);
    drop(token);
    started(owned.0.id());
    loop {
        if let Some(status) = owned.0.try_wait().map_err(|_| "无法观察 FRP 进程")? {
            return Ok(status.code().unwrap_or(-1));
        }
        if c.check().is_err() {
            owned.0.kill().map_err(|_| "FRP 进程尚未退出")?;
            owned.0.wait().map_err(|_| "FRP 进程尚未退出")?;
            return Ok(0);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn template_binds_only_selected_local_port_and_keeps_secrets_out() {
        let config = Config {
            server: Address::parse("example.com:7000").unwrap(),
            local_port: 25565,
            remote_port: 25566,
            certificate: "ca.pem".into(),
        };
        let t = template(&config, "amberl-test", Path::new("ca.pem")).unwrap();
        assert!(t.contains("{{ .Envs.AMBERL_FRP_TOKEN_JSON }}"));
        let rendered = t.replace(
            "{{ .Envs.AMBERL_FRP_TOKEN_JSON }}",
            &serde_json::to_string("quote\"\\token").unwrap(),
        );
        let v: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(v["auth"]["token"], "quote\"\\token");
        assert_eq!(v["proxies"][0]["localIP"], "127.0.0.1");
        assert_eq!(v["transport"]["tls"]["enable"], true);
        assert_eq!(v["transport"]["tls"]["serverName"], "example.com");
        assert!(v.get("webServer").is_none());
    }
    #[test]
    fn profile_tokens_are_protected_and_cannot_follow_a_changed_endpoint() {
        let app = std::env::temp_dir().join(format!("amberl-frp-{}", new_id().unwrap()));
        fs::create_dir(&app).unwrap();
        let ca = app.join("qa.pem");
        fs::write(
            &ca,
            b"-----BEGIN CERTIFICATE-----\nTEST\n-----END CERTIFICATE-----",
        )
        .unwrap();
        let mut config = Config {
            server: Address::parse("localhost:7000").unwrap(),
            local_port: 25565,
            remote_port: 25566,
            certificate: ca,
        };
        save(&app, &config, "test-only-frp-secret".into()).unwrap();
        let bytes = fs::read(app.join("frp-v1.json")).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("test-only-frp-secret"));
        assert_eq!(load(&app).unwrap(), Some(config.clone()));
        save(&app, &config, String::new()).unwrap();
        config.server.host = "other.example".into();
        assert!(save(&app, &config, String::new()).is_err());
        let mut document = Document::<Stored>::open(&app, "frp-v1.json").unwrap();
        assert!(credential(&document.data).is_ok());
        document.data.config.as_mut().unwrap().server = config.server;
        assert!(credential(&document.data).is_err());
        drop(document);
        fs::remove_dir_all(app).unwrap();
    }
}
