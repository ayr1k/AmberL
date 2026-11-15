// SPDX-License-Identifier: MPL-2.0
//! Copy only the device code explicitly requested by the sign-in flow.
pub fn copy(window: &slint::Window, code: &str) -> bool {
    use slint::winit_030::{
        WinitWindowAccessor,
        winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
    };
    use windows_sys::Win32::{
        Foundation::GlobalFree,
        System::{
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
        },
    };
    if code.is_empty() || code.len() > 128 {
        return false;
    }
    let Some(Some(owner)) =
        window.with_winit_window(|native| match native.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as *mut std::ffi::c_void),
            _ => None,
        })
    else {
        return false;
    };
    let text: Vec<u16> = code.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, text.len() * 2);
        if memory.is_null() {
            return false;
        }
        let pointer = GlobalLock(memory);
        if pointer.is_null() {
            GlobalFree(memory);
            return false;
        }
        std::ptr::copy_nonoverlapping(text.as_ptr(), pointer.cast::<u16>(), text.len());
        GlobalUnlock(memory);
        if OpenClipboard(owner) == 0 {
            GlobalFree(memory);
            return false;
        }
        let copied = EmptyClipboard() != 0 && !SetClipboardData(13, memory).is_null();
        CloseClipboard();
        if !copied {
            GlobalFree(memory);
        }
        copied
    }
}
