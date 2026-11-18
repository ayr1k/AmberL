// SPDX-License-Identifier: MPL-2.0
//! Microsoft device flow -> Xbox -> XSTS -> Minecraft -> entitlement/profile.
//! No raw response, password, credential URL or token is logged or returned by Debug.
pub mod http;
use crate::{MICROSOFT_CLIENT_ID, MICROSOFT_SCOPE, Result, task::Cancellation};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(transparent)]
pub(crate) struct Secret(String);
impl Secret {
    pub(crate) fn new(s: String) -> Result<Self> {
        if s.is_empty() || s.len() > 32000 || !s.bytes().all(|c| c.is_ascii_graphic()) {
            return Err("Invalid authentication credential".into());
        }
        Ok(Self(s))
    }
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Endpoint {
    Device,
    Token,
    Xbox,
    Xsts,
    Minecraft,
    Entitlements,
    Profile,
}
impl Endpoint {
    fn destination(self) -> (&'static str, &'static str) {
        match self {
            Self::Device => (
                "login.microsoftonline.com",
                "/consumers/oauth2/v2.0/devicecode",
            ),
            Self::Token => ("login.microsoftonline.com", "/consumers/oauth2/v2.0/token"),
            Self::Xbox => ("user.auth.xboxlive.com", "/user/authenticate"),
            Self::Xsts => ("xsts.auth.xboxlive.com", "/xsts/authorize"),
            Self::Minecraft => ("api.minecraftservices.com", "/launcher/login"),
            Self::Entitlements => ("api.minecraftservices.com", "/entitlements/mcstore"),
            Self::Profile => ("api.minecraftservices.com", "/minecraft/profile"),
        }
    }
    fn is_get(self) -> bool {
        matches!(self, Self::Entitlements | Self::Profile)
    }
}
pub struct Request<'a> {
    pub endpoint: Endpoint,
    pub body: &'a [u8],
    pub(crate) bearer: Option<&'a Secret>,
}
pub struct Reply {
    pub status: u32,
    pub body: Zeroizing<Vec<u8>>,
}
pub trait Transport {
    fn request(&self, request: Request<'_>, cancel: &Cancellation) -> Result<Reply>;
}

fn decode<T: serde::de::DeserializeOwned>(reply: &Reply) -> Result<T> {
    serde_json::from_slice(&reply.body)
        .map_err(|_| "认证响应格式无效 / Invalid authentication response".into())
}
fn form(pairs: &[(&str, &str)]) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(
        form_urlencoded::Serializer::new(String::new())
            .extend_pairs(pairs.iter().copied())
            .finish()
            .into_bytes(),
    )
}
fn json<T: Serialize>(value: &T) -> Result<Zeroizing<Vec<u8>>> {
    serde_json::to_vec(value)
        .map(Zeroizing::new)
        .map_err(|_| "Cannot encode authentication request".into())
}
fn check(reply: &Reply, endpoint: Endpoint) -> Result<()> {
    if reply.status == 200 {
        return Ok(());
    }
    // Only bounded, known error categories leave this layer; never echo provider messages.
    #[derive(Deserialize)]
    struct Error {
        #[serde(default)]
        error: String,
        #[serde(rename = "XErr")]
        xerr: Option<u64>,
    }
    let reason = decode::<Error>(reply)
        .ok()
        .map(|e| match e.error.as_str() {
            "invalid_grant" | "interaction_required" => "登录已失效，请重新登录 / Sign in again",
            "access_denied" | "authorization_declined" => "登录被取消或拒绝 / Sign-in declined",
            "invalid_client" | "unauthorized_client" => {
                "应用认证配置被拒绝 / Application authentication rejected"
            }
            _ => match e.xerr {
                Some(2148916233) => "需要先创建 Xbox 个人资料 / Xbox profile required",
                Some(2148916238) => {
                    "需要检查 Xbox 家庭账户设置 / Xbox family settings require attention"
                }
                _ => "认证服务拒绝请求 / Authentication service rejected request",
            },
        })
        .unwrap_or("认证服务返回无效响应 / Authentication service returned an invalid response");
    Err(format!("{reason} ({endpoint:?}, HTTP {})", reply.status))
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub name: String,
}
impl Profile {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.id.len() != 32
            || !self.id.bytes().all(|c| c.is_ascii_hexdigit())
            || !(1..=16).contains(&self.name.len())
            || !self
                .name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err("Invalid Minecraft profile".into());
        }
        Ok(())
    }
}

pub struct Identity {
    pub profile: Profile,
    pub(crate) token: Secret,
    pub(crate) xuid: String,
    pub(crate) expires: Instant,
}
impl Identity {
    pub fn is_current(&self) -> bool {
        Instant::now() < self.expires
    }
}

