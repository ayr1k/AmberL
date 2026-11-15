// SPDX-License-Identifier: MPL-2.0
//! Bounded worker queue. Only verified cache commits count as completed files.
use super::{Artifact, Cache, Downloaded, Phase, Progress, Transport};
use crate::{Result, task::Cancellation};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Instant,
};

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct BatchProgress {
    pub verified_files: usize,
    pub total_files: usize,
    pub verified_bytes: u64,
    pub total_bytes: u64,
    pub network_bytes: u64,
    pub bytes_per_second: u64,
    pub retries: usize,
}
#[derive(Default)]
pub struct BatchResult {
    pub verified_files: usize,
    pub verified_bytes: u64,
    pub cache_hits: usize,
}
enum Event {
    Progress(usize, Progress),
    Done(usize, Result<Downloaded>),
}
struct Meter {
    state: BatchProgress,
    positions: Vec<u64>,
    started: Instant,
    hits: usize,
}
impl Meter {
    fn update(&mut self, index: usize, p: Progress) {
        match p.phase {
            Phase::Retrying => {
                self.state.retries += 1;
                self.positions[index] = 0;
            }
            Phase::Downloading | Phase::Verifying => {
                self.state.network_bytes += p.received.saturating_sub(self.positions[index]);
                self.positions[index] = p.received;
            }
            _ => {}
        }
        self.state.bytes_per_second = (self.state.network_bytes as f64
            / self.started.elapsed().as_secs_f64().max(0.1))
            as u64;
    }
    fn done(&mut self, d: Downloaded) {
        self.state.verified_files += 1;
        self.state.verified_bytes += d.size;
        self.hits += usize::from(d.cache_hit);
    }
}
impl Cache {
    /// HTTPS supplies up to eight workers. Injected non-threadsafe transports retain
    /// sequential behavior. Duplicate digest entries share one verified download.
    pub fn fetch_many(
        &self,
        artifacts: &[Artifact],
        transport: &impl Transport,
        cancel: &Cancellation,
        mut progress: impl FnMut(BatchProgress, Option<Progress>),
    ) -> Result<BatchResult> {
        let mut unique: BTreeMap<String, Artifact> = BTreeMap::new();
        for a in artifacts {
            cancel.check()?;
            let key = a.key()?;
            if let Some(old) = unique.get(&key) {
                if old.size != a.size {
                    return Err("Conflicting download sizes".into());
                }
            } else {
                unique.insert(key, a.clone());
            }
            if unique.len() > 105_000 {
                return Err("Download queue exceeds file limit".into());
            }
        }
        let files: Vec<_> = unique.into_values().collect();
        let total_bytes = files.iter().try_fold(0u64, |n, a| {
            n.checked_add(a.size).ok_or("Download size overflow")
        })?;
        let mut meter = Meter {
            state: BatchProgress {
                total_files: files.len(),
                total_bytes,
                ..Default::default()
            },
            positions: vec![0; files.len()],
            started: Instant::now(),
            hits: 0,
        };
        progress(meter.state, None);
        let workers: Vec<_> = (0..files.len().min(8))
            .filter_map(|_| transport.worker())
            .collect();
        if workers.is_empty() {
            for (index, artifact) in files.iter().enumerate() {
                let d = self.fetch(artifact, transport, cancel, |p| {
                    meter.update(index, p);
                    progress(meter.state, Some(p));
                })?;
                meter.done(d);
                progress(meter.state, None);
            }
        } else {
            let stop = cancel.child();
            let next = AtomicUsize::new(0);
            let (tx, rx) = mpsc::sync_channel(64);
            std::thread::scope(|scope| -> Result<()> {
                let mut handles = vec![];
                for net in workers {
                    let tx = tx.clone();
                    let (files, stop, next) = (&files, &stop, &next);
                    handles.push(scope.spawn(move || {
                        while stop.check().is_ok() {
                            let index = next.fetch_add(1, Ordering::Relaxed);
                            let Some(a) = files.get(index) else {
                                break;
                            };
                            let result = self.fetch(a, net.as_ref(), stop, |p| {
                                let _ = tx.send(Event::Progress(index, p));
                            });
                            let failed = result.is_err();
                            if tx.send(Event::Done(index, result)).is_err() {
                                break;
                            }
                            if failed {
                                stop.cancel();
                                break;
                            }
                        }
                    }));
                }
                drop(tx);
                let mut failure = None;
                for event in rx {
                    match event {
                        Event::Progress(index, p) => {
                            meter.update(index, p);
                            progress(meter.state, Some(p));
                        }
                        Event::Done(_, Ok(d)) => {
                            meter.done(d);
                            progress(meter.state, None);
                        }
                        Event::Done(index, Err(e)) if failure.is_none() => {
                            let a = &files[index];
                            failure = Some(format!(
                                "资源 {} / {}：{e}；已校验文件保留，可重新安装继续 / Verified cache retained",
                                a.url.split('/').nth(2).unwrap_or("HTTPS"),
                                &a.digest[..12]
                            ));
                        }
                        _ => {}
                    }
                }
                for handle in handles {
                    if handle.join().is_err() && failure.is_none() {
                        failure =
                            Some("Download worker interrupted; verified cache retained".into());
                    }
                }
                cancel.check()?;
                if let Some(error) = failure {
                    return Err(error);
                }
                if meter.state.verified_files != files.len() {
                    return Err("Download queue incomplete".into());
                }
                Ok(())
            })?;
        }
        cancel.check()?;
        Ok(BatchResult {
            verified_files: meter.state.verified_files,
            verified_bytes: meter.state.verified_bytes,
            cache_hits: meter.hits,
        })
    }
}
