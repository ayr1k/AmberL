// SPDX-License-Identifier: MPL-2.0
//! Optional thumbnails load independently; text results never wait for image requests.
use ambrum::{
    download::{Https, Transport},
    task::Cancellation,
};
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read},
    sync::{Arc, Mutex, mpsc},
};
#[derive(Default)]
pub struct Thumbnails {
    images: HashMap<String, slint::Image>,
    requested: HashSet<String>,
    receiver: Option<mpsc::Receiver<(String, Vec<u8>, u32, u32)>>,
    sender: Option<mpsc::SyncSender<(String, Cancellation)>>,
    cancel: Cancellation,
    page: Vec<String>,
}
impl Thumbnails {
    pub fn request(&mut self, urls: Vec<String>) {
        if urls == self.page {
            return;
        }
        self.cancel.cancel();
        self.cancel = Cancellation::default();
        self.requested.clear();
        self.page = urls.clone();
        if self.images.len() > 128 {
            self.images.retain(|url, _| urls.contains(url));
        }
        let urls: Vec<_> = urls
            .into_iter()
            .filter(|u| !self.images.contains_key(u) && self.requested.insert(u.clone()))
            .collect();
        if self.sender.is_none() {
            let (jobs, queue) = mpsc::sync_channel::<(String, Cancellation)>(64);
            let (tx, rx) = mpsc::channel();
            self.sender = Some(jobs);
            self.receiver = Some(rx);
            let queue = Arc::new(Mutex::new(queue));
            for _ in 0..4 {
                let (queue, tx) = (queue.clone(), tx.clone());
                let _ = std::thread::Builder::new()
                    .name("amberl-content-icon".into())
                    .spawn(move || {
                        loop {
                            let job = match queue.lock() {
                                Ok(q) => q.recv(),
                                Err(_) => break,
                            };
                            let Ok((url, cancel)) = job else { break };
                            if cancel.check().is_err() {
                                continue;
                            }
                            if let Ok((bytes, w, h)) = load(&url, &cancel) {
                                if cancel.check().is_ok() {
                                    let _ = tx.send((url, bytes, w, h));
                                }
                            }
                        }
                    });
            }
        }
        if let Some(tx) = &self.sender {
            for url in urls {
                let _ = tx.try_send((url, self.cancel.clone()));
            }
        }
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        if let Some(rx) = &self.receiver {
            while let Ok((url, bytes, w, h)) = rx.try_recv() {
                self.images.insert(
                    url,
                    slint::Image::from_rgba8(
                        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                            &bytes, w, h,
                        ),
                    ),
                );
                changed = true;
            }
        }
        changed
    }
    pub fn image(&self, url: Option<&str>) -> slint::Image {
        url.and_then(|u| self.images.get(u))
            .cloned()
            .unwrap_or_default()
    }
    pub fn has(&self, url: Option<&str>) -> bool {
        url.is_some_and(|u| self.images.contains_key(u))
    }
}
impl Drop for Thumbnails {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
fn load(url: &str, cancel: &Cancellation) -> Result<(Vec<u8>, u32, u32), String> {
    if !url.starts_with("https://cdn.modrinth.com/data/") {
        return Err("Unsupported icon origin".into());
    }
    cancel.check()?;
    let mut bytes = vec![];
    for attempt in 0..3 {
        cancel.check()?;
        bytes.clear();
        let result = Https.open(url).and_then(|input| {
            input
                .take(512 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map(|_| ())
                .map_err(|_| "Icon download interrupted".into())
        });
        match result {
            Ok(()) => break,
            Err(e) => {
                cancel.check()?;
                if attempt == 2 {
                    return Err(e);
                }
                for _ in 0..10 {
                    cancel.check()?;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    }
    cancel.check()?;
    if bytes.len() > 512 * 1024 {
        return Err("Icon size limit".into());
    }
    let mut input = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "Invalid icon")?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(1024);
    limits.max_image_height = Some(1024);
    limits.max_alloc = Some(8 * 1024 * 1024);
    input.limits(limits);
    let image = input
        .decode()
        .map_err(|_| "Unsupported icon")?
        .thumbnail(96, 96)
        .to_rgba8();
    Ok((image.as_raw().clone(), image.width(), image.height()))
}
