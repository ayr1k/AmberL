// SPDX-License-Identifier: MPL-2.0
//! Shared, read-only comparison workload. No framework types, account access or game writes.
use ambrum::{
    library::{Instance, Isolation, Library, Scan},
    task::Cancellation,
};
use serde::Serialize;
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const LOG_LIMIT: usize = 120;
#[derive(Clone, Serialize)]
pub struct Row {
    pub id: String,
    pub text: String,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub revision: u64,
    pub rows_revision: u64,
    pub rows: Vec<Row>,
    pub selected_id: Option<String>,
    pub detail: String,
    pub status: String,
    pub scanning: bool,
    pub task_running: bool,
    pub task_progress: f32,
    pub log: String,
}
struct ScanWorker {
    cancel: Cancellation,
    receiver: mpsc::Receiver<ambrum::Result<Scan>>,
    started: Instant,
}
struct Task {
    started: Instant,
    last_tick: u128,
}
pub struct Model {
    instances: Vec<Instance>,
    rows: Vec<Row>,
    query: String,
    selected: Option<String>,
    detail: String,
    status: String,
    worker: Option<ScanWorker>,
    task: Option<Task>,
    progress: f32,
    log: VecDeque<String>,
    revision: u64,
    rows_revision: u64,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            instances: vec![],
            rows: vec![],
            query: String::new(),
            selected: None,
            detail: "选择条目查看核心返回的隔离证据。Select a row for details.".into(),
            status: "比较样机 · 仅扫描与交互测试，不启动或修改游戏".into(),
            worker: None,
            task: None,
            progress: 0.0,
            log: VecDeque::new(),
            revision: 1,
            rows_revision: 1,
        }
    }
}
fn key(instance: &Instance) -> String {
    serde_json::to_string(&instance.id).expect("String-only instance identity")
}
fn isolation(value: Isolation) -> &'static str {
    match value {
        Isolation::Isolated => "独立 / Isolated",
        Isolation::Shared => "共享 / Shared",
        Isolation::Unknown => "待确认 / Unknown",
        Isolation::Conflict => "冲突 / Conflict",
    }
}
impl Model {
    fn note(&mut self, text: String) {
        if self.log.len() == LOG_LIMIT {
            self.log.pop_front();
        }
        self.log.push_back(text);
        self.revision += 1;
    }
    pub fn scan(&mut self, path: PathBuf) -> Result<(), String> {
        if self.worker.is_some() {
            return Err("扫描正在进行；请等待或取消 / Scan already running".into());
        }
        let cancel = Cancellation::default();
        let token = cancel.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("amberl-ui-scan".into())
            .spawn(move || {
                let result = Library::open(&path).and_then(|library| library.scan(&token));
                let _ = sender.send(result);
            })
            .map_err(|_| "Cannot create scan worker")?;
        self.worker = Some(ScanWorker {
            cancel,
            receiver,
            started: Instant::now(),
        });
        self.status = "正在读取游戏库… / Scanning library…".into();
        self.note("开始只读扫描 / Read-only scan started".into());
        Ok(())
    }
    pub fn cancel_scan(&mut self) {
        if let Some(worker) = &self.worker {
            worker.cancel.cancel();
            self.status = "正在取消扫描… / Cancelling scan…".into();
            self.revision += 1;
        }
    }
    pub fn filter(&mut self, query: &str) -> Result<(), String> {
        if query.len() > 4096 {
            return Err("Search input exceeds limit".into());
        }
        if self.query == query {
            return Ok(());
        }
        self.query = query.into();
        self.refilter();
        Ok(())
    }
    fn refilter(&mut self) {
        let query = self.query.to_lowercase();
        self.rows = self
            .instances
            .iter()
            .filter(|i| {
                format!("{} {} {}", i.name, i.game, i.loader)
                    .to_lowercase()
                    .contains(&query)
            })
            .map(|i| Row {
                id: key(i),
                text: format!(
                    "{}  ·  {}  ·  {}  ·  {}",
                    i.name,
                    i.game,
                    i.loader,
                    isolation(i.isolation)
                ),
            })
            .collect();
        if self
            .selected
            .as_ref()
            .is_some_and(|id| !self.rows.iter().any(|r| &r.id == id))
        {
            self.selected = None;
            self.detail = "当前筛选中未选中条目 / No selected row in this filter".into();
        }
        self.rows_revision += 1;
        self.revision += 1;
    }
    pub fn select(&mut self, id: &str) -> Result<(), String> {
        if !self.rows.iter().any(|r| r.id == id) {
            return Err("Stale or filtered row identity".into());
        }
        let i = self
            .instances
            .iter()
            .find(|i| key(i) == id)
            .ok_or("Unknown row identity")?;
        self.detail = format!(
            "{}\n{} · {}\n{}\n{}\n{}",
            i.name,
            i.game,
            i.loader,
            isolation(i.isolation),
            i.game_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "实际游戏目录未确认 / Game directory unresolved".into()),
            i.issue.as_deref().unwrap_or("")
        );
        for evidence in &i.evidence {
            self.detail.push_str(&format!("\n• {evidence}"));
        }
        self.selected = Some(id.into());
        self.revision += 1;
        Ok(())
    }
    pub fn start_task(&mut self) {
        if self.task.is_some() {
            return;
        }
        self.task = Some(Task {
            started: Instant::now(),
            last_tick: 0,
        });
        self.progress = 0.0;
        self.note(
            "模拟任务：10 秒、每秒 100 条日志；不是下载或游戏就绪检测 / Synthetic task".into(),
        );
    }
    pub fn cancel_task(&mut self) {
        if self.task.take().is_some() {
            self.note("模拟任务已取消 / Synthetic task cancelled".into());
        }
    }
    /// Called by each UI's 50 ms timer. Progress/log workload is identical, bounded and synthetic.
    pub fn tick(&mut self) {
        if let Some(worker) = &self.worker {
            let result = match worker.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Scan worker disconnected".into()))
                }
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                let worker = self.worker.take().unwrap();
                match result {
                    Ok(scan) if worker.cancel.check().is_ok() => {
                        self.instances = scan.instances;
                        self.selected = None;
                        self.detail = "扫描完成，请选择条目 / Select a row".into();
                        self.refilter();
                        self.status = format!(
                            "{} 个条目 · {} ms / Read-only scan complete",
                            self.instances.len(),
                            worker.started.elapsed().as_millis()
                        );
                    }
                    Ok(_) => {
                        self.status = "扫描已取消，未应用结果 / Cancelled; result discarded".into()
                    }
                    Err(error) => self.status = error,
                }
                self.note(self.status.clone());
            }
        }
        if let Some(task) = &mut self.task {
            let elapsed = task.started.elapsed();
            let tick = (elapsed.as_millis() / 100).min(100);
            if tick > task.last_tick {
                let previous = task.last_tick;
                task.last_tick = tick;
                self.progress = tick as f32 / 100.0;
                for sequence in previous * 10..tick * 10 {
                    self.note(format!(
                        "模拟日志 / Synthetic log {:04} · 测试中文与滚动",
                        sequence + 1
                    ));
                }
            }
            if elapsed >= Duration::from_secs(10) {
                self.task = None;
                self.note("模拟任务完成 / Synthetic task complete".into());
            }
        }
    }
    pub fn snapshot(&self, include_rows: bool) -> Snapshot {
        Snapshot {
            revision: self.revision,
            rows_revision: self.rows_revision,
            rows: if include_rows {
                self.rows.clone()
            } else {
                vec![]
            },
            selected_id: self.selected.clone(),
            detail: self.detail.clone(),
            status: self.status.clone(),
            scanning: self.worker.is_some(),
            task_running: self.task.is_some(),
            task_progress: self.progress,
            log: self.log.iter().cloned().collect::<Vec<_>>().join("\n"),
        }
    }
}
impl Drop for Model {
    fn drop(&mut self) {
        if let Some(worker) = &self.worker {
            worker.cancel.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ambrum::library::{ClientInstanceId, LibraryId, LibraryKind};

    #[test]
    fn cancellation_discards_queued_success_and_filter_preserves_identity() {
        let library = Library {
            id: LibraryId::Observed("synthetic".into()),
            root: "synthetic".into(),
            kind: LibraryKind::Minecraft,
        };
        let instance = Instance {
            id: ClientInstanceId {
                library: library.id.clone(),
                profile: "中文".into(),
            },
            name: "中文测试".into(),
            game: "1.21.4".into(),
            loader: "Vanilla".into(),
            isolation: Isolation::Unknown,
            game_dir: None,
            evidence: vec!["未确认".into()],
            issue: None,
        };
        let mut model = Model::default();
        model.instances = vec![instance];
        model.refilter();
        let id = model.rows[0].id.clone();
        model.select(&id).unwrap();
        model.filter("中文").unwrap();
        assert_eq!(model.selected.as_deref(), Some(id.as_str()));
        let (sender, receiver) = mpsc::sync_channel(1);
        sender
            .send(Ok(Scan {
                library,
                instances: vec![],
                warnings: vec![],
            }))
            .unwrap();
        let cancel = Cancellation::default();
        cancel.cancel();
        model.worker = Some(ScanWorker {
            cancel,
            receiver,
            started: Instant::now(),
        });
        model.tick();
        assert_eq!(model.rows.len(), 1);
        assert!(model.status.contains("discarded"));
        model.filter("missing").unwrap();
        assert!(model.selected.is_none());
        assert!(model.select(&id).is_err());
    }

    #[test]
    fn failed_scan_cannot_replace_existing_results() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancel = Cancellation::default();
        cancel.cancel();
        sender.send(Err("cancelled test scan".into())).unwrap();
        let mut model = Model::default();
        model.rows.push(Row {
            id: "original".into(),
            text: "原始条目".into(),
        });
        model.worker = Some(ScanWorker {
            cancel,
            receiver,
            started: Instant::now(),
        });
        assert!(model.scan(PathBuf::from("unused")).is_err());
        model.tick();
        assert!(!model.snapshot(true).scanning);
        assert_eq!(model.snapshot(true).rows[0].id, "original");
    }

    #[test]
    fn log_backlog_is_bounded_and_cancellation_stops_updates() {
        let mut model = Model::default();
        model.start_task();
        model.task.as_mut().unwrap().started = Instant::now() - Duration::from_secs(11);
        model.tick();
        let snapshot = model.snapshot(false);
        assert!(!snapshot.task_running);
        assert_eq!(snapshot.task_progress, 1.0);
        assert_eq!(snapshot.log.lines().count(), LOG_LIMIT);
        model.start_task();
        model.cancel_task();
        let revision = model.snapshot(false).revision;
        model.tick();
        assert_eq!(model.snapshot(false).revision, revision);
    }

    #[test]
    fn rejects_stale_selection_and_oversized_query() {
        let mut model = Model::default();
        assert!(model.select("untrusted row").is_err());
        model.filter("测试").unwrap();
        let revision = model.snapshot(false).revision;
        assert!(model.filter(&"界".repeat(2000)).is_err());
        assert_eq!(model.query, "测试");
        assert_eq!(model.snapshot(false).revision, revision);
    }
}