#[derive(Deserialize)]
struct OAuth {
    access_token: Secret,
    refresh_token: Option<Secret>,
}
impl OAuth {
    fn validate(&self) -> Result<()> {
        Secret::new(self.access_token.expose().into())?;
        if let Some(r) = &self.refresh_token {
            Secret::new(r.expose().into())?;
        }
        Ok(())
    }
}

#[derive(Serialize)]
pub struct DevicePrompt {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}
pub struct DeviceFlow {
    code: Secret,
    prompt: DevicePrompt,
    deadline: Instant,
    next: Instant,
    interval: Duration,
    finished: bool,
}
pub enum Poll {
    Waiting,
    Authorized(Authorization),
}
pub struct Authorization {
    pub identity: Identity,
    pub(crate) refresh: Secret,
}
impl DeviceFlow {
    pub fn prompt(&self) -> &DevicePrompt {
        &self.prompt
    }
    pub fn begin(transport: &impl Transport, cancel: &Cancellation) -> Result<Self> {
        let body = form(&[
            ("client_id", MICROSOFT_CLIENT_ID),
            ("scope", MICROSOFT_SCOPE),
        ]);
        let reply = transport.request(
            Request {
                endpoint: Endpoint::Device,
                body: &body,
                bearer: None,
            },
            cancel,
        )?;
        check(&reply, Endpoint::Device)?;
        #[derive(Deserialize)]
        struct Device {
            device_code: Secret,
            user_code: String,
            verification_uri: String,
            expires_in: u64,
            interval: Option<u64>,
        }
        let d: Device = decode(&reply)?;
        Secret::new(d.device_code.expose().into())?;
        if !matches!(
            d.verification_uri.as_str(),
            "https://www.microsoft.com/link"
                | "https://microsoft.com/link"
                | "https://microsoft.com/devicelogin"
                | "https://www.microsoft.com/devicelogin"
        ) || d.user_code.is_empty()
            || d.user_code.len() > 32
            || !d
                .user_code
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            || !(1..=1800).contains(&d.expires_in)
            || !(1..=60).contains(&d.interval.unwrap_or(5))
        {
            return Err("Invalid device sign-in instructions".into());
        }
        let now = Instant::now();
        let interval = Duration::from_secs(d.interval.unwrap_or(5).max(5));
        Ok(Self {
            code: d.device_code,
            prompt: DevicePrompt {
                user_code: d.user_code,
                verification_uri: d.verification_uri,
                expires_in: d.expires_in,
            },
            deadline: now + Duration::from_secs(d.expires_in),
            next: now + interval,
            interval,
            finished: false,
        })
    }
    pub fn poll(&mut self, transport: &impl Transport, cancel: &Cancellation) -> Result<Poll> {
        if cancel.check().is_err() {
            self.finished = true;
            return Err("登录已取消 / Sign-in cancelled".into());
        }
        if self.finished || Instant::now() >= self.deadline {
            self.finished = true;
            return Err("设备登录已结束或过期 / Device sign-in ended or expired".into());
        }
        if Instant::now() < self.next {
            return Ok(Poll::Waiting);
        }
        self.next = Instant::now() + self.interval;
        let body = form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", MICROSOFT_CLIENT_ID),
            ("device_code", self.code.expose()),
        ]);
        let reply = transport.request(
            Request {
                endpoint: Endpoint::Token,
                body: &body,
                bearer: None,
            },
            cancel,
        )?;
        if reply.status == 400 {
            #[derive(Deserialize)]
            struct Error {
                error: String,
            }
            if let Ok(e) = decode::<Error>(&reply) {
                if e.error == "authorization_pending" {
                    return Ok(Poll::Waiting);
                }
                if e.error == "slow_down" {
                    self.interval += Duration::from_secs(5);
                    self.next = Instant::now() + self.interval;
                    return Ok(Poll::Waiting);
                }
            }
        }
        self.finished = true;
        check(&reply, Endpoint::Token)?;
        let tokens: OAuth = decode(&reply)?;
        tokens.validate()?;
        let refresh = tokens
            .refresh_token
            .ok_or("Microsoft did not issue a refresh credential")?;
        let identity = minecraft(&tokens.access_token, transport, cancel)?;
        Ok(Poll::Authorized(Authorization { identity, refresh }))
    }
}

pub(crate) fn refresh(
    refresh: &Secret,
    transport: &impl Transport,
    cancel: &Cancellation,
) -> Result<(Secret, Secret)> {
    let body = form(&[
        ("grant_type", "refresh_token"),
        ("client_id", MICROSOFT_CLIENT_ID),
        ("refresh_token", refresh.expose()),
        ("scope", MICROSOFT_SCOPE),
    ]);
    let reply = transport.request(
        Request {
            endpoint: Endpoint::Token,
            body: &body,
            bearer: None,
        },
        cancel,
    )?;
    check(&reply, Endpoint::Token)?;
    let tokens: OAuth = decode(&reply)?;
    tokens.validate()?;
    Ok((
        tokens.access_token,
        tokens.refresh_token.unwrap_or_else(|| refresh.clone()),
    ))
}

