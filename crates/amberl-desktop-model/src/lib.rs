// SPDX-License-Identifier: MPL-2.0
//! UI-independent coordinator for the native launcher. All authority stays in Ambrum.
#![cfg(windows)]
use ambrum::{
    Result,
    auth::{DeviceFlow, Poll, Profile},
    download::Cache,
    java::Candidate,
    launch,
    library::Isolation,
    manifest::Platform,
    registry::Registry,
    secure::Store,
    task::Cancellation,
};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
pub mod content;
pub mod network;

#[derive(Clone, Debug)]
pub struct Game {
    pub id: String,
    pub library_id: String,
    pub root: PathBuf,
    pub version: String,
    pub title: String,
    pub icon: String,
    pub subtitle: String,
    /// A display hint only. launch::run rechecks receipts, identity and all resources.
    pub installation_hint: bool,
}

#[derive(Clone)]
pub struct LaunchChoice {
    pub game: Game,
    pub java: PathBuf,
    pub account_id: String,
    pub memory_mib: u32,
    pub os_version: String,
    pub target: Option<ambrum::multiplayer::Address>,
}
impl LaunchChoice {
    pub fn capture(game: &Game, java: &str, account: &str, memory: i32, os: &str) -> Result<Self> {
        if !game.installation_hint {
            return Err("此游戏库仅供查看；请先使用 AmberL 完成游戏安装".into());
        }
        if java.trim().is_empty() || account.is_empty() {
            return Err("请明确选择微软账户和 Java".into());
        }
        if !(512..=65536).contains(&memory) {
            return Err("内存需在 512–65536 MiB 之间".into());
        }
        if os.is_empty() {
            return Err("无法确认 Windows 版本，暂不能启动".into());
        }
        Ok(Self {
            game: game.clone(),
            java: java.into(),
            account_id: account.into(),
            memory_mib: memory as u32,
            os_version: os.into(),
            target: None,
        })
    }
}
enum Message {
    Content(content::Response),
    Data(Vec<Game>, Vec<Profile>),
    Java(Vec<Candidate>),
    Catalog(Vec<ambrum::catalog::Version>),
    Loaders(String, String, Vec<String>),
    Prompt(String),
    Status(String),
    Started,
    Finished(Result<()>),
}
enum Operation {
    Content(content::Request),
    Reload,
    Catalog,
    Loaders(String, String),
    Attach(PathBuf),
    Java,
    InstallJava(u32),
    Login,
    Forget(String),
    Launch(LaunchChoice),
    Install {
        version: String,
        parent: PathBuf,
        name: String,
        budget: u64,
        title: String,
        icon: String,
        loader: Option<ambrum::loader::Choice>,
        java: PathBuf,
        os: String,
    },
}
struct Worker {
    cancel: Cancellation,
    receiver: mpsc::Receiver<Message>,
    handle: thread::JoinHandle<()>,
}
pub struct Model {
    app: PathBuf,
    pub games: Vec<Game>,
    pub accounts: Vec<Profile>,
    pub java: Vec<Candidate>,
    pub catalog: Vec<ambrum::catalog::Version>,
    pub loaders: Vec<String>,
    pub loader_key: (String, String),
    pub catalog_revision: u64,
    pub selected: Option<String>,
    pub status: String,
    pub device_prompt: String,
    pub revision: u64,
    pub data_revision: u64,
    pub java_revision: u64,
    pub running: bool,
    pub network: network::Network,
    pub content: content::Content,
    pub preferences: std::collections::BTreeMap<String, ambrum::preferences::Launch>,
    worker: Option<Worker>,
    log: VecDeque<String>,
}
impl Model {
    pub fn new(app: PathBuf) -> Self {
        let saved = ambrum::preferences::load(&app);
        let (selected, preferences) = saved.as_ref().cloned().unwrap_or_default();
        Self {
            network: network::Network::new(app.clone()),
            content: content::Content::default(),
            app,
            games: vec![],
            accounts: vec![],
            java: vec![],
            catalog: vec![],
            loaders: vec![],
            loader_key: (String::new(), String::new()),
            catalog_revision: 0,
            selected,
            preferences,
            status: saved.err().unwrap_or_else(|| "选择或安装一个游戏".into()),
            device_prompt: String::new(),
            revision: 1,
            data_revision: 1,
            java_revision: 1,
            running: false,
            worker: None,
            log: VecDeque::new(),
        }
    }
    pub fn busy(&self) -> bool {
        self.worker.is_some()
    }
    pub fn save_preferences(&mut self, preference: ambrum::preferences::Launch) -> Result<()> {
        let id = self.selected_game().ok_or("请先选择游戏")?.id.clone();
        if preference
            .account
            .as_ref()
            .is_some_and(|id| !self.accounts.iter().any(|a| &a.id == id))
        {
            return Err("账户选择已失效".into());
        }
        ambrum::preferences::save(&self.app, &id, preference.clone())?;
        self.preferences.insert(id, preference);
        self.report("已保存此游戏的启动配置".into());
        Ok(())
    }
    pub fn selected_game(&self) -> Option<&Game> {
        self.games
            .iter()
            .find(|g| Some(&g.id) == self.selected.as_ref())
    }
    pub fn select(&mut self, id: &str) -> Result<()> {
        if self.busy() {
            return Err("请等待当前操作完成".into());
        }
        if !self.games.iter().any(|g| g.id == id) {
            return Err("游戏条目已失效，请刷新".into());
        }
        self.selected = Some(id.into());
        self.revision += 1;
        Ok(())
    }
    pub fn log(&self) -> String {
        self.log.iter().cloned().collect::<Vec<_>>().join("\n")
    }
    pub fn report(&mut self, message: String) {
        let message = message.chars().take(2048).collect::<String>();
        self.status = message.clone();
        if self.log.len() == 120 {
            self.log.pop_front();
        }
        self.log.push_back(message);
        self.revision += 1;
    }
    pub fn reload(&mut self) -> Result<()> {
        self.start(Operation::Reload)
    }
    pub fn attach(&mut self, path: PathBuf) -> Result<()> {
        self.start(Operation::Attach(path))
    }
    pub fn discover_java(&mut self) -> Result<()> {
        self.start(Operation::Java)
    }
    pub fn install_java(&mut self, major: u32) -> Result<()> {
        self.start(Operation::InstallJava(major))
    }
    pub fn login(&mut self) -> Result<()> {
        self.start(Operation::Login)
    }
    pub fn forget(&mut self, id: String) -> Result<()> {
        if !self.accounts.iter().any(|a| a.id == id) {
            return Err("请先选择要退出的账户".into());
        }
        self.start(Operation::Forget(id))
    }
    pub fn launch(&mut self, choice: LaunchChoice) -> Result<()> {
        if !self
            .games
            .iter()
            .any(|g| g.id == choice.game.id && g.root == choice.game.root)
        {
            return Err("启动选择已失效".into());
        }
        if !self.accounts.iter().any(|a| a.id == choice.account_id) {
            return Err("账户选择已失效".into());
        }
        self.start(Operation::Launch(choice))
    }
    pub fn catalog(&mut self) -> Result<()> {
        self.start(Operation::Catalog)
    }
    pub fn loaders(&mut self, game: String, kind: String) -> Result<()> {
        if !self.catalog.iter().any(|v| v.id == game) {
            return Err("请先从列表选择游戏版本".into());
        }
        self.loaders.clear();
        self.loader_key = (game.clone(), kind.clone());
        self.catalog_revision += 1;
        self.start(Operation::Loaders(game, kind))
    }
    #[allow(clippy::too_many_arguments)]
    pub fn install(
        &mut self,
        version: String,
        parent: PathBuf,
        name: String,
        budget_gib: i32,
        os: String,
        title: String,
        icon: String,
        loader: Option<ambrum::loader::Choice>,
        java: PathBuf,
    ) -> Result<()> {
        ambrum::paths::name(&version)?;
        ambrum::paths::name(&name)?;
        if !self.catalog.iter().any(|v| v.id == version) {
            return Err("请从官方版本列表选择游戏".into());
        }
        if let Some(l) = &loader
            && (self.loader_key != (version.clone(), l.kind.clone())
                || !self.loaders.contains(&l.version))
        {
            return Err("加载器选择已失效，请重新选择".into());
        }
        ambrum::loader::display(&mut serde_json::json!({}), &title, &icon)?;
        if !(1..=128).contains(&budget_gib) || os.is_empty() {
            return Err("请选择 1–128 GiB 的安装预算，并确认 Windows 版本".into());
        }
        self.start(Operation::Install {
            version,
            parent,
            name,
            budget: budget_gib as u64 * 1024 * 1024 * 1024,
            os,
            title,
            icon,
            loader,
            java,
        })
    }
    pub fn cancel(&mut self) {
        if let Some(worker) = &self.worker {
            worker.cancel.cancel();
            self.report(
                if self.running {
                    "正在请求游戏正常退出，请在游戏中保存；窗口将等待进程结束"
                } else {
                    "正在取消，请稍候"
                }
                .into(),
            );
        }
    }
    fn start(&mut self, operation: Operation) -> Result<()> {
        if self.busy() {
            return Err("已有操作正在进行，请等待或取消".into());
        }
        let app = self.app.clone();
        let cancel = Cancellation::default();
        let token = cancel.clone();
        let (sender, receiver) = mpsc::sync_channel(32);
        let handle = thread::Builder::new()
            .name("amberl-desktop-task".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    execute(&app, operation, &token, &sender)
                }))
                .unwrap_or_else(|_| Err("操作意外结束；请检查任务记录，已有数据已保留".into()));
                let _ = sender.send(Message::Finished(result));
            })
            .map_err(|_| "无法创建后台任务")?;
        self.worker = Some(Worker {
            cancel,
            receiver,
            handle,
        });
        self.running = false;
        self.device_prompt.clear();
        self.report("正在处理…".into());
        Ok(())
    }
    pub fn tick(&mut self) {
        let previous = self.network.revision;
        self.network.tick();
        if previous != self.network.revision {
            self.revision += 1;
        }
        let mut finished = false;
        for _ in 0..32 {
            let Some(worker) = &self.worker else { break };
            let message = match worker.receiver.try_recv() {
                Ok(message) => message,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.report("后台任务已断开".into());
                    finished = true;
                    break;
                }
            };
            match message {
                Message::Content(response) => {
                    if worker.cancel.check().is_err() {
                        continue;
                    }
                    self.content.apply(response);
                    self.revision += 1;
                }
                Message::Data(games, accounts) => {
                    if worker.cancel.check().is_err() {
                        continue;
                    }
                    self.games = games;
                    self.accounts = accounts;
                    if !self
                        .games
                        .iter()
                        .any(|g| Some(&g.id) == self.selected.as_ref())
                    {
                        self.selected = None;
                    }
                    self.data_revision += 1;
                    self.revision += 1;
                }
                Message::Java(java) => {
                    if worker.cancel.check().is_err() {
                        continue;
                    }
                    self.java = java;
                    self.java_revision += 1;
                    self.revision += 1;
                }
                Message::Catalog(versions) => {
                    if worker.cancel.check().is_err() {
                        continue;
                    }
                    self.catalog = versions;
                    self.catalog_revision += 1;
                    self.revision += 1;
                }
                Message::Loaders(game, kind, versions) => {
                    if worker.cancel.check().is_err() {
                        continue;
                    }
                    self.loader_key = (game, kind);
                    self.loaders = versions;
                    self.catalog_revision += 1;
                    self.revision += 1;
                }
                Message::Prompt(prompt) => {
                    if worker.cancel.check().is_err() {
                        continue;
                    }
                    self.device_prompt = prompt;
                    self.revision += 1;
                }
                Message::Status(status) => self.report(status),
                Message::Started => {
                    self.running = true;
                    self.revision += 1;
                }
                Message::Finished(result) => {
                    self.report(match result {
                        Ok(()) => "操作完成".into(),
                        Err(error) => error,
                    });
                    finished = true;
                    break;
                }
            }
        }
        if finished {
            if let Some(worker) = self.worker.take() {
                let _ = worker.handle.join();
            }
            self.running = false;
            self.device_prompt.clear();
            self.revision += 1;
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

fn send(sender: &mpsc::SyncSender<Message>, message: Message) -> Result<()> {
    sender.send(message).map_err(|_| "窗口已关闭".into())
}
fn reload(
    app: &std::path::Path,
    cancel: &Cancellation,
    sender: &mpsc::SyncSender<Message>,
) -> Result<()> {
    let registry = Registry::open(app)?;
    let mut games = Vec::new();
    if registry.entries().len() > 64 {
        send(
            sender,
            Message::Status("本次最多扫描前 64 个游戏库；其余登记仍保留".into()),
        )?;
    }
    for entry in registry.entries().iter().take(64) {
        cancel.check()?;
        match registry.scan(&entry.id, cancel) {
            Ok(scan) => {
                for instance in scan.instances.into_iter().take(20000 - games.len()) {
                    let hint = entry.path.join("installation.ready.json").is_file();
                    let metadata = ambrum::paths::json(
                        &entry.path,
                        &entry
                            .path
                            .join("versions")
                            .join(&instance.name)
                            .join(format!("{}.json", instance.name)),
                    )
                    .ok();
                    let display = metadata.as_ref().and_then(|m| m.get("amberlDisplay"));
                    let title = display
                        .and_then(|d| d["name"].as_str())
                        .unwrap_or(&instance.name)
                        .to_owned();
                    let icon = display
                        .and_then(|d| d["icon"].as_str())
                        .filter(|i| ["grass", "dirt", "anvil", "pickaxe"].contains(i))
                        .unwrap_or("grass")
                        .to_owned();
                    let isolation = match instance.isolation {
                        Isolation::Isolated => "独立目录",
                        Isolation::Shared => "共享目录",
                        Isolation::Unknown => "隔离待确认",
                        Isolation::Conflict => "隔离冲突",
                    };
                    games.push(Game {
                        id: serde_json::to_string(&instance.id).map_err(|_| "无效条目身份")?,
                        library_id: entry.id.clone(),
                        root: entry.path.clone(),
                        version: instance.name.clone(),
                        title,
                        icon,
                        subtitle: format!(
                            "{} · {} · {}",
                            instance.game,
                            instance.loader,
                            if hint {
                                "安装记录待核验"
                            } else {
                                isolation
                            }
                        ),
                        installation_hint: hint,
                    });
                }
            }
            Err(error) => {
                let _ = sender.try_send(Message::Status(format!("{}：{error}", entry.label)));
            }
        }
        if games.len() >= 20000 {
            send(
                sender,
                Message::Status("已达到本次 20,000 个游戏条目上限".into()),
            )?;
            break;
        }
    }
    let accounts = Store::open(app)?.profiles();
    cancel.check()?;
    send(sender, Message::Data(games, accounts))
}
fn execute(
    app: &std::path::Path,
    operation: Operation,
    cancel: &Cancellation,
    sender: &mpsc::SyncSender<Message>,
) -> Result<()> {
    match operation {
        Operation::Reload => reload(app, cancel, sender),
        Operation::Catalog => send(
            sender,
            Message::Catalog(ambrum::catalog::list(&ambrum::download::Https, cancel)?),
        ),
        Operation::Loaders(game, kind) => {
            let versions =
                ambrum::loader::versions(&game, &kind, &ambrum::download::Https, cancel)?;
            send(sender, Message::Loaders(game, kind, versions))
        }
        Operation::Attach(path) => {
            Registry::open(app)?.attach(&path)?;
            reload(app, cancel, sender)
        }
        Operation::Content(request) => content::execute(app, request, cancel, sender),
        Operation::Java => {
            let mut rows = ambrum::java::runtime::discover(app, cancel)?;
            rows.extend(ambrum::java::discover(cancel)?);
            send(sender, Message::Java(rows))
        }
        Operation::InstallJava(major) => {
            ambrum::java::runtime::install(app, major, cancel, |done, total| {
                let _ = sender.try_send(Message::Status(format!(
                    "正在下载并校验 Java {major}：{done}/{total}"
                )));
            })?;
            let mut rows = ambrum::java::runtime::discover(app, cancel)?;
            rows.extend(ambrum::java::discover(cancel)?);
            send(sender, Message::Java(rows))
        }
        Operation::Forget(id) => {
            Store::open(app)?.forget(&id)?;
            reload(app, cancel, sender)
        }
        Operation::Login => {
            drop(Store::open(app)?);
            ambrum::secure::check_available()?;
            let mut flow = DeviceFlow::begin(&ambrum::auth::http::Https, cancel)?;
            let prompt = flow.prompt();
            send(
                sender,
                Message::Prompt(format!(
                    "在微软官方页面输入设备码：\n{}\n{}\n有效期 {} 分钟；请勿将密码或验证码发送给他人。",
                    prompt.user_code,
                    prompt.verification_uri,
                    prompt.expires_in / 60
                )),
            )?;
            loop {
                match flow.poll(&ambrum::auth::http::Https, cancel)? {
                    Poll::Waiting => thread::sleep(Duration::from_millis(200)),
                    Poll::Authorized(authorization) => {
                        cancel.check()?;
                        Store::open(app)?.save_authorization(authorization)?;
                        break;
                    }
                }
            }
            reload(app, cancel, sender)
        }
        Operation::Launch(choice) => {
            let registry = Registry::open(app)?;
            let scan = registry.scan(&choice.game.library_id, cancel)?;
            if !scan
                .instances
                .iter()
                .any(|i| serde_json::to_string(&i.id).ok().as_ref() == Some(&choice.game.id))
            {
                return Err("游戏身份已变化，请刷新后重试".into());
            }
            let selection = launch::Selection {
                app_directory: app,
                game_directory: &choice.game.root,
                version: &choice.game.version,
                java_executable: &choice.java,
                account_id: &choice.account_id,
                max_memory_mib: choice.memory_mib,
                os_version: &choice.os_version,
            };
            let exit = launch::run_join(&selection, choice.target.as_ref(), cancel, |event| {
                let status = match event {
                    launch::Event::CheckingResources => "正在核验游戏文件…".into(),
                    launch::Event::Authenticating => "正在刷新并验证微软账户…".into(),
                    launch::Event::Started { process_id, .. } => {
                        let _ = sender.send(Message::Started);
                        format!("游戏进程已启动（{process_id}）；进入世界状态未知")
                    }
                    launch::Event::CloseRequested { .. } => {
                        "已请求游戏正常退出，等待保存和结束…".into()
                    }
                };
                let _ = sender.try_send(Message::Status(status));
            })?;
            if exit.success {
                send(sender, Message::Status("游戏已正常退出".into()))?;
                Ok(())
            } else {
                Err(format!(
                    "游戏退出，代码 {:?}。请查看游戏日志。",
                    exit.exit_code
                ))
            }
        }
        Operation::Install {
            version,
            parent,
            name,
            budget,
            os,
            title,
            icon,
            loader,
            java,
        } => {
            ambrum::install::check_destination(app, &parent, &name)?;
            let cache = Cache::open(app)?;
            let resolved =
                ambrum::catalog::resolve(&cache, &version, &ambrum::download::Https, cancel)?;
            let mut metadata = ambrum::paths::json(
                resolved.metadata_path.parent().ok_or("缺少元数据目录")?,
                &resolved.metadata_path,
            )?;
            let platform = Platform::windows_x64(&os);
            if let Some(choice) = loader {
                send(
                    sender,
                    Message::Status(format!("正在准备 {} {}…", choice.kind, choice.version)),
                )?;
                metadata = ambrum::loader::resolve(
                    &cache,
                    &metadata,
                    &choice,
                    &java,
                    &platform,
                    budget,
                    &ambrum::download::Https,
                    cancel,
                )?;
            }
            ambrum::loader::display(&mut metadata, &title, &icon)?;
            let mut last = Instant::now() - Duration::from_secs(1);
            ambrum::prepare::vanilla(
                &cache,
                &metadata,
                &platform,
                budget,
                &ambrum::download::Https,
                cancel,
                |progress| {
                    if last.elapsed() >= Duration::from_millis(200) {
                        last = Instant::now();
                        let _ = sender.try_send(Message::Status(format!(
                            "下载与校验：{} / {} 个文件",
                            progress.verified_files,
                            progress
                                .total_files
                                .map(|n| n.to_string())
                                .unwrap_or_else(|| "待计算".into())
                        )));
                    }
                },
            )?;
            let installed = ambrum::install::vanilla(
                &cache,
                &metadata,
                &platform,
                &parent,
                &name,
                budget,
                cancel,
                |progress| {
                    if last.elapsed() >= Duration::from_millis(200) {
                        last = Instant::now();
                        let _ = sender.try_send(Message::Status(format!(
                            "安装：{} / {} 个文件",
                            progress.files_written, progress.total_files
                        )));
                    }
                },
            )?;
            Registry::open(app)?
                .attach(&installed.game_directory)
                .map_err(|error| {
                    format!(
                        "安装已完成于 {}，登记失败，请手动添加：{error}",
                        installed.game_directory.display()
                    )
                })?;
            reload(app, cancel, sender)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn game() -> Game {
        Game {
            id: "library-A/profile-A".into(),
            library_id: "A".into(),
            root: "test-game".into(),
            version: "26.3".into(),
            title: "原版生存".into(),
            icon: "grass".into(),
            subtitle: String::new(),
            installation_hint: true,
        }
    }
    #[test]
    fn stale_loader_cannot_start_an_install_worker() {
        let mut m = Model::new(PathBuf::from("unused"));
        m.catalog = vec![ambrum::catalog::Version {
            id: "26.3".into(),
            kind: "release".into(),
        }];
        m.loaders = vec!["0.20.1".into()];
        m.loader_key = ("26.2".into(), "Fabric".into());
        assert!(
            m.install(
                "26.3".into(),
                "unused".into(),
                "new".into(),
                4,
                "10.0.26200".into(),
                "name".into(),
                "grass".into(),
                Some(ambrum::loader::Choice {
                    kind: "Fabric".into(),
                    version: "0.20.1".into()
                }),
                "java.exe".into()
            )
            .is_err()
        );
        assert!(!m.busy());
    }
    #[test]
    fn launch_captures_immutable_selection_and_rejects_missing_inputs() {
        let mut game = game();
        let choice =
            LaunchChoice::capture(&game, "java.exe", "account", 4096, "10.0.26100").unwrap();
        game.root = "different".into();
        assert_ne!(choice.game.root, game.root);
        assert_eq!(choice.game.version, "26.3");
        assert!(LaunchChoice::capture(&game, "", "account", 4096, "10").is_err());
        assert!(LaunchChoice::capture(&game, "java.exe", "", 4096, "10").is_err());
        assert!(LaunchChoice::capture(&game, "java.exe", "account", 0, "10").is_err());
        game.installation_hint = false;
        assert!(LaunchChoice::capture(&game, "java.exe", "account", 4096, "10").is_err());
    }
    #[test]
    fn stale_row_and_account_cannot_authorize_launch() {
        let mut model = Model::new("unused".into());
        let game = game();
        let choice = LaunchChoice::capture(&game, "java.exe", "account", 4096, "10").unwrap();
        assert!(model.select(&game.id).is_err());
        assert!(model.launch(choice.clone()).is_err());
        model.games.push(game);
        assert!(model.launch(choice).is_err());
        assert!(!model.busy());
    }
    #[test]
    fn task_log_is_bounded_in_lines_and_unicode_length() {
        let mut model = Model::new("unused".into());
        for _ in 0..200 {
            model.report("测".repeat(3000));
        }
        assert_eq!(model.log.len(), 120);
        assert_eq!(model.status.chars().count(), 2048);
    }

    #[test]
    fn cancelled_queued_results_preserve_selection_and_never_show_a_device_code() {
        let mut model = Model::new("unused".into());
        model.games.push(game());
        model.selected = Some(game().id);
        let (sender, receiver) = mpsc::sync_channel(32);
        sender.send(Message::Data(vec![], vec![])).unwrap();
        sender
            .send(Message::Prompt("private-device-code".into()))
            .unwrap();
        sender
            .send(Message::Finished(Err("Cancelled".into())))
            .unwrap();
        let cancel = Cancellation::default();
        cancel.cancel();
        model.worker = Some(Worker {
            cancel,
            receiver,
            handle: thread::spawn(move || drop(sender)),
        });
        model.tick();
        assert_eq!(model.selected_game().unwrap().id, game().id);
        assert!(model.device_prompt.is_empty());
        assert!(!model.log().contains("private-device-code"));
        assert!(!model.busy());
    }

    #[test]
    fn running_state_requires_process_event_and_finishing_releases_the_worker() {
        let mut model = Model::new("unused".into());
        let (sender, receiver) = mpsc::sync_channel(32);
        model.worker = Some(Worker {
            cancel: Cancellation::default(),
            receiver,
            handle: thread::spawn(|| {}),
        });
        sender
            .send(Message::Status("Authenticating".into()))
            .unwrap();
        model.tick();
        assert!(model.busy());
        assert!(!model.running);
        sender.send(Message::Started).unwrap();
        model.tick();
        assert!(model.running);
        sender.send(Message::Finished(Ok(()))).unwrap();
        model.tick();
        assert!(!model.running);
        assert!(!model.busy());
    }
}
