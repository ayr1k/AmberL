// SPDX-License-Identifier: MPL-2.0
#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
mod native;
fn main() {
    #[cfg(windows)]
    native::run();
    #[cfg(not(windows))]
    eprintln!("This UI baseline is Windows-only. Use amberl-inspect for the core.");
}
