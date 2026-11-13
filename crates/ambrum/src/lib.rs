// SPDX-License-Identifier: MPL-2.0
//! UI-independent inspection, resource preparation and explicit new-directory installation.
//! Attaching an external game library remains read-only; no login or game execution yet.
#[cfg(windows)]
pub mod catalog;
#[cfg(windows)]
pub mod download;
#[cfg(windows)]
pub mod install;
pub mod java;
pub mod library;
#[cfg(windows)]
pub mod manifest;
pub mod paths;
#[cfg(windows)]
pub mod prepare;
#[cfg(windows)]
pub mod registry;
pub mod task;
#[cfg(windows)]
mod transaction;

/// Public application configuration explicitly shared with AMBER. Never a credential.
pub const MICROSOFT_CLIENT_ID: &str = "ab7cf49a-fea7-432d-8ce8-beb922eb554e";
pub const MICROSOFT_AUTHORITY: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/";
pub const MICROSOFT_SCOPE: &str = "XboxLive.SignIn XboxLive.offline_access";
pub const APPLICATION_ID: &str = "io.github.ayr1k.amberl";

pub type Result<T> = std::result::Result<T, String>;
