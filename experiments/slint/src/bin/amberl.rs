// SPDX-License-Identifier: MPL-2.0
#![cfg_attr(windows, windows_subsystem = "windows")]
use amberl_desktop_model::{LaunchChoice, Model};
use slint::Model as _;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};

#[path = "../glass.rs"]
mod glass;
slint::slint! { export { Launcher, GameRow, GlassAssets, VersionRow, OptionalRow } from "ui/launcher.slint"; }

#[derive(Default)]
struct View {
    revision: u64,
    data_revision: u64,
    java_revision: u64,
    catalog_revision: u64,
    account: Option<String>,
    selected: Option<String>,
    network_revision: u64,
    server_loader_key: (String, String),
    server_loaders: Vec<String>,
    backup_ids: Vec<String>,
    archived_ids: Vec<String>,
    content_revision: u64,
    content_projects: Vec<String>,
    content_versions: Vec<String>,
    content_groups: Vec<String>,
    content_plan: String,
    configuration_key: (String, String),
    configuration_revision: u64,
}
fn strings(values: Vec<String>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        values.into_iter().map(Into::into).collect::<Vec<_>>(),
    ))
}
fn filter_versions(ui: &Launcher, model: &Model) {
    let query = ui.get_version_search().to_lowercase();
    let filter = ui.get_version_filter();
    ui.set_versions(ModelRc::new(VecModel::from(
        model
            .catalog
            .iter()
            .filter(|v| {
                v.id.to_lowercase().contains(&query)
                    && match filter {
                        0 => v.kind == "release",
                        1 => v.kind == "snapshot",
                        2 => v.kind.starts_with("old_"),
                        _ => true,
                    }
            })
            .map(|v| VersionRow {
                id: v.id.as_str().into(),
                kind: v.kind.as_str().into(),
            })
            .collect::<Vec<_>>(),
    )));
}
fn refresh(ui: &Launcher, model: &Model, view: &mut View) {
    if view.revision == model.revision && view.network_revision == model.network.revision {
        return;
    }
    if view.content_revision != model.content.revision {
        let c = &model.content;
        let projects = c.projects.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
        let versions = c.versions.iter().map(|v| v.id.clone()).collect::<Vec<_>>();
        let groups = c.groups.iter().map(|g| g.id.clone()).collect::<Vec<_>>();
        if projects != view.content_projects {
            ui.set_content_project_index(-1);
            view.content_projects = projects;
        }
        if versions != view.content_versions {
            ui.set_content_version_index(-1);
            view.content_versions = versions;
        }
        if groups != view.content_groups {
            ui.set_content_group_index(-1);
            view.content_groups = groups;
        }
        if c.planned_version != view.content_plan {
            ui.set_content_step(if c.plan.is_some() { 1 } else { 0 });
            view.content_plan = c.planned_version.clone();
        }
        ui.set_content_target(c.title.as_str().into());
        ui.set_content_group_states(strings(c.groups.iter().map(|g| g.state.clone()).collect()));
        ui.set_content_projects(strings(
            c.projects.iter().map(|p| p.title.clone()).collect(),
        ));
        ui.set_content_versions(strings(
            c.versions.iter().map(|v| v.title.clone()).collect(),
        ));
        ui.set_content_groups(strings(
            c.groups
                .iter()
                .map(|g| {
                    format!(
                        "{} · {}",
                        g.title,
                        match g.state.as_str() {
                            "installed" => "已安装",
                            "pending" => "安装未完成",
                            "removing" => "移除未完成",
                            "removed" => "已移除，可恢复",
                            _ => "恢复未完成",
                        }
                    )
                })
                .collect(),
        ));
        ui.set_content_preview(c.preview().into());
        ui.set_content_optional(ModelRc::new(VecModel::from(c.plan.as_ref().map_or_else(
            Vec::new,
            |p| {
                p.optional
                    .iter()
                    .map(|o| OptionalRow {
                        title: o.title.as_str().into(),
                        selected: o.selected,
                    })
                    .collect()
            },
        ))));
        ui.set_content_has_plan(c.plan.is_some());
        view.content_revision = c.revision;
    }
    if view.selected != model.selected && model.selected_game().is_some() {
        if let Some(p) = model
            .selected
            .as_ref()
            .and_then(|id| model.preferences.get(id))
        {
            ui.set_java_path(p.java.to_string_lossy().as_ref().into());
            ui.set_memory_mib(p.memory_mib as i32);
            view.account = p.account.clone();
        }
        view.selected = model.selected.clone();
        view.data_revision = 0;
    }
    if view.network_revision != model.network.revision {
        let n = &model.network;
        let config = n
            .configuration
            .as_ref()
            .filter(|c| Some(&c.server) == n.selected.as_ref());
        ui.set_server_config_ready(config.is_some());
        let key = config.map_or_else(Default::default, |c| (c.server.clone(), c.digest.clone()));
        if key != view.configuration_key || n.configuration_revision != view.configuration_revision
        {
            if let Some(c) = config {
                ui.set_server_motd(c.settings.motd.as_str().into());
                ui.set_server_players(c.settings.players.into());
                ui.set_server_distance(c.settings.distance.into());
                ui.set_server_difficulty(c.settings.difficulty.as_str().into());
                ui.set_server_mode(c.settings.mode.as_str().into());
            }
            view.configuration_key = key;
            view.configuration_revision = n.configuration_revision;
        }
        let backup_ids = n.backups.iter().map(|b| b.id.clone()).collect::<Vec<_>>();
        let archived_ids = n.archived.iter().map(|b| b.id.clone()).collect::<Vec<_>>();
        if view.backup_ids != backup_ids {
            ui.set_backup_index(-1);
            view.backup_ids = backup_ids;
        }
        if view.archived_ids != archived_ids {
            ui.set_archived_index(-1);
            view.archived_ids = archived_ids;
        }
        ui.set_backup_options(strings(
            n.backups
                .iter()
                .map(|b| format!("{} · {} · {}", b.name, b.version, &b.id[..6]))
                .collect(),
        ));
        ui.set_archived_options(strings(
            n.archived
                .iter()
                .map(|s| format!("{} · {}", s.name, &s.id[..6]))
                .collect(),
        ));
        if view.server_loader_key != n.loader_key || view.server_loaders != n.loaders {
            ui.set_server_loader_options(strings(n.loaders.clone()));
            ui.set_server_loader_index(if n.loaders.is_empty() { -1 } else { 0 });
            view.server_loader_key = n.loader_key.clone();
            view.server_loaders = n.loaders.clone();
        }
        ui.set_server_options(strings(
            n.servers
                .iter()
                .map(|s| {
                    format!(
                        "{} · {} {} · :{} · {}",
                        s.name,
                        s.version,
                        s.loader.as_ref().map_or("原版", |l| l.choice.kind.as_str()),
                        s.port,
                        &s.id[..6]
                    )
                })
                .collect(),
        ));
        ui.set_server_index(
            n.selected
                .as_ref()
                .and_then(|id| n.servers.iter().position(|s| &s.id == id))
                .map_or(-1, |i| i as i32),
        );
        ui.set_server_active(n.server_active());
        ui.set_room_active(n.room_active());
        ui.set_network_busy(n.busy());
        ui.set_network_status(n.status.as_str().into());
        ui.set_server_status(n.server_status.as_str().into());
        ui.set_room_status(n.room_status.as_str().into());
        ui.set_query_status(n.query_status.as_str().into());
        ui.set_server_log(n.log().into());
        ui.set_room_invite(n.invite.as_str().into());
        ui.set_component_ready(n.component);
        ui.set_room_address(n.join_address.as_str().into());
        view.network_revision = n.revision;
    }
    if view.data_revision != model.data_revision {
        ui.set_games(ModelRc::new(VecModel::from(
            model
                .games
                .iter()
                .map(|g| GameRow {
                    id: g.id.as_str().into(),
                    title: g.title.as_str().into(),
                    subtitle: g.subtitle.as_str().into(),
                    icon: g.icon.as_str().into(),
                })
                .collect::<Vec<_>>(),
        )));
        let index = view
            .account
            .as_ref()
            .and_then(|id| model.accounts.iter().position(|a| &a.id == id));
        if index.is_none() {
            view.account = None;
        }
        let mut accounts = vec!["选择账户".into()];
        accounts.extend(model.accounts.iter().map(|a| a.name.clone()));
        ui.set_account_options(strings(accounts));
        ui.set_account_index(index.map_or(0, |i| i as i32 + 1));
        ui.set_account_label(
            index
                .map_or("登录微软账户", |i| model.accounts[i].name.as_str())
                .into(),
        );
        view.data_revision = model.data_revision;
    }
    if view.java_revision != model.java_revision {
        let mut java = vec!["选择检测到的 Java".into()];
        java.extend(model.java.iter().map(|c| {
            format!(
                "{} · {}",
                c.java.as_ref().map_or_else(
                    || "无法使用".into(),
                    |j| format!("Java {} ({})", j.version, j.architecture)
                ),
                c.executable.display()
            )
        }));
        ui.set_java_options(strings(java));
        ui.set_java_index(0);
        view.java_revision = model.java_revision;
    }
    if view.catalog_revision != model.catalog_revision {
        filter_versions(ui, model);
        ui.set_server_versions(strings(
            model
                .catalog
                .iter()
                .filter(|v| v.kind == "release")
                .map(|v| v.id.clone())
                .collect(),
        ));
        ui.set_server_version_index(-1);
        if model.loader_key
            == (
                ui.get_install_version().to_string(),
                ui.get_loader_kind().to_string(),
            )
        {
            ui.set_loader_options(strings(model.loaders.clone()));
            ui.set_loader_index(if model.loaders.is_empty() { -1 } else { 0 });
        }
        view.catalog_revision = model.catalog_revision;
    }
    let selected = model.selected_game();
    ui.set_selected_id(selected.map_or("", |g| g.id.as_str()).into());
    ui.set_game_title(
        selected
            .map_or("你的下一段冒险", |g| g.title.as_str())
            .into(),
    );
    ui.set_game_subtitle(
        selected
            .map_or("安装游戏，或添加已有游戏库", |g| {
                g.subtitle.as_str()
            })
            .into(),
    );
    ui.set_launch_candidate(selected.is_some_and(|g| g.installation_hint));
    ui.set_busy(model.busy());
    ui.set_running(model.running);
    ui.set_status(model.status.as_str().into());
    ui.set_task_log(model.log().into());
    ui.set_device_prompt(model.device_prompt.as_str().into());
    view.revision = model.revision;
}
fn save_choice(ui: &Launcher, model: &mut Model, view: &View) -> ambrum::Result<()> {
    model.save_preferences(ambrum::preferences::Launch {
        java: ui.get_java_path().as_str().into(),
        account: view.account.clone(),
        memory_mib: u32::try_from(ui.get_memory_mib()).map_err(|_| "无效内存")?,
    })
}
fn content_bindings(ui: &Launcher, model: &Rc<RefCell<Model>>) {
    {
        let model = model.clone();
        ui.on_select_optional(move |index, selected| {
            operation(&model, |m| m.select_optional_content(index, selected))
        });
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_open_content(move |server| {
            if let Some(u) = weak.upgrade() {
                let result = model.borrow_mut().open_content(server);
                match result {
                    Ok(()) => {
                        u.set_content_step(0);
                        u.set_content_tab(0);
                        u.set_panel(8);
                    }
                    Err(e) => model.borrow_mut().report(e),
                }
            }
        });
    }
    macro_rules! action {
        ($name:ident,$f:expr) => {{
            let model = model.clone();
            let weak = ui.as_weak();
            ui.$name(move || {
                if let Some(u) = weak.upgrade() {
                    operation(&model, |m| $f(&u, m));
                }
            });
        }};
    }
    action!(on_search_content, |u: &Launcher, m: &mut Model| {
        let kind = *["mod", "resourcepack", "shader"]
            .get(usize::try_from(u.get_content_kind_index()).map_err(|_| "请选择内容类型")?)
            .ok_or("请选择内容类型")?;
        m.search_content(kind.into(), u.get_content_query().to_string())
    });
    action!(on_content_project, |u: &Launcher, m: &mut Model| m
        .content_versions(u.get_content_project_index()));
    action!(on_preview_content, |u: &Launcher, m: &mut Model| m
        .preview_content(u.get_content_version_index()));
    action!(on_install_content, |u: &Launcher, m: &mut Model| m
        .install_content(u.get_content_version_index()));
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_change_content(move |action| {
            if let Some(u) = weak.upgrade() {
                operation(&model, |m| {
                    m.change_content(u.get_content_group_index(), action.to_string())
                });
            }
        });
    }
}
fn net(
    model: &Rc<RefCell<Model>>,
    f: impl FnOnce(&mut amberl_desktop_model::network::Network) -> ambrum::Result<()>,
) {
    let mut m = model.borrow_mut();
    if let Err(error) = f(&mut m.network) {
        m.network.report(error);
    }
}
fn network_callbacks(ui: &Launcher, model: &Rc<RefCell<Model>>) {
    macro_rules! action {
        ($name:ident,$f:expr) => {{
            let model = model.clone();
            let weak = ui.as_weak();
            ui.$name(move || {
                if let Some(ui) = weak.upgrade() {
                    net(&model, |n| ($f)(&ui, n));
                }
            });
        }};
    }
    action!(
        on_load_server_config,
        |_: &Launcher, n: &mut amberl_desktop_model::network::Network| n.load_configuration(false)
    );
    action!(
        on_recover_server_config,
        |_: &Launcher, n: &mut amberl_desktop_model::network::Network| n.load_configuration(true)
    );
    action!(
        on_save_server_config,
        |u: &Launcher, n: &mut amberl_desktop_model::network::Network| n.save_configuration(
            ambrum::multiplayer::server::config::Settings {
                motd: u.get_server_motd().to_string(),
                players: u16::try_from(u.get_server_players()).map_err(|_| "无效人数")?,
                distance: u8::try_from(u.get_server_distance()).map_err(|_| "无效视距")?,
                difficulty: u.get_server_difficulty().to_string(),
                mode: u.get_server_mode().to_string()
            }
        )
    );
    action!(
        on_refresh_servers,
        |_: &Launcher, n: &mut amberl_desktop_model::network::Network| n.reload()
    );
    action!(
        on_install_network,
        |_: &Launcher, n: &mut amberl_desktop_model::network::Network| n.install_component()
    );
    action!(
        on_create_server,
        |u: &Launcher, n: &mut amberl_desktop_model::network::Network| {
            let version = u
                .get_server_versions()
                .row_data(
                    usize::try_from(u.get_server_version_index())
                        .map_err(|_| "请选择服务端版本")?,
                )
                .ok_or("请选择服务端版本")?;
            let kind = u.get_server_loader_kind().to_string();
            let loader = if kind == "原版" {
                None
            } else {
                if n.loader_key != (version.to_string(), kind.clone()) {
                    return Err("请刷新当前游戏的加载器列表".into());
                }
                Some(ambrum::loader::Choice {
                    kind,
                    version: n
                        .loaders
                        .get(
                            usize::try_from(u.get_server_loader_index())
                                .map_err(|_| "请选择加载器版本")?,
                        )
                        .ok_or("请选择加载器版本")?
                        .clone(),
                })
            };
            n.create(ambrum::multiplayer::server::Create {
                name: u.get_server_name().to_string(),
                version: version.to_string(),
                port: u16::try_from(u.get_server_port()).map_err(|_| "无效端口")?,
                eula: u.get_server_eula(),
                loader,
                java: Some(u.get_java_path().as_str().into()),
            })
        }
    );
    action!(
        on_load_server_loaders,
        |u: &Launcher, n: &mut amberl_desktop_model::network::Network| {
            let version = u
                .get_server_versions()
                .row_data(
                    usize::try_from(u.get_server_version_index()).map_err(|_| "请选择游戏版本")?,
                )
                .ok_or("请选择游戏版本")?;
            n.load_loaders(version.to_string(), u.get_server_loader_kind().to_string())
        }
    );
    action!(
        on_start_server,
        |u: &Launcher, n: &mut amberl_desktop_model::network::Network| n
            .start_server(u.get_java_path().as_str().into(), u.get_server_memory())
    );
    action!(
        on_backup_server,
        |_: &Launcher, n: &mut amberl_desktop_model::network::Network| n.backup()
    );
    action!(
        on_archive_server,
        |_: &Launcher, n: &mut amberl_desktop_model::network::Network| n.archive(None)
    );
    action!(
        on_restore_archived,
        |u: &Launcher, n: &mut amberl_desktop_model::network::Network| n
            .archive(Some(u.get_archived_index()))
    );
    action!(
        on_restore_backup,
        |u: &Launcher, n: &mut amberl_desktop_model::network::Network| n.restore_backup(
            u.get_backup_index(),
            u.get_restore_name().to_string(),
            u.get_restore_port()
        )
    );
    action!(
        on_stop_server,
        |_: &Launcher, n: &mut amberl_desktop_model::network::Network| {
            n.stop_server();
            Ok(())
        }
    );
    action!(
        on_server_command,
        |u: &Launcher, n: &mut amberl_desktop_model::network::Network| {
            n.command(u.get_console_command().to_string())?;
            u.set_console_command("".into());
            Ok(())
        }
    );
    action!(
        on_host_room,
        |u: &Launcher, n: &mut amberl_desktop_model::network::Network| n
            .host(u.get_host_port(), u.get_room_peer().to_string())
    );
    action!(
        on_join_room,
        |u: &Launcher, n: &mut amberl_desktop_model::network::Network| n
            .join(u.get_invite_input().to_string(), u.get_guest_port())
    );
    action!(
        on_leave_room,
        |_: &Launcher, n: &mut amberl_desktop_model::network::Network| {
            n.leave();
            Ok(())
        }
    );
    action!(
        on_probe_server,
        |u: &Launcher, n: &mut amberl_desktop_model::network::Network| n
            .probe(u.get_join_target().to_string())
    );
    {
        let model = model.clone();
        ui.on_choose_server(move |i| net(&model, |n| n.select(i)));
    }
}
fn operation(model: &Rc<RefCell<Model>>, f: impl FnOnce(&mut Model) -> ambrum::Result<()>) {
    let mut model = model.borrow_mut();
    if let Err(error) = f(&mut model) {
        model.report(error);
    }
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn open(target: &str) -> ambrum::Result<()> {
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    let action = wide("open");
    if target.contains('\0') {
        return Err("无法打开无效路径".into());
    }
    let target = wide(target);
    // ShellExecute invokes the registered URL/folder handler, never a command shell.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            action.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        Err("无法打开，请手动打开该页面或目录".into())
    } else {
        Ok(())
    }
}
fn windows_version() -> String {
    use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlGetVersion(version: *mut OSVERSIONINFOW) -> i32;
    }
    let mut info: OSVERSIONINFOW = unsafe { std::mem::zeroed() };
    info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as u32;
    if unsafe { RtlGetVersion(&mut info) } == 0 {
        format!(
            "{}.{}.{}",
            info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber
        )
    } else {
        String::new()
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let app = match args.next() {
        None => ambrum::registry::default_directory()?,
        Some(flag) if flag == "--app-directory" => {
            PathBuf::from(args.next().ok_or("Missing application directory")?)
        }
        _ => return Err("Usage: amberl.exe [--app-directory DIRECTORY]".into()),
    };
    if args.next().is_some() {
        return Err("Unexpected argument".into());
    }
    let ui = Launcher::new()?;
    {
        let weak = ui.as_weak();
        ui.on_minimize(move || {
            if let Some(ui) = weak.upgrade() {
                ui.window().set_minimized(true);
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_maximize(move || {
            if let Some(ui) = weak.upgrade() {
                ui.window().set_maximized(!ui.window().is_maximized());
            }
        });
    }
    let glass = RefCell::new(glass::Glass::new()?);
    ui.global::<GlassAssets>()
        .on_tile(move |x, y, w, h, vw, vh| glass.borrow_mut().tile([x, y, w, h, vw, vh]));
    ui.global::<GlassAssets>().set_ready(true);
    let model = Rc::new(RefCell::new(Model::new(app)));
    content_bindings(&ui, &model);
    let view = Rc::new(RefCell::new(View::default()));
    let os = windows_version();
    network_callbacks(&ui, &model);
    {
        let model = model.clone();
        let view = view.clone();
        let weak = ui.as_weak();
        ui.on_save_config(move || {
            if let Some(ui) = weak.upgrade() {
                operation(&model, |m| save_choice(&ui, m, &view.borrow()));
            }
        });
    }
    {
        let model = model.clone();
        let view = view.clone();
        let weak = ui.as_weak();
        let os = os.clone();
        ui.on_launch_join(move || {
            if let Some(ui) = weak.upgrade() {
                operation(&model, |m| {
                    save_choice(&ui, m, &view.borrow())?;
                    let mut choice = LaunchChoice::capture(
                        m.selected_game().ok_or("请先返回游戏页选择游戏")?,
                        &ui.get_java_path(),
                        view.borrow().account.as_deref().unwrap_or(""),
                        ui.get_memory_mib(),
                        &os,
                    )?;
                    choice.target =
                        Some(ambrum::multiplayer::Address::parse(&ui.get_join_target())?);
                    m.launch(choice)
                });
            }
        });
    }
    {
        let model = model.clone();
        ui.on_open_eula(move || operation(&model, |_| open("https://aka.ms/MinecraftEULA")));
    }
    {
        let model = model.clone();
        ui.on_load_versions(move || operation(&model, Model::catalog));
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_filter_versions(move || {
            if let Some(ui) = weak.upgrade() {
                filter_versions(&ui, &model.borrow());
            }
        });
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_pick_version(move |id| {
            if let Some(ui) = weak.upgrade() {
                if model.borrow().busy()
                    || !model.borrow().catalog.iter().any(|v| v.id == id.as_str())
                {
                    return;
                }
                ui.set_install_version(id.clone());
                ui.set_install_name(format!("{id}-vanilla").into());
                ui.set_install_title(format!("{id}-vanilla").into());
                ui.set_loader_kind("原版".into());
                ui.set_loader_options(strings(vec![]));
                ui.set_loader_index(-1);
            }
        });
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_pick_loader(move |kind| {
            if let Some(ui) = weak.upgrade() {
                if model.borrow().busy() {
                    return;
                }
                ui.set_loader_kind(kind.clone());
                ui.set_loader_options(strings(vec![]));
                ui.set_loader_index(-1);
                let name = format!(
                    "{}-{}",
                    ui.get_install_version(),
                    if kind == "原版" {
                        "vanilla".into()
                    } else {
                        kind.to_lowercase()
                    }
                );
                ui.set_install_name(name.clone().into());
                ui.set_install_title(name.into());
                if kind != "原版" {
                    operation(&model, |m| {
                        m.loaders(ui.get_install_version().to_string(), kind.to_string())
                    });
                }
            }
        });
    }
    {
        let model = model.clone();
        ui.on_refresh(move || operation(&model, Model::reload));
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        let view = view.clone();
        ui.on_select_game(move |id| {
            if let Some(ui) = weak.upgrade() {
                operation(&model, |m| {
                    if m.selected_game().is_some() {
                        save_choice(&ui, m, &view.borrow())?;
                    }
                    m.select(&id)
                })
            }
        });
    }
    {
        let model = model.clone();
        ui.on_login(move || operation(&model, Model::login));
    }
    {
        let model = model.clone();
        ui.on_discover_java(move || operation(&model, Model::discover_java));
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_install_java(move || {
            if let Some(ui) = weak.upgrade() {
                operation(&model, |m| {
                    let major = *[8, 16, 17, 21, 25]
                        .get(
                            usize::try_from(ui.get_java_download_index())
                                .map_err(|_| "请选择 Java 版本")?,
                        )
                        .ok_or("请选择 Java 版本")?;
                    m.install_java(major)
                });
            }
        });
    }
    {
        let model = model.clone();
        ui.on_cancel(move || model.borrow_mut().cancel());
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_add_library(move || {
            if let Some(ui) = weak.upgrade() {
                operation(&model, |m| {
                    m.attach(PathBuf::from(ui.get_library_path().as_str()))
                });
            }
        });
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        let os = os.clone();
        ui.on_install_game(move || {
            if let Some(ui) = weak.upgrade() {
                operation(&model, |m| {
                    m.install(
                        ui.get_install_version().to_string(),
                        PathBuf::from(ui.get_install_parent().as_str()),
                        ui.get_install_name().to_string(),
                        ui.get_budget_gib(),
                        os.clone(),
                        ui.get_install_title().to_string(),
                        ui.get_install_icon().to_string(),
                        if ui.get_loader_kind() == "原版" {
                            None
                        } else {
                            Some(ambrum::loader::Choice {
                                kind: ui.get_loader_kind().to_string(),
                                version: m
                                    .loaders
                                    .get(
                                        usize::try_from(ui.get_loader_index())
                                            .map_err(|_| "请选择加载器版本")?,
                                    )
                                    .ok_or("请选择加载器版本")?
                                    .clone(),
                            })
                        },
                        PathBuf::from(ui.get_java_path().as_str()),
                    )
                });
            }
        });
    }
    {
        let model = model.clone();
        let view = view.clone();
        ui.on_logout(move || {
            operation(&model, |m| {
                m.forget(view.borrow().account.clone().unwrap_or_default())
            });
        });
    }
    {
        let model = model.clone();
        let view = view.clone();
        let weak = ui.as_weak();
        ui.on_choose_account(move |index| {
            let m = model.borrow();
            if m.busy() {
                return;
            }
            let account = usize::try_from(index - 1)
                .ok()
                .and_then(|i| m.accounts.get(i));
            view.borrow_mut().account = account.map(|a| a.id.clone());
            if let Some(ui) = weak.upgrade() {
                ui.set_account_label(account.map_or("登录微软账户", |a| a.name.as_str()).into());
            }
        });
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_choose_java(move |index| {
            let m = model.borrow();
            if m.busy() {
                return;
            }
            let candidate = usize::try_from(index - 1)
                .ok()
                .and_then(|i| m.java.get(i))
                .filter(|c| c.java.is_some());
            if let (Some(ui), Some(candidate)) = (weak.upgrade(), candidate) {
                ui.set_java_path(candidate.executable.to_string_lossy().as_ref().into());
            }
        });
    }
    {
        let model = model.clone();
        let view = view.clone();
        let weak = ui.as_weak();
        ui.on_launch(move || {
            if let Some(ui) = weak.upgrade() {
                operation(&model, |m| {
                    save_choice(&ui, m, &view.borrow())?;
                    let game = m.selected_game().ok_or("请先选择游戏")?;
                    let choice = LaunchChoice::capture(
                        game,
                        &ui.get_java_path(),
                        view.borrow().account.as_deref().unwrap_or(""),
                        ui.get_memory_mib(),
                        &os,
                    )?;
                    m.launch(choice)
                });
            }
        });
    }
    {
        let model = model.clone();
        ui.on_open_microsoft(move || operation(&model, |_| open("https://www.microsoft.com/link")));
    }
    {
        let model = model.clone();
        ui.on_open_folder(move || {
            operation(&model, |m| {
                let path = ambrum::paths::ordinary(&m.selected_game().ok_or("请先选择游戏")?.root)?;
                open(&path.to_string_lossy())
            })
        });
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.window().on_close_requested(move || {
            if model.borrow().busy() || model.borrow().network.active() {
                model.borrow_mut().cancel();
                model.borrow_mut().network.shutdown();
                if let Some(ui) = weak.upgrade() {
                    ui.set_panel(5);
                }
                slint::CloseRequestResponse::KeepWindowShown
            } else {
                slint::CloseRequestResponse::HideWindow
            }
        });
    }
    operation(&model, Model::reload);
    net(&model, |n| n.reload());
    refresh(&ui, &model.borrow(), &mut view.borrow_mut());
    let timer = slint::Timer::default();
    {
        let weak = ui.as_weak();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(75),
            move || {
                model.borrow_mut().tick();
                if let Some(ui) = weak.upgrade() {
                    refresh(&ui, &model.borrow(), &mut view.borrow_mut());
                }
            },
        );
    }
    ui.run()?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        let message = wide(&error.to_string());
        let title = wide("AmberL 无法打开");
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                title.as_ptr(),
                0x10,
            );
        }
    }
}
