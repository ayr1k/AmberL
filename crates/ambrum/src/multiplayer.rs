// SPDX-License-Identifier: MPL-2.0
//! Explicit endpoint parsing and bounded Minecraft status queries; never imply login readiness.
pub mod frp;
pub mod room;
pub mod server;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::{
    io::{Cursor, Read, Write},
    net::{IpAddr, TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    pub host: String,
    pub port: u16,
}
impl Address {
    pub fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        if value.is_empty()
            || value.len() > 260
            || value.chars().any(|c| c.is_whitespace() || c.is_control())
            || value.contains(['/', '\\', '@', '?', '#'])
        {
            return Err("请输入主机名或 IP，可附端口；不要输入网址".into());
        }
        let (host, port) = if let Some(v) = value.strip_prefix('[') {
            let (h, tail) = v.split_once(']').ok_or("IPv6 地址需要方括号")?;
            if !matches!(h.parse::<IpAddr>(), Ok(IpAddr::V6(_))) {
                return Err("无效 IPv6 地址".into());
            }
            (
                h,
                if tail.is_empty() {
                    25565
                } else {
                    tail.strip_prefix(':')
                        .ok_or("无效端口")?
                        .parse::<u16>()
                        .map_err(|_| "无效端口")?
                },
            )
        } else {
            let (h, p) = match value.split_once(':') {
                Some((h, p)) => (h, p.parse::<u16>().map_err(|_| "无效端口")?),
                None => (value, 25565),
            };
            if h.len() > 253
                || h.split('.').any(|s| {
                    s.is_empty()
                        || s.len() > 63
                        || s.starts_with('-')
                        || s.ends_with('-')
                        || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
            {
                return Err("无效主机名".into());
            }
            (h, p)
        };
        if port == 0 {
            return Err("端口必须为 1–65535".into());
        }
        Ok(Self {
            host: host.into(),
            port,
        })
    }
    pub fn text(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub version: String,
    pub players: u64,
    pub maximum: u64,
    pub latency_ms: u128,
}
fn varint(mut value: u32, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 127) as u8;
        value >>= 7;
        out.push(if value > 0 { byte | 128 } else { byte });
        if value == 0 {
            break;
        }
    }
}
fn read_varint(input: &mut impl Read) -> Result<u32> {
    let mut out = 0;
    for i in 0..5 {
        let mut b = [0];
        input.read_exact(&mut b).map_err(|_| "服务端响应不完整")?;
        if i == 4 && b[0] > 15 {
            return Err("响应长度无效".into());
        }
        out |= u32::from(b[0] & 127) << (7 * i);
        if b[0] & 128 == 0 {
            return Ok(out);
        }
    }
    Err("响应长度无效".into())
}
fn packet(body: &[u8]) -> Vec<u8> {
    let mut out = vec![];
    varint(body.len() as u32, &mut out);
    out.extend(body);
    out
}
pub fn probe(address: &Address) -> Result<Status> {
    // Parsing again prevents forged deserialized addresses from bypassing validation.
    let address = Address::parse(&address.text())?;
    let started = Instant::now();
    let sockets = (address.host.as_str(), address.port)
        .to_socket_addrs()
        .map_err(|_| "无法解析服务器地址")?;
    let mut stream = None;
    for socket in sockets.take(8) {
        if started.elapsed() > Duration::from_secs(5) {
            break;
        }
        if let Ok(s) = TcpStream::connect_timeout(&socket, Duration::from_millis(650)) {
            stream = Some(s);
            break;
        }
    }
    let mut stream = stream.ok_or("暂时无法连接；这不代表服务器已关闭")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    let mut hello = vec![0];
    varint(u32::MAX, &mut hello);
    varint(address.host.len() as u32, &mut hello);
    hello.extend(address.host.as_bytes());
    hello.extend(address.port.to_be_bytes());
    hello.push(1);
    stream
        .write_all(&packet(&hello))
        .and_then(|_| stream.write_all(&[1, 0]))
        .map_err(|_| "状态请求失败")?;
    let size = read_varint(&mut stream)? as usize;
    if !(3..=128 * 1024).contains(&size) {
        return Err("服务器状态响应超出限制".into());
    }
    let mut bytes = vec![0; size];
    let mut offset = 0;
    while offset < size {
        if started.elapsed() > Duration::from_secs(8) {
            return Err("状态查询超时".into());
        }
        let n = stream
            .read(&mut bytes[offset..])
            .map_err(|_| "状态查询超时或中断")?;
        if n == 0 {
            return Err("状态响应中断".into());
        }
        offset += n;
    }
    parse_status(&bytes, started.elapsed().as_millis())
}
fn parse_status(bytes: &[u8], latency_ms: u128) -> Result<Status> {
    let mut cursor = Cursor::new(bytes);
    if read_varint(&mut cursor)? != 0 {
        return Err("不是 Minecraft 状态响应".into());
    }
    let size = read_varint(&mut cursor)? as usize;
    let pos = cursor.position() as usize;
    if size != bytes.len() - pos {
        return Err("状态响应长度不符".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes[pos..]).map_err(|_| "状态响应格式无效")?;
    let version = value["version"]["name"].as_str().ok_or("缺少游戏版本")?;
    if version.len() > 256 || version.chars().any(char::is_control) {
        return Err("状态版本无效".into());
    }
    Ok(Status {
        version: version.into(),
        players: value["players"]["online"].as_u64().ok_or("缺少玩家数量")?,
        maximum: value["players"]["max"].as_u64().ok_or("缺少玩家上限")?,
        latency_ms,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn addresses_are_not_urls_or_arguments() {
        for bad in [
            "",
            "a:0",
            "a:70000",
            "a\n--demo",
            "https://a",
            "a/b",
            "x@y:1",
            "::1",
            "a..b",
            "-host",
        ] {
            assert!(Address::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(Address::parse("[::1]:25565").unwrap().text(), "[::1]:25565");
        assert_eq!(Address::parse("example.org").unwrap().port, 25565);
    }
    #[test]
    fn status_requires_protocol_and_bounded_complete_json() {
        assert!(read_varint(&mut Cursor::new([255; 5])).is_err());
        assert!(parse_status(b"\0\x05{}", 1).is_err());
        let data = br#"{"version":{"name":"26.3"},"players":{"online":1,"max":8}}"#;
        let mut p = vec![0];
        varint(data.len() as u32, &mut p);
        p.extend(data);
        assert_eq!(parse_status(&p, 5).unwrap().players, 1);
    }
}
