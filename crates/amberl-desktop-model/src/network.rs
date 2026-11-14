// SPDX-License-Identifier: MPL-2.0
//! Independent server, room and query lifecycles; a running server does not block game launch.
use ambrum::{
    Result,
    multiplayer::{self, Address, room, server},
    task::Cancellation,
};
use std::{collections::VecDeque, path::PathBuf, sync::mpsc, thread};
enum Message {
    Configuration(server::config::Snapshot),
    Loaders(String, String, Vec<String>),
    Servers(Vec<server::Server>),
    Recovery(Vec<server::backup::Backup>, Vec<server::Server>),
    Server(server::Event),
    RoomStarted(String),
    Probe(String, Result<multiplayer::Status>),
    Finished(Result<()>),
}
struct Worker {
    cancel: Cancellation,
    receiver: mpsc::Receiver<Message>,
    handle: thread::JoinHandle<()>,
}
fn worker(
    f: impl FnOnce(Cancellation, mpsc::SyncSender<Message>) -> Result<()> + Send + 'static,
) -> Result<Worker> {
    let cancel = Cancellation::default();
    let token = cancel.clone();
    let (tx, receiver) = mpsc::sync_channel(256);
    let handle = thread::Builder::new()
        .name("amberl-network".into())
        .spawn(move || {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(token, tx.clone())))
                    .unwrap_or_else(|_| Err("联机任务意外结束；请检查状态".into()));
            let _ = tx.send(Message::Finished(result));
        })
        .map_err(|_| "无法创建联机任务")?;
    Ok(Worker {
        cancel,
        receiver,
        handle,
    })
}
pub struct Network {
    app: PathBuf,
    pub servers: Vec<server::Server>,
    pub selected: Option<String>,
    pub status: String,
    pub server_status: String,
    pub room_status: String,
    pub query_status: String,
    pub invite: String,
    pub join_address: String,
    pub component: bool,
    pub revision: u64,
    pub loader_key: (String, String),
    pub loaders: Vec<String>,
    pub backups: Vec<server::backup::Backup>,
    pub archived: Vec<server::Server>,
    pub configuration: Option<server::config::Snapshot>,
    pub configuration_revision: u64,
    maintenance: Option<Worker>,
    server: Option<Worker>,
    room: Option<Worker>,
    query: Option<Worker>,
    commands: Option<mpsc::SyncSender<String>>,
    log: VecDeque<String>,
}
impl Network {
    pub fn new(app: PathBuf) -> Self {
        let component = room::installed(&app);
        Self {
            app,
            servers: vec![],
            selected: None,
            status: "创建服务端，或使用邀请加入房间".into(),
            server_status: "未启动".into(),
            room_status: "未加入房间".into(),
            query_status: "尚未查询".into(),
            invite: String::new(),
            join_address: String::new(),
            component,
            revision: 1,
            loader_key: (String::new(), String::new()),
            loaders: vec![],
            backups: vec![],
            archived: vec![],
            configuration: None,
            configuration_revision: 0,
            maintenance: None,
            server: None,
            room: None,
            query: None,
            commands: None,
            log: VecDeque::new(),
        }
    }
    pub fn busy(&self) -> bool {
        self.maintenance.is_some()
    }
    pub fn server_active(&self) -> bool {
        self.server.is_some()
    }
    pub fn room_active(&self) -> bool {
        self.room.is_some()
    }
    pub fn active(&self) -> bool {
        self.busy() || self.server_active() || self.room_active() || self.query.is_some()
    }
    pub fn report(&mut self, value: String) {
        self.status = value.chars().take(2048).collect();
        self.revision += 1;
    }
    pub fn log(&self) -> String {
        self.log.iter().cloned().collect::<Vec<_>>().join("\n")
    }
    fn maintenance(
        &mut self,
        f: impl FnOnce(PathBuf, Cancellation, mpsc::SyncSender<Message>) -> Result<()> + Send + 'static,
    ) -> Result<()> {
        if self.busy() {
            return Err("请等待当前联机操作完成".into());
        }
        let app = self.app.clone();
        self.maintenance = Some(worker(move |c, t| f(app, c, t))?);
        self.report("正在处理联机配置…".into());
        Ok(())
    }
    pub fn reload(&mut self) -> Result<()> {
        self.maintenance(|app, _, tx| {
            tx.send(Message::Servers(server::list(&app)?))
                .map_err(|_| "Window closed")?;
            tx.send(Message::Recovery(
                server::backup::list(&app)?,
                server::archived(&app)?,
            ))
            .map_err(|_| "Window closed")?;
            Ok(())
        })
    }
    pub fn load_configuration(&mut self, recover: bool) -> Result<()> {
        let id = self.selected.clone().ok_or("请选择服务端")?;
        self.maintenance(move |app, _, tx| {
            if recover {
                server::config::recover(&app, &id)?;
            }
            tx.send(Message::Configuration(server::config::read(&app, &id)?))
                .map_err(|_| "Window closed")?;
            Ok(())
        })?;
        self.configuration = None;
        Ok(())
    }
    pub fn save_configuration(&mut self, settings: server::config::Settings) -> Result<()> {
        let expected = self.configuration.clone().ok_or("请先读取配置")?;
        if Some(&expected.server) != self.selected.as_ref() {
            return Err("服务端已变化，请重新读取配置".into());
        }
        self.maintenance(move |app, _, tx| {
            tx.send(Message::Configuration(server::config::save(
                &app, &expected, &settings,
            )?))
            .map_err(|_| "Window closed")?;
            Ok(())
        })
    }
    pub fn backup(&mut self) -> Result<()> {
        let id = self.selected.clone().ok_or("请选择服务端")?;
        if self.server_active() {
            return Err("请先保存并停止服务端，再备份".into());
        }
        self.maintenance(move |app, c, tx| {
            server::backup::create(&app, &id, &c)?;
            tx.send(Message::Recovery(
                server::backup::list(&app)?,
                server::archived(&app)?,
            ))
            .map_err(|_| "Window closed")?;
            Ok(())
        })
    }
    pub fn restore_backup(&mut self, index: i32, name: String, port: i32) -> Result<()> {
        let id = self
            .backups
            .get(usize::try_from(index).map_err(|_| "请选择备份")?)
            .ok_or("请选择备份")?
            .id
            .clone();
        let port = u16::try_from(port).map_err(|_| "无效端口")?;
        self.maintenance(move |app, c, tx| {
            server::backup::restore(&app, &id, &name, port, &c)?;
            tx.send(Message::Servers(server::list(&app)?))
                .map_err(|_| "Window closed")?;
            Ok(())
        })
    }
    pub fn archive(&mut self, undo: Option<i32>) -> Result<()> {
        let id = if let Some(index) = undo {
            self.archived
                .get(usize::try_from(index).map_err(|_| "请选择移除的服务端")?)
                .ok_or("请选择移除的服务端")?
                .id
                .clone()
        } else {
            self.selected.clone().ok_or("请选择服务端")?
        };
        self.maintenance(move |app, _, tx| {
            server::set_archived(&app, &id, undo.is_none())?;
            tx.send(Message::Servers(server::list(&app)?))
                .map_err(|_| "Window closed")?;
            tx.send(Message::Recovery(
                server::backup::list(&app)?,
                server::archived(&app)?,
            ))
            .map_err(|_| "Window closed")?;
            Ok(())
        })
    }
    pub fn load_loaders(&mut self, game: String, kind: String) -> Result<()> {
        self.maintenance(move |_, c, tx| {
            let rows = ambrum::loader::versions(&game, &kind, &ambrum::download::Https, &c)?;
            tx.send(Message::Loaders(game, kind, rows))
                .map_err(|_| "Window closed")?;
            Ok(())
        })
    }
    pub fn create(&mut self, request: server::Create) -> Result<()> {
        self.maintenance(move |app, c, tx| {
            server::create_with_loader(&app, &request, &c)?;
            tx.send(Message::Servers(server::list(&app)?))
                .map_err(|_| "Window closed")?;
            Ok(())
        })
    }
    pub fn install_component(&mut self) -> Result<()> {
        self.maintenance(move |app, c, _| room::install(&app, &c))
    }
    pub fn select(&mut self, index: i32) -> Result<()> {
        if self.server_active() {
            return Err("请先停止当前服务端".into());
        }
        self.selected = self
            .servers
            .get(usize::try_from(index).map_err(|_| "请选择服务端")?)
            .map(|s| s.id.clone());
        self.revision += 1;
        Ok(())
    }
    pub fn start_server(&mut self, java: PathBuf, memory: i32) -> Result<()> {
        if self.server_active() || self.busy() {
            return Err("服务端正在运行或配置中".into());
        }
        let id = self.selected.clone().ok_or("请选择服务端")?;
        let app = self.app.clone();
        let memory = u32::try_from(memory).map_err(|_| "无效内存")?;
        let (commands, rx) = mpsc::sync_channel(32);
        self.server = Some(worker(move |cancel, tx| {
            let code = server::run(&app, &id, &java, memory, &cancel, &rx, |e| {
                let _ = tx.try_send(Message::Server(e));
            })?;
            if code != 0 {
                return Err(format!("服务端退出码 {code}；请检查控制台"));
            }
            Ok(())
        })?);
        self.commands = Some(commands);
        self.server_status = "正在校验并启动".into();
        self.log.clear();
        self.revision += 1;
        Ok(())
    }
    pub fn stop_server(&mut self) {
        if let Some(w) = &self.server {
            w.cancel.cancel();
            self.server_status = "正在请求保存并停止，等待进程退出".into();
            self.revision += 1;
        }
    }
    pub fn command(&mut self, command: String) -> Result<()> {
        server::console(&command)?;
        self.commands
            .as_ref()
            .ok_or("服务端未运行")?
            .try_send(command)
            .map_err(|_| "控制台繁忙，请稍后重试".into())
    }
    pub fn host(&mut self, port: i32, peer: String) -> Result<()> {
        let invite = room::Invite::host(u16::try_from(port).map_err(|_| "无效共享端口")?, &peer)?;
        self.start_room(invite, None)?;
        self.revision += 1;
        Ok(())
    }
    pub fn join(&mut self, code: String, port: i32) -> Result<()> {
        let invite = room::Invite::parse(&code)?;
        let port = u16::try_from(port).map_err(|_| "无效本地端口")?;
        self.start_room(invite, Some(port))?;
        self.join_address = format!("127.0.0.1:{port}");
        self.revision += 1;
        Ok(())
    }
    fn start_room(&mut self, invite: room::Invite, guest: Option<u16>) -> Result<()> {
        if self.room_active() || self.busy() {
            return Err("请先离开当前房间或等待配置完成".into());
        }
        if !self.component {
            return Err("请先下载联机组件".into());
        }
        let app = self.app.clone();
        self.room = Some(worker(move |c, tx| {
            let invitation = if guest.is_none() {
                multiplayer::probe(&Address {
                    host: "127.0.0.1".into(),
                    port: invite.port,
                })
                .map_err(
                    |_| "共享端口没有响应 Minecraft 状态；请先启动服务端或在游戏中开放局域网",
                )?;
                invite.code()?
            } else {
                String::new()
            };
            let code = room::run(&app, invite, guest, &c, |_| {
                let _ = tx.send(Message::RoomStarted(invitation.clone()));
            })?;
            if code != 0 {
                return Err(format!("联机组件退出码 {code}；请检查节点地址和网络"));
            }
            Ok(())
        })?);
        self.invite.clear();
        self.join_address.clear();
        self.query_status = "目标可达性尚未检查".into();
        self.room_status = "正在校验并启动联机组件".into();
        self.revision += 1;
        Ok(())
    }
    pub fn leave(&mut self) {
        if let Some(w) = &self.room {
            w.cancel.cancel();
            self.room_status = "正在离开，等待联机进程退出".into();
            self.revision += 1;
        }
    }
    pub fn probe(&mut self, value: String) -> Result<()> {
        if self.query.is_some() {
            return Err("查询正在进行".into());
        }
        let address = Address::parse(&value)?;
        self.query = Some(worker(move |_, tx| {
            let result = multiplayer::probe(&address);
            let _ = tx.send(Message::Probe(address.text(), result));
            Ok(())
        })?);
        self.query_status = "正在查询 Minecraft 状态…".into();
        self.revision += 1;
        Ok(())
    }
    pub fn shutdown(&mut self) {
        self.stop_server();
        self.leave();
        if let Some(w) = &self.maintenance {
            w.cancel.cancel();
        }
        if let Some(w) = &self.query {
            w.cancel.cancel();
        }
    }
    pub fn tick(&mut self) {
        for slot in 0..4 {
            let mut done = None;
            let mut messages = vec![];
            {
                let worker = match slot {
                    0 => &self.maintenance,
                    1 => &self.server,
                    2 => &self.room,
                    _ => &self.query,
                };
                if let Some(w) = worker {
                    for _ in 0..128 {
                        match w.receiver.try_recv() {
                            Ok(Message::Finished(result)) => {
                                done = Some(result);
                                break;
                            }
                            Ok(m) => messages.push(m),
                            Err(mpsc::TryRecvError::Disconnected) => {
                                done = Some(Err("联机任务意外断开".into()));
                                break;
                            }
                            Err(mpsc::TryRecvError::Empty) => break,
                        }
                    }
                }
            }
            for message in messages {
                match message {
                    Message::Configuration(snapshot) => {
                        self.configuration = Some(snapshot);
                        self.configuration_revision += 1;
                    }
                    Message::Recovery(backups, archived) => {
                        self.backups = backups;
                        self.archived = archived;
                    }
                    Message::Loaders(game, kind, rows) => {
                        self.loader_key = (game, kind);
                        self.loaders = rows;
                    }
                    Message::Servers(rows) => {
                        self.servers = rows;
                        if !self
                            .servers
                            .iter()
                            .any(|s| Some(&s.id) == self.selected.as_ref())
                        {
                            self.selected = None;
                        }
                    }
                    Message::Server(server::Event::Started(pid)) => {
                        self.server_status = format!("进程 {pid} 已运行，正在准备世界")
                    }
                    Message::Server(server::Event::Ready) => {
                        self.server_status = "服务端报告就绪；外部可达性尚未确认".into()
                    }
                    Message::Server(server::Event::Stopping) => {
                        self.server_status = "正在保存并停止".into()
                    }
                    Message::Server(server::Event::Line(line)) => {
                        if self.log.len() >= 160 {
                            self.log.pop_front();
                        }
                        self.log.push_back(line);
                    }
                    Message::RoomStarted(invite) => {
                        self.invite = invite;
                        self.room_status = "联机进程运行中；尚不能证明对方可达".into()
                    }
                    Message::Probe(address, result) => {
                        self.query_status = match result {
                            Ok(s) => format!(
                                "本次查询 {address}：Minecraft {} 可响应 · {}/{} 人 · {} ms；入服仍需游戏确认",
                                s.version, s.players, s.maximum, s.latency_ms
                            ),
                            Err(e) => format!("尚未确认可达：{e}"),
                        }
                    }
                    Message::Finished(_) => {}
                }
                self.revision += 1;
            }
            if let Some(result) = done {
                let slot_ref = match slot {
                    0 => &mut self.maintenance,
                    1 => &mut self.server,
                    2 => &mut self.room,
                    _ => &mut self.query,
                };
                if let Some(w) = slot_ref.take() {
                    let _ = w.handle.join();
                }
                let error = result.err();
                match slot {
                    0 => {
                        self.component = room::installed(&self.app);
                        self.status = error.unwrap_or_else(|| "联机配置已完成".into());
                    }
                    1 => {
                        self.commands = None;
                        self.server_status = error.unwrap_or_else(|| "服务端已正常退出".into());
                    }
                    2 => {
                        self.invite.clear();
                        self.join_address.clear();
                        self.room_status = error.unwrap_or_else(|| "已离开房间".into());
                    }
                    _ => {
                        if let Some(e) = error {
                            self.query_status = e;
                        }
                    }
                }
                self.revision += 1;
            }
        }
    }
}
impl Drop for Network {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn server_and_room_workers_have_independent_cancellation_and_truthful_states() {
        let mut n = Network::new(PathBuf::from("missing"));
        n.server = Some(
            worker(|cancel, tx| {
                let _ = tx.send(Message::Server(server::Event::Started(123)));
                while cancel.check().is_ok() {
                    thread::sleep(std::time::Duration::from_millis(5));
                }
                Ok(())
            })
            .unwrap(),
        );
        n.room = Some(
            worker(|cancel, tx| {
                let _ = tx.send(Message::RoomStarted("test invitation".into()));
                while cancel.check().is_ok() {
                    thread::sleep(std::time::Duration::from_millis(5));
                }
                Ok(())
            })
            .unwrap(),
        );
        for _ in 0..20 {
            n.tick();
            if n.invite == "test invitation" {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!n.busy());
        assert!(n.server_status.contains("正在准备世界"));
        assert!(n.room_status.contains("尚不能证明"));
        n.leave();
        for _ in 0..40 {
            n.tick();
            if !n.room_active() {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!n.room_active());
        assert!(n.invite.is_empty());
        assert!(n.server_active());
        n.shutdown();
        for _ in 0..40 {
            n.tick();
            if !n.active() {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!n.active());
    }
}
