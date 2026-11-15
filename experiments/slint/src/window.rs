// SPDX-License-Identifier: MPL-2.0
//! Fit initial physical bounds to the selected monitor's work area at the actual UI scale.
pub fn fit(window: &slint::Window) -> bool {
    use slint::winit_030::{WinitWindowAccessor, winit};
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO};
    use winit::platform::windows::MonitorHandleExtWindows;
    let scale = window.scale_factor().max(0.5);
    let bounds = window.with_winit_window(|native| {
        let monitor = native.current_monitor()?;
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(monitor.hmonitor() as _, &mut info) } == 0 {
            return None;
        }
        let area = info.rcWork;
        let available_width = (area.right - area.left - 32).max(1) as u32;
        let available_height = (area.bottom - area.top - 32).max(1) as u32;
        let width = (1280.0 * scale) as u32;
        let height = (820.0 * scale) as u32;
        let width = width.min(available_width).max((800.0 * scale) as u32);
        let height = height.min(available_height).max((500.0 * scale) as u32);
        Some((
            width,
            height,
            area.left + ((area.right - area.left - width as i32) / 2).max(0),
            area.top + ((area.bottom - area.top - height as i32) / 2).max(0),
        ))
    });
    if let Some(Some((width, height, x, y))) = bounds {
        window.set_size(slint::PhysicalSize::new(width, height));
        window.with_winit_window(|native| {
            native.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
        });
        return true;
    }
    false
}
