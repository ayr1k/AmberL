// SPDX-License-Identifier: MPL-2.0
//! Cached static-scene glass. Software Slint does not round child-image clips.
use image::{RgbaImage, imageops};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

pub struct Glass {
    blurred: RgbaImage,
    viewport: RgbaImage,
    size: (u32, u32),
    tiles: Vec<([u32; 4], Image)>,
}
impl Glass {
    pub fn new() -> Result<Self, image::ImageError> {
        let scene =
            image::load_from_memory(include_bytes!("../../../assets/launcher/forest-lake.png"))?;
        let small = scene
            .resize(744, 529, imageops::FilterType::Triangle)
            .to_rgba8();
        Ok(Self {
            blurred: imageops::blur(&small, 14.0),
            viewport: RgbaImage::new(0, 0),
            size: (0, 0),
            tiles: vec![],
        })
    }
    pub fn tile(&mut self, geometry: [f32; 6]) -> Image {
        let [x, y, width, height, vw, vh] = geometry;
        if geometry.iter().any(|n| !n.is_finite())
            || vw <= 0.0
            || vh <= 0.0
            || width <= 0.0
            || height <= 0.0
        {
            return Image::default();
        }
        // Blurred imagery needs little resolution; bound memory even on very large monitors.
        let scale = 0.5_f32.min(2048.0 / vw).min(1080.0 / vh);
        let size = ((vw * scale).ceil() as u32, (vh * scale).ceil() as u32);
        if size != self.size {
            self.viewport = imageops::resize(
                &self.blurred,
                (self.blurred.width() as f32
                    * (size.0 as f32 / self.blurred.width() as f32)
                        .max(size.1 as f32 / self.blurred.height() as f32))
                .ceil() as u32,
                (self.blurred.height() as f32
                    * (size.0 as f32 / self.blurred.width() as f32)
                        .max(size.1 as f32 / self.blurred.height() as f32))
                .ceil() as u32,
                imageops::FilterType::Triangle,
            );
            let ox = (self.viewport.width() - size.0) / 2;
            let oy = (self.viewport.height() - size.1) / 2;
            self.viewport = imageops::crop_imm(&self.viewport, ox, oy, size.0, size.1).to_image();
            self.size = size;
            self.tiles.clear();
        }
        let x = ((x * scale).round() as u32).min(size.0 - 1);
        let y = ((y * scale).round() as u32).min(size.1 - 1);
        let w = ((width * scale).round() as u32).max(1).min(size.0 - x);
        let h = ((height * scale).round() as u32).max(1).min(size.1 - y);
        let key = [x, y, w, h];
        if let Some((_, image)) = self.tiles.iter().find(|(k, _)| *k == key) {
            return image.clone();
        }
        let mut tile = imageops::crop_imm(&self.viewport, x, y, w, h).to_image();
        let radius = (15.0 * scale).min(w as f32 / 2.0).min(h as f32 / 2.0);
        for (px, py, pixel) in tile.enumerate_pixels_mut() {
            let dx = (radius - (px as f32 + 0.5).min(w as f32 - px as f32 - 0.5)).max(0.0);
            let dy = (radius - (py as f32 + 0.5).min(h as f32 - py as f32 - 0.5)).max(0.0);
            pixel[3] = ((radius + 0.5 - dx.hypot(dy)).clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        let image = Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
            tile.as_raw(),
            w,
            h,
        ));
        if self.tiles.len() == 16 {
            self.tiles.remove(0);
        }
        self.tiles.push((key, image.clone()));
        image
    }
}
