// SPDX-License-Identifier: MPL-2.0
use std::{
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::mpsc,
};
pub struct Scene {
    pub pixels: image::RgbaImage,
    pub blurred: image::RgbaImage,
}
#[derive(Default)]
pub struct Loader {
    receiver: Option<mpsc::Receiver<(Option<PathBuf>, Result<Scene, String>)>>,
}
impl Loader {
    pub fn start(&mut self, path: Option<PathBuf>) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("amberl-background".into())
            .spawn(move || {
                let result = load(path.as_deref());
                let _ = tx.send((path, result));
            })
            .map_err(|_| "无法读取背景图")?;
        self.receiver = Some(rx);
        Ok(())
    }
    pub fn poll(&mut self) -> Option<(Option<PathBuf>, Result<Scene, String>)> {
        self.receiver.as_ref()?.try_recv().ok()
    }
}
fn load(path: Option<&Path>) -> Result<Scene, String> {
    let bytes = if let Some(path) = path {
        let mut data = vec![];
        std::fs::File::open(path)
            .map_err(|_| "无法打开背景图")?
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut data)
            .map_err(|_| "无法读取背景图")?;
        if data.len() > 32 * 1024 * 1024 {
            return Err("背景图片不能超过 32 MiB".into());
        }
        data
    } else {
        include_bytes!("../../../assets/launcher/forest-lake.png").to_vec()
    };
    let mut input = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "无效背景图")?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(128 * 1024 * 1024);
    input.limits(limits);
    let scene = input.decode().map_err(|_| "不支持或过大的背景图片")?;
    let blurred = image::imageops::blur(
        &scene
            .resize(744, 529, image::imageops::FilterType::Triangle)
            .to_rgba8(),
        14.0,
    );
    let pixels = scene
        .resize(3840, 2160, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    Ok(Scene { pixels, blurred })
}
pub fn pick(window: &slint::Window) -> Option<PathBuf> {
    use slint::winit_030::{
        WinitWindowAccessor,
        winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
    };
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let owner = window
        .with_winit_window(|w| match w.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(h.hwnd.get() as *mut std::ffi::c_void),
            _ => None,
        })
        .flatten()?;
    let filter: Vec<u16> = "Images (PNG, JPEG, WebP, GIF)\0*.png;*.jpg;*.jpeg;*.webp;*.gif\0\0"
        .encode_utf16()
        .collect();
    let mut path = vec![0u16; 32768];
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrFilter: filter.as_ptr(),
        lpstrFile: path.as_mut_ptr(),
        nMaxFile: path.len() as u32,
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    if unsafe { GetOpenFileNameW(&mut dialog) } == 0 {
        return None;
    }
    Some(PathBuf::from(
        String::from_utf16(&path[..path.iter().position(|c| *c == 0)?]).ok()?,
    ))
}
pub fn color(value: &str) -> Option<slint::Color> {
    let v = value.strip_prefix('#')?;
    if v.len() != 6 {
        return None;
    }
    let rgb = u32::from_str_radix(v, 16).ok()?;
    Some(slint::Color::from_rgb_u8(
        (rgb >> 16) as u8,
        (rgb >> 8) as u8,
        rgb as u8,
    ))
}
