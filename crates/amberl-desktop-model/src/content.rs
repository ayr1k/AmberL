// SPDX-License-Identifier: MPL-2.0
use super::*;
use ambrum::content::{self, modrinth as api};
#[derive(Default)]
pub struct Content {
    pub title: String,
    pub target: Option<content::Target>,
    pub context: Option<api::Context>,
    pub kind: String,
    pub browse: api::Browse,
    pub projects: Vec<api::Project>,
    pub versions: Vec<api::Version>,
    pub groups: Vec<content::Group>,
    pub plan: Option<api::Plan>,
    pub planned_version: String,
    pub revision: u64,
}
pub(super) enum Request {
    Open(content::Target),
    Search(content::Target, String, String, api::Browse),
    Versions(content::Target, String, String, api::Browse),
    Plan(
        content::Target,
        String,
        String,
        std::collections::BTreeSet<String>,
    ),
    Install(content::Target, api::Plan),
    Change(content::Target, String, String),
}
pub(super) enum Response {
    Open(api::Context, Vec<content::Group>),
    Projects(String, Vec<api::Project>),
    Versions(Vec<api::Version>),
    Plan(String, api::Plan),
    Groups(Vec<content::Group>),
}
impl Content {
    pub fn preview(&self) -> String {
        self.plan.as_ref().map_or_else(
            || "先选择项目与版本，再预览安装内容。".into(),
            |p| {
                let mut lines = p
                    .files
                    .iter()
                    .map(|f| format!("{}  ·  {}", f.title, f.path))
                    .collect::<Vec<_>>();
                if !p.optional.is_empty() {
                    lines.push("可选依赖可在下方选择；更改后会重新校验整份清单。".into());
                }
                lines.push(
                    "移除会按此安装组处理，并保留可恢复副本；其他安装组仍在使用的依赖会保留。"
                        .into(),
                );
                lines.join("\n")
            },
        )
    }
    pub(super) fn apply(&mut self, response: Response) {
        match response {
            Response::Open(context, groups) => {
                self.kind = if context.server && context.loader == "paper" {
                    "plugin"
                } else {
                    "mod"
                }
                .into();
                self.context = Some(context);
                self.groups = groups;
            }
            Response::Projects(kind, rows) => {
                self.kind = kind;
                self.projects = rows;
                self.versions.clear();
                self.plan = None;
                self.planned_version.clear();
            }
            Response::Versions(rows) => {
                self.versions = rows;
                self.plan = None;
                self.planned_version.clear();
            }
            Response::Plan(version, plan) => {
                self.plan = Some(plan);
                self.planned_version = version;
            }
            Response::Groups(rows) => self.groups = rows,
        }
        self.revision += 1;
    }
}
impl Model {
    pub fn open_content(&mut self, server: bool) -> Result<()> {
        if self.busy() {
            return Err("请等待当前操作完成".into());
        }
        let (target, title) = if server {
            let s = self
                .network
                .servers
                .iter()
                .find(|s| Some(&s.id) == self.network.selected.as_ref())
                .ok_or("请先选择服务端")?;
            (
                content::Target::Server { id: s.id.clone() },
                format!("服务端 · {} · {}", s.name, &s.id[..6]),
            )
        } else {
            let g = self.selected_game().ok_or("请先选择游戏")?;
            if !g.installation_hint {
                return Err("外部游戏库保持只读".into());
            }
            (
                content::Target::Client {
                    root: g.root.clone(),
                    version: g.version.clone(),
                },
                format!("游戏 · {}", g.title),
            )
        };
        self.content = Content {
            title,
            target: Some(target.clone()),
            kind: "mod".into(),
            revision: self.content.revision + 1,
            ..Default::default()
        };
        self.start(Operation::Content(Request::Open(target)))
    }
    fn content_target(&self) -> Result<content::Target> {
        self.content.target.clone().ok_or("请先选择内容目标".into())
    }
    pub fn search_content(
        &mut self,
        kind: String,
        query: String,
        browse: api::Browse,
    ) -> Result<()> {
        self.start(Operation::Content(Request::Search(
            self.content_target()?,
            kind.clone(),
            query,
            browse,
        )))?;
        self.content.kind = kind;
        self.content.browse = browse;
        self.content.projects.clear();
        self.content.versions.clear();
        self.content.plan = None;
        self.content.planned_version.clear();
        self.content.revision += 1;
        Ok(())
    }
    pub fn content_versions(&mut self, index: i32) -> Result<()> {
        let p = self
            .content
            .projects
            .get(usize::try_from(index).map_err(|_| "请选择项目")?)
            .ok_or("请选择项目")?;
        self.start(Operation::Content(Request::Versions(
            self.content_target()?,
            self.content.kind.clone(),
            p.id.clone(),
            self.content.browse,
        )))?;
        self.content.versions.clear();
        self.content.plan = None;
        self.content.planned_version.clear();
        self.content.revision += 1;
        Ok(())
    }
    pub fn preview_content(&mut self, index: i32) -> Result<()> {
        let v = self
            .content
            .versions
            .get(usize::try_from(index).map_err(|_| "请选择版本")?)
            .ok_or("请选择版本")?;
        self.start(Operation::Content(Request::Plan(
            self.content_target()?,
            self.content.kind.clone(),
            v.id.clone(),
            Default::default(),
        )))?;
        self.content.plan = None;
        self.content.planned_version.clear();
        self.content.revision += 1;
        Ok(())
    }
    pub fn install_content(&mut self, index: i32) -> Result<()> {
        let v = self
            .content
            .versions
            .get(usize::try_from(index).map_err(|_| "请选择版本")?)
            .ok_or("请选择版本")?;
        if v.id != self.content.planned_version {
            return Err("版本已变化，请重新预览依赖".into());
        }
        self.start(Operation::Content(Request::Install(
            self.content_target()?,
            self.content.plan.clone().ok_or("请先预览安装内容")?,
        )))
    }
    pub fn select_optional_content(&mut self, index: i32, selected: bool) -> Result<()> {
        let plan = self.content.plan.as_ref().ok_or("请先预览依赖")?;
        let choice = plan
            .optional
            .get(usize::try_from(index).map_err(|_| "请选择依赖")?)
            .ok_or("依赖选择已变化")?;
        let mut choices = plan
            .optional
            .iter()
            .filter(|o| o.selected)
            .map(|o| o.key.clone())
            .collect::<std::collections::BTreeSet<_>>();
        if selected {
            choices.insert(choice.key.clone());
        } else {
            choices.remove(&choice.key);
        }
        self.start(Operation::Content(Request::Plan(
            self.content_target()?,
            self.content.kind.clone(),
            self.content.planned_version.clone(),
            choices,
        )))?;
        self.content.plan = None;
        self.content.planned_version.clear();
        self.content.revision += 1;
        Ok(())
    }
    pub fn change_content(&mut self, index: i32, action: String) -> Result<()> {
        let g = self
            .content
            .groups
            .get(usize::try_from(index).map_err(|_| "请选择安装组")?)
            .ok_or("请选择安装组")?;
        self.start(Operation::Content(Request::Change(
            self.content_target()?,
            g.id.clone(),
            action,
        )))
    }
}
pub(super) fn execute(
    app: &std::path::Path,
    request: Request,
    c: &Cancellation,
    sender: &mpsc::SyncSender<Message>,
) -> Result<()> {
    let response =
        match request {
            Request::Open(t) => {
                Response::Open(content::context(app, &t, c)?, content::list(app, &t, c)?)
            }
            Request::Search(t, kind, query, browse) => Response::Projects(
                kind.clone(),
                api::search_with_filters(&query, &kind, &content::context(app, &t, c)?, browse, c)?,
            ),
            Request::Versions(t, kind, id, browse) => Response::Versions(
                api::versions_with_filters(&id, &kind, &content::context(app, &t, c)?, browse, c)?,
            ),
            Request::Plan(t, kind, id, choices) => Response::Plan(
                id.clone(),
                api::plan_with_choices(&id, &kind, &content::context(app, &t, c)?, &choices, c)?,
            ),
            Request::Install(t, plan) => {
                content::install(app, &t, &plan, c)?;
                Response::Groups(content::list(app, &t, c)?)
            }
            Request::Change(t, id, action) => {
                content::change(app, &t, &id, &action, c)?;
                Response::Groups(content::list(app, &t, c)?)
            }
        };
    send(sender, Message::Content(response))
}