pub(crate) fn minecraft(
    access: &Secret,
    transport: &impl Transport,
    cancel: &Cancellation,
) -> Result<Identity> {
    #[derive(Deserialize)]
    struct Xbox {
        #[serde(rename = "Token")]
        token: Secret,
    }
    let ticket = Zeroizing::new(format!("d={}", access.expose()));
    let body = json(
        &serde_json::json!({"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":ticket.as_str()},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"}),
    )?;
    let reply = transport.request(
        Request {
            endpoint: Endpoint::Xbox,
            body: &body,
            bearer: None,
        },
        cancel,
    )?;
    check(&reply, Endpoint::Xbox)?;
    let xbox: Xbox = decode(&reply)?;
    Secret::new(xbox.token.expose().into())?;
    let body = json(
        &serde_json::json!({"Properties":{"SandboxId":"RETAIL","UserTokens":[xbox.token.expose()]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"}),
    )?;
    let reply = transport.request(
        Request {
            endpoint: Endpoint::Xsts,
            body: &body,
            bearer: None,
        },
        cancel,
    )?;
    check(&reply, Endpoint::Xsts)?;
    #[derive(Deserialize)]
    struct Claims {
        #[serde(default)]
        xui: Vec<User>,
    }
    #[derive(Deserialize)]
    struct User {
        uhs: String,
        #[serde(default)]
        xid: String,
    }
    #[derive(Deserialize)]
    struct Xsts {
        #[serde(rename = "Token")]
        token: Secret,
        #[serde(rename = "DisplayClaims")]
        claims: Claims,
    }
    let xsts: Xsts = decode(&reply)?;
    Secret::new(xsts.token.expose().into())?;
    let user = xsts
        .claims
        .xui
        .first()
        .filter(|_| xsts.claims.xui.len() == 1)
        .ok_or("Xbox identity is unavailable")?;
    if user.uhs.is_empty()
        || user.uhs.len() > 64
        || !user.uhs.bytes().all(|c| c.is_ascii_digit())
        || user.xid.len() > 64
        || !user.xid.bytes().all(|c| c.is_ascii_digit())
    {
        return Err("Invalid Xbox identity".into());
    }
    let xtoken = Zeroizing::new(format!("XBL3.0 x={};{}", user.uhs, xsts.token.expose()));
    let body = json(&serde_json::json!({"xtoken":xtoken.as_str(),"platform":"PC_LAUNCHER"}))?;
    let reply = transport.request(
        Request {
            endpoint: Endpoint::Minecraft,
            body: &body,
            bearer: None,
        },
        cancel,
    )?;
    check(&reply, Endpoint::Minecraft)?;
    #[derive(Deserialize)]
    struct Minecraft {
        access_token: Secret,
        expires_in: u64,
    }
    let mc: Minecraft = decode(&reply)?;
    Secret::new(mc.access_token.expose().into())?;
    if !(60..=86400).contains(&mc.expires_in) {
        return Err("Invalid Minecraft credential lifetime".into());
    }
    let expires = Instant::now() + Duration::from_secs(mc.expires_in - 30);
    let reply = transport.request(
        Request {
            endpoint: Endpoint::Entitlements,
            body: &[],
            bearer: Some(&mc.access_token),
        },
        cancel,
    )?;
    check(&reply, Endpoint::Entitlements)?;
    #[derive(Deserialize)]
    struct Item {
        name: String,
    }
    #[derive(Deserialize)]
    struct Entitlements {
        items: Vec<Item>,
    }
    let entitlements: Entitlements = decode(&reply)?;
    if !entitlements
        .items
        .iter()
        .any(|i| matches!(i.name.as_str(), "game_minecraft" | "product_minecraft"))
    {
        return Err("未确认 Minecraft Java 所有权 / Minecraft Java ownership not verified".into());
    }
    let reply = transport.request(
        Request {
            endpoint: Endpoint::Profile,
            body: &[],
            bearer: Some(&mc.access_token),
        },
        cancel,
    )?;
    check(&reply, Endpoint::Profile)?;
    // Provider profiles contain optional skins/capes; keep only validated identity fields.
    #[derive(Deserialize)]
    struct ProviderProfile {
        id: String,
        name: String,
    }
    let p: ProviderProfile = decode(&reply)?;
    let profile = Profile {
        id: p.id,
        name: p.name,
    };
    profile.validate()?;
    cancel.check()?;
    Ok(Identity {
        profile,
        token: mc.access_token,
        xuid: user.xid.clone(),
        expires,
    })
}

#[cfg(test)]
pub(crate) mod tests;
