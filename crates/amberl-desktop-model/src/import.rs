// SPDX-License-Identifier: MPL-2.0
use super::*;
pub(super) struct Request {
    game: Game,
    parent: PathBuf,
    name: String,
    shared: bool,
    os: String,
}
impl Model {
    pub fn import_game(
        &mut self,
        parent: PathBuf,
        name: String,
        shared: bool,
        os: String,
    ) -> Result<()> {
        let game = self
            .selected_game()
            .filter(|g| g.copy_candidate)
            .cloned()
            .ok_or("当前实例不支持独立副本导入")?;
        self.start(Operation::Import(Request {
            game,
            parent,
            name,
            shared,
            os,
        }))
    }
}
pub(super) fn execute(
    app: &std::path::Path,
    request: Request,
    cancel: &Cancellation,
    sender: &mpsc::SyncSender<Message>,
) -> Result<()> {
    let Request {
        game,
        parent,
        name,
        shared,
        os,
    } = request;
    ambrum::install::check_destination(app, &parent, &name)?;
    let registry = Registry::open(app)?;
    if !registry
        .entries()
        .iter()
        .any(|e| e.id == game.library_id && e.path == game.root)
    {
        return Err("游戏库登记已变化，请重新选择".into());
    }
    drop(registry);
    let mut source = ambrum::import::Source::open(
        app,
        game.copy_source.as_ref().ok_or("缺少已确认的导入来源")?,
        shared,
        cancel,
    )?;
    source.check_parent(&parent)?;
    let cache = Cache::open(app)?;
    let resolved =
        ambrum::catalog::resolve(&cache, source.version(), &ambrum::download::Https, cancel)?;
    let mut metadata = ambrum::paths::json(
        resolved.metadata_path.parent().ok_or("缺少元数据目录")?,
        &resolved.metadata_path,
    )?;
    ambrum::loader::display(&mut metadata, &name, &game.icon)?;
    let platform = Platform::windows_x64(&os);
    let budget = 16 * 1024 * 1024 * 1024;
    send(
        sender,
        Message::Status("正在下载并校验副本的官方游戏资源…".into()),
    )?;
    ambrum::prepare::vanilla(
        &cache,
        &metadata,
        &platform,
        budget,
        &ambrum::download::Https,
        cancel,
        |_| {},
    )?;
    send(
        sender,
        Message::Status("正在创建独立副本，原游戏库保持只读…".into()),
    )?;
    let installed = ambrum::install::vanilla_with_data(
        &cache,
        &metadata,
        &platform,
        &parent,
        &name,
        budget,
        Some(&mut source),
        cancel,
        |_| {},
    )?;
    drop(source);
    Registry::open(app)?
        .attach(&installed.game_directory)
        .map_err(|error| {
            format!(
                "副本已完成于 {}，登记失败，请手动添加：{error}",
                installed.game_directory.display()
            )
        })?;
    reload(app, cancel, sender)
}
