// SPDX-License-Identifier: MPL-2.0
use super::*;
use ambrum::maintenance as api;
#[derive(Default)]
pub struct Maintenance {
    pub target: Option<Game>,
    pub backups: Vec<api::Backup>,
    pub archived: Vec<api::Profile>,
    pub revision: u64,
}
pub(super) enum Request {
    List,
    Save(Game, String, String, bool),
    Backup(Game),
    Restore(String, PathBuf, String),
    Unarchive(api::Profile),
}
pub(super) struct Response {
    pub backups: Vec<api::Backup>,
    pub archived: Vec<api::Profile>,
}
impl Model {
    pub fn open_maintenance(&mut self) -> Result<()> {
        if self.busy() {
            return Err("请等待当前操作完成".into());
        }
        self.maintenance.target = self
            .selected_game()
            .filter(|g| g.installation_hint)
            .cloned();
        self.maintenance.revision += 1;
        self.start(Operation::Maintenance(Request::List))
    }
    fn maintenance_game(&self) -> Result<Game> {
        self.maintenance
            .target
            .clone()
            .ok_or_else(|| "请选择 AmberL 创建的游戏".into())
    }
    pub fn edit_instance(&mut self, title: String, icon: String, archived: bool) -> Result<()> {
        self.start(Operation::Maintenance(Request::Save(
            self.maintenance_game()?,
            title,
            icon,
            archived,
        )))
    }
    pub fn backup_instance(&mut self) -> Result<()> {
        self.start(Operation::Maintenance(Request::Backup(
            self.maintenance_game()?,
        )))
    }
    pub fn restore_instance(&mut self, index: i32, parent: PathBuf, name: String) -> Result<()> {
        let b = self
            .maintenance
            .backups
            .get(usize::try_from(index).map_err(|_| "请选择备份")?)
            .ok_or("请选择备份")?;
        self.start(Operation::Maintenance(Request::Restore(
            b.id.clone(),
            parent,
            name,
        )))
    }
    pub fn unarchive_instance(&mut self, index: i32) -> Result<()> {
        let p = self
            .maintenance
            .archived
            .get(usize::try_from(index).map_err(|_| "请选择已移除游戏")?)
            .ok_or("请选择已移除游戏")?
            .clone();
        self.start(Operation::Maintenance(Request::Unarchive(p)))
    }
}
pub(super) fn execute(
    app: &std::path::Path,
    request: Request,
    c: &Cancellation,
    sender: &mpsc::SyncSender<Message>,
) -> Result<()> {
    let verify = |g: &Game| -> Result<()> {
        let registry = Registry::open(app)?;
        let entry = registry
            .entries()
            .iter()
            .find(|e| e.id == g.library_id)
            .ok_or("游戏库登记已变化")?;
        if entry.path != g.root
            || !registry
                .scan(&g.library_id, c)?
                .instances
                .iter()
                .any(|i| serde_json::to_string(&i.id).ok().as_ref() == Some(&g.id))
        {
            return Err("游戏身份已变化，请重新选择".into());
        }
        Ok(())
    };
    match request {
        Request::List => {}
        Request::Save(g, title, icon, archived) => {
            verify(&g)?;
            api::save_profile(app, &g.root, &g.version, &title, &icon, archived, c)?
        }
        Request::Backup(g) => {
            verify(&g)?;
            let id = api::backup(app, &g.root, &g.version, c)?;
            send(
                sender,
                Message::Status(format!("备份已创建 · {}", &id[..8])),
            )?;
        }
        Request::Restore(id, parent, name) => {
            let root = api::restore(app, &id, &parent, &name, c)?;
            send(
                sender,
                Message::Status(format!("已恢复为独立副本：{}", root.display())),
            )?;
        }
        Request::Unarchive(p) => {
            api::save_profile(app, &p.root, &p.version, &p.title, &p.icon, false, c)?
        }
    }
    send(
        sender,
        Message::Maintenance(Response {
            backups: api::backups(app)?,
            archived: api::profiles(app)?
                .into_iter()
                .filter(|p| p.archived)
                .collect(),
        }),
    )?;
    reload(app, c, sender)
}
