// SPDX-License-Identifier: MPL-2.0
//! Credential-bearing HTTPS is separate from the public resource downloader.
//! Callers select a fixed service, never an arbitrary token destination.
use super::{Endpoint, Reply, Request, Transport};
use crate::{Result, task::Cancellation};
use std::{
    ffi::c_void,
    ptr::{null, null_mut},
    time::{Duration, Instant},
};
use windows_sys::Win32::Networking::WinHttp::*;
use zeroize::Zeroizing;

pub struct Https;
struct Handle(*mut c_void);
fn checked(ok: i32) -> Result<()> {
    if ok == 0 {
        Err(format!(
            "认证网络请求失败 / Authentication HTTPS failed (Windows {})",
            std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
        ))
    } else {
        Ok(())
    }
}
impl Handle {
    fn new(value: *mut c_void) -> Result<Self> {
        checked(i32::from(!value.is_null()))?;
        Ok(Self(value))
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

impl Transport for Https {
    fn request(&self, request: Request<'_>, cancel: &Cancellation) -> Result<Reply> {
        cancel.check()?;
        if request.body.len() > 256 * 1024 {
            return Err("Authentication request exceeds limit".into());
        }
        let (host, path) = request.endpoint.destination();
        let mut headers = Zeroizing::new(String::from("Accept: application/json\r\n"));
        if request.endpoint.is_get() {
            let token = request.bearer.ok_or("Missing service credential")?;
            headers.push_str("Authorization: Bearer ");
            headers.push_str(token.expose());
            headers.push_str("\r\n");
        } else {
            if request.bearer.is_some() {
                return Err("Unexpected bearer credential".into());
            }
            headers.push_str(
                if matches!(request.endpoint, Endpoint::Device | Endpoint::Token) {
                    "Content-Type: application/x-www-form-urlencoded\r\n"
                } else {
                    "Content-Type: application/json\r\n"
                },
            );
        }
        if matches!(request.endpoint, Endpoint::Xbox | Endpoint::Xsts) {
            headers.push_str("x-xbl-contract-version: 1\r\n");
        }
        let headers = Zeroizing::new(wide(&headers));
        let started = Instant::now();
        unsafe {
            let session = Handle::new(WinHttpOpen(
                wide("AmberL/0.1 Ambrum").as_ptr(),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                null(),
                null(),
                0,
            ))?;
            checked(WinHttpSetTimeouts(session.0, 5000, 5000, 5000, 5000))?;
            let connection = Handle::new(WinHttpConnect(session.0, wide(host).as_ptr(), 443, 0))?;
            let call = Handle::new(WinHttpOpenRequest(
                connection.0,
                wide(if request.endpoint.is_get() {
                    "GET"
                } else {
                    "POST"
                })
                .as_ptr(),
                wide(path).as_ptr(),
                null(),
                null(),
                null(),
                WINHTTP_FLAG_SECURE,
            ))?;
            let disabled = WINHTTP_DISABLE_REDIRECTS
                | WINHTTP_DISABLE_COOKIES
                | WINHTTP_DISABLE_AUTHENTICATION;
            checked(WinHttpSetOption(
                call.0,
                WINHTTP_OPTION_DISABLE_FEATURE,
                (&disabled as *const u32).cast(),
                4,
            ))?;
            let no_logon = WINHTTP_AUTOLOGON_SECURITY_LEVEL_HIGH;
            checked(WinHttpSetOption(
                call.0,
                WINHTTP_OPTION_AUTOLOGON_POLICY,
                (&no_logon as *const u32).cast(),
                4,
            ))?;
            cancel.check()?;
            checked(WinHttpSendRequest(
                call.0,
                headers.as_ptr(),
                (headers.len() - 1) as u32,
                request.body.as_ptr().cast(),
                request.body.len() as u32,
                request.body.len() as u32,
                0,
            ))?;
            checked(WinHttpReceiveResponse(call.0, null_mut()))?;
            let mut status = 0u32;
            let mut size = 4u32;
            checked(WinHttpQueryHeaders(
                call.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                null(),
                (&mut status as *mut u32).cast(),
                &mut size,
                null_mut(),
            ))?;
            // Never follow a redirect with a token; never include response bodies in errors.
            if (300..400).contains(&status) {
                return Err("Authentication redirect rejected".into());
            }
            let mut body = Zeroizing::new(Vec::new());
            let mut buffer = Zeroizing::new(vec![0u8; 16384]);
            loop {
                cancel.check()?;
                if started.elapsed() > Duration::from_secs(30) {
                    return Err("Authentication request deadline exceeded".into());
                }
                let mut count = 0;
                checked(WinHttpReadData(
                    call.0,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    &mut count,
                ))?;
                if count == 0 {
                    break;
                }
                if body.len() + count as usize > 1024 * 1024 {
                    return Err("Authentication response exceeds limit".into());
                }
                body.extend_from_slice(&buffer[..count as usize]);
            }
            cancel.check()?;
            Ok(Reply { status, body })
        }
    }
}
