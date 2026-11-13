// SPDX-License-Identifier: MPL-2.0
//! Windows adapters only. TLS and hashing use the OS, not custom implementations.
use super::{Algorithm, Transport};
use crate::Result;
use std::{
    ffi::c_void,
    io::{self, Read},
    ptr::{null, null_mut},
};
use windows_sys::Win32::{Networking::WinHttp::*, Security::Cryptography::*};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn checked(ok: i32) -> Result<()> {
    if ok == 0 {
        Err(format!(
            "HTTPS 请求失败，可重试 / HTTPS failed (Windows {})",
            io::Error::last_os_error().raw_os_error().unwrap_or(0)
        ))
    } else {
        Ok(())
    }
}
struct Handle(*mut c_void);
impl Handle {
    fn new(h: *mut c_void) -> Result<Self> {
        checked(i32::from(!h.is_null()))?;
        Ok(Self(h))
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}

pub struct Https;
struct Url {
    host: Vec<u16>,
    path: Vec<u16>,
    port: u16,
}
impl Https {
    pub fn validate(url: &str) -> Result<()> {
        Self::parse(url).map(|_| ())
    }
    fn parse(url: &str) -> Result<Url> {
        if url.len() > 8192
            || url.chars().any(|c| c.is_control() || c.is_whitespace())
            || url.contains(['\\', '#'])
        {
            return Err("下载地址无效 / Invalid download URL".into());
        }
        let input = wide(url);
        let mut parts = URL_COMPONENTS {
            dwStructSize: size_of::<URL_COMPONENTS>() as u32,
            dwHostNameLength: u32::MAX,
            dwUrlPathLength: u32::MAX,
            dwExtraInfoLength: u32::MAX,
            dwUserNameLength: u32::MAX,
            dwPasswordLength: u32::MAX,
            ..Default::default()
        };
        if unsafe { WinHttpCrackUrl(input.as_ptr(), (input.len() - 1) as u32, 0, &mut parts) } == 0
            || parts.nScheme != WINHTTP_INTERNET_SCHEME_HTTPS
            || parts.dwHostNameLength == 0
            || parts.dwUserNameLength != 0
            || parts.dwPasswordLength != 0
            || url.split('/').nth(2).is_some_and(|a| a.contains('@'))
        {
            return Err(
                "仅接受不带账号密码的 HTTPS 地址 / HTTPS URL without credentials required".into(),
            );
        }
        let copy = |p: *const u16, n: u32| {
            if n == 0 {
                vec![]
            } else {
                unsafe { std::slice::from_raw_parts(p, n as usize).to_vec() }
            }
        };
        let mut host = copy(parts.lpszHostName, parts.dwHostNameLength);
        host.push(0);
        let mut path = copy(parts.lpszUrlPath, parts.dwUrlPathLength);
        if path.is_empty() {
            path.push('/' as u16);
        }
        path.extend(copy(parts.lpszExtraInfo, parts.dwExtraInfoLength));
        path.push(0);
        Ok(Url {
            host,
            path,
            port: parts.nPort,
        })
    }
}
impl Transport for Https {
    fn open(&self, url: &str) -> Result<Box<dyn Read>> {
        let url = Self::parse(url)?;
        unsafe {
            let session = Handle::new(WinHttpOpen(
                wide("AmberL/0.1 Ambrum").as_ptr(),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                null(),
                null(),
                0,
            ))?;
            checked(WinHttpSetTimeouts(session.0, 5000, 5000, 5000, 5000))?;
            let connection =
                Handle::new(WinHttpConnect(session.0, url.host.as_ptr(), url.port, 0))?;
            let request = Handle::new(WinHttpOpenRequest(
                connection.0,
                wide("GET").as_ptr(),
                url.path.as_ptr(),
                null(),
                null(),
                null(),
                WINHTTP_FLAG_SECURE,
            ))?;
            let disabled = WINHTTP_DISABLE_REDIRECTS
                | WINHTTP_DISABLE_COOKIES
                | WINHTTP_DISABLE_AUTHENTICATION;
            checked(WinHttpSetOption(
                request.0,
                WINHTTP_OPTION_DISABLE_FEATURE,
                (&disabled as *const u32).cast(),
                4,
            ))?;
            let no_logon = WINHTTP_AUTOLOGON_SECURITY_LEVEL_HIGH;
            checked(WinHttpSetOption(
                request.0,
                WINHTTP_OPTION_AUTOLOGON_POLICY,
                (&no_logon as *const u32).cast(),
                4,
            ))?;
            checked(WinHttpSendRequest(request.0, null(), 0, null(), 0, 0, 0))?;
            checked(WinHttpReceiveResponse(request.0, null_mut()))?;
            let mut status = 0u32;
            let mut length = 4u32;
            checked(WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                null(),
                (&mut status as *mut u32).cast(),
                &mut length,
                null_mut(),
            ))?;
            if status != 200 {
                return Err(format!(
                    "下载服务返回 HTTP {status}，未保存；重定向尚未支持 / HTTP {status} rejected"
                ));
            }
            Ok(Box::new(Response {
                request,
                _connection: connection,
                _session: session,
            }))
        }
    }
}
struct Response {
    request: Handle,
    _connection: Handle,
    _session: Handle,
}
impl Read for Response {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut count = 0;
        if unsafe {
            WinHttpReadData(
                self.request.0,
                buffer.as_mut_ptr().cast(),
                buffer.len().min(u32::MAX as usize) as u32,
                &mut count,
            )
        } == 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(count as usize)
        }
    }
}

pub(crate) struct Hash {
    algorithm: BCRYPT_ALG_HANDLE,
    hash: BCRYPT_HASH_HANDLE,
    size: usize,
}
impl Hash {
    pub fn new(algorithm: Algorithm) -> Result<Self> {
        let mut state = Self {
            algorithm: null_mut(),
            hash: null_mut(),
            size: algorithm.bytes(),
        };
        let name = match algorithm {
            Algorithm::Sha1 => BCRYPT_SHA1_ALGORITHM,
            Algorithm::Sha256 => BCRYPT_SHA256_ALGORITHM,
        };
        let status = unsafe { BCryptOpenAlgorithmProvider(&mut state.algorithm, name, null(), 0) };
        if status < 0 {
            return Err("OS hash provider unavailable".into());
        }
        // Windows 7+ allocates the hash object when pbHashObject is NULL.
        if unsafe {
            BCryptCreateHash(
                state.algorithm,
                &mut state.hash,
                null_mut(),
                0,
                null(),
                0,
                0,
            )
        } < 0
        {
            return Err("OS hash initialization failed".into());
        }
        Ok(state)
    }
    pub fn update(&mut self, bytes: &[u8]) -> Result<()> {
        if unsafe { BCryptHashData(self.hash, bytes.as_ptr(), bytes.len() as u32, 0) } < 0 {
            Err("OS hash update failed".into())
        } else {
            Ok(())
        }
    }
    pub fn finish(self) -> Result<String> {
        let mut bytes = vec![0u8; self.size];
        if unsafe { BCryptFinishHash(self.hash, bytes.as_mut_ptr(), bytes.len() as u32, 0) } < 0 {
            return Err("OS hash failed".into());
        }
        Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }
}
impl Drop for Hash {
    fn drop(&mut self) {
        unsafe {
            if !self.hash.is_null() {
                BCryptDestroyHash(self.hash);
            }
            if !self.algorithm.is_null() {
                BCryptCloseAlgorithmProvider(self.algorithm, 0);
            }
        }
    }
}
