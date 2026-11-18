// SPDX-License-Identifier: MPL-2.0
//! Short-lived, bounded anonymous metadata cache for interactive dependency choices.
use crate::{Result, task::Cancellation};
use serde_json::Value;
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
struct Entry {
    key: String,
    at: Instant,
    data: Value,
    bytes: usize,
}
static CACHE: OnceLock<Mutex<VecDeque<Entry>>> = OnceLock::new();
pub fn clear() {
    if let Ok(mut entries) = CACHE.get_or_init(Mutex::default).lock() {
        entries.clear();
    }
}
pub fn get(key: &str, c: &Cancellation, fetch: impl FnOnce() -> Result<Value>) -> Result<Value> {
    c.check()?;
    let cache = CACHE.get_or_init(Mutex::default);
    if let Ok(entries) = cache.lock() {
        if let Some(e) = entries
            .iter()
            .find(|e| e.key == key && e.at.elapsed() < Duration::from_secs(120))
        {
            return Ok(e.data.clone());
        }
    }
    let data = fetch()?;
    c.check()?;
    let bytes = data.to_string().len();
    if bytes <= 1024 * 1024 {
        if let Ok(mut entries) = cache.lock() {
            entries.retain(|e| e.key != key && e.at.elapsed() < Duration::from_secs(120));
            while entries.len() >= 128
                || entries.iter().map(|e| e.bytes).sum::<usize>() + bytes > 16 * 1024 * 1024
            {
                entries.pop_front();
            }
            entries.push_back(Entry {
                key: key.into(),
                at: Instant::now(),
                data: data.clone(),
                bytes,
            });
        }
    }
    Ok(data)
}
