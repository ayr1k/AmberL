// SPDX-License-Identifier: MPL-2.0
#![cfg_attr(windows, windows_subsystem = "windows")]
use amberl_desktop_model::{LaunchChoice, Model};
use slint::Model as _;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};

#[path = "../appearance.rs"]
mod appearance;
#[path = "../clipboard.rs"]
mod clipboard;
#[path = "../diagnostics.rs"]
mod diagnostics;
#[path = "../glass.rs"]
mod glass;
#[path = "../i18n.rs"]
mod i18n;
#[path = "../thumbnails.rs"]
mod thumbnails;
#[path = "../window.rs"]
mod window;
slint::include_modules!();

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
    maintenance_revision: u64,
    content_projects: Vec<String>,
    content_versions: Vec<String>,
    content_groups: Vec<String>,
    content_plan: String,
    configuration_key: (String, String),
    configuration_revision: u64,
    frp_revision: u64,
    copied_device_code: String,
    memory_value: i32,
    thumbnails: thumbnails::Thumbnails,
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
    let english = ui.global::<Locale>().get_english();
    let tr = |value: &str| -> SharedString { i18n::message(english, value).into() };
    ui.set_login_remaining(model.login_remaining());
    ui.set_window_maximized(ui.window().is_maximized());
    if view.memory_value != ui.get_memory_mib() {
        let memory = memory_values(ui.get_memory_mib());
        ui.set_memory_options(strings(memory.iter().map(|n| format!("{n} MiB")).collect()));
        ui.set_memory_index(
            memory
                .iter()
                .position(|n| *n == ui.get_memory_mib())
                .unwrap_or(0) as i32,
        );
        view.memory_value = ui.get_memory_mib();
    }
    let icons_changed = view.thumbnails.poll();
    if view.revision == model.revision
        && view.network_revision == model.network.revision
        && !icons_changed
    {
        return;
    }
    if view.maintenance_revision != model.maintenance.revision {
        let m = &model.maintenance;
        ui.set_maintenance_has_target(m.target.is_some());
        ui.set_maintenance_target(
            m.target
                .as_ref()
                .map_or("".into(), |g| format!("{} · {}", g.title, g.version).into()),
        );
        ui.set_maintenance_name(
            m.target
                .as_ref()
                .map_or("".into(), |g| g.title.as_str().into()),
        );
        ui.set_maintenance_icon(
            m.target
                .as_ref()
                .and_then(|g| {
                    ["grass", "dirt", "anvil", "pickaxe"]
                        .iter()
                        .position(|i| *i == g.icon)
                })
                .unwrap_or(0) as i32,
        );
        ui.set_client_backups(strings(
            m.backups
                .iter()
                .map(|b| format!("{} · {} · {}", b.title, b.version, &b.id[..8]))
                .collect(),
        ));
        ui.set_archived_games(strings(
            m.archived
                .iter()
                .map(|p| format!("{} · {}", p.title, p.version))
                .collect(),
        ));
        ui.set_client_backup_index(-1);
        ui.set_archived_game_index(-1);
        view.maintenance_revision = m.revision;
    }
    if view.content_revision != model.content.revision || icons_changed {
        let c = &model.content;
        ui.set_content_server(c.context.as_ref().is_some_and(|c| c.server));
        ui.set_content_match_game(c.browse.game);
        ui.set_content_match_loader(c.browse.loader);
        ui.set_content_kind_index(
            ["mod", "resourcepack", "shader", "plugin"]
                .iter()
                .position(|k| *k == c.kind)
                .unwrap_or(0) as i32,
        );
        let projects = c.projects.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
        let versions = c.versions.iter().map(|v| v.id.clone()).collect::<Vec<_>>();
        let groups = c.groups.iter().map(|g| g.id.clone()).collect::<Vec<_>>();
        if projects != view.content_projects {
            ui.set_content_project_index(-1);
            view.content_projects = projects;
        }
        if versions != view.content_versions {
            ui.set_content_version_index(
                c.versions
                    .iter()
                    .position(|v| v.recommended)
                    .or_else(|| c.versions.iter().position(|v| v.compatible))
                    .map_or(-1, |i| i as i32),
            );
            ui.set_content_expanded_group(
                c.context
                    .as_ref()
                    .map_or("", |ctx| ctx.game.as_str())
                    .into(),
            );
            view.content_versions = versions;
        }
        if groups != view.content_groups {
            if c.plan.is_some() && groups.len() > view.content_groups.len() {
                ui.set_content_step(0);
                ui.set_content_tab(1);
            }
            ui.set_content_group_index(-1);
            view.content_groups = groups;
        }
        if c.planned_version != view.content_plan {
            if c.plan.is_some() {
                ui.set_content_step(1);
            }
            view.content_plan = c.planned_version.clone();
        }
        ui.set_content_target(i18n::content_target(english, &c.title).into());
        ui.set_content_destination(match c.target.as_ref() {
            Some(ambrum::content::Target::Client { root, .. }) => root
                .join(match c.kind.as_str() {
                    "resourcepack" => "resourcepacks",
                    "shader" => "shaderpacks",
                    _ => "mods",
                })
                .display()
                .to_string()
                .into(),
            _ => "".into(),
        });
        ui.set_content_group_states(strings(c.groups.iter().map(|g| g.state.clone()).collect()));
        ui.set_content_offset(c.offset as i32);
        ui.set_content_total(c.total as i32);
        ui.set_content_loaded(c.loaded);
        view.thumbnails.request(
            c.projects
                .iter()
                .filter_map(|p| p.icon_url.clone())
                .collect(),
        );
        ui.set_content_projects(ModelRc::new(VecModel::from(
            c.projects
                .iter()
                .enumerate()
                .map(|(i, p)| ContentRow {
                    title: p.title.as_str().into(),
                    icon: view.thumbnails.image(p.icon_url.as_deref()),
                    has_icon: view.thumbnails.has(p.icon_url.as_deref()),
                    downloaded: c.downloaded.iter().any(|(id, _)| id == &p.id),
                    initial: p.title.chars().next().unwrap_or('•').to_string().into(),
                    description: p.description.as_str().into(),
                    meta: format!(
                        "{} · {} {}",
                        p.author,
                        p.downloads,
                        i18n::text(english, "次下载")
                    )
                    .into(),
                    index: i as i32,
                })
                .collect::<Vec<_>>(),
        )));
        ui.set_content_versions(strings(
            c.versions
                .iter()
                .map(|v| i18n::subtitle(english, &v.title))
                .collect(),
        ));
        ui.set_content_version_rows(ModelRc::new(VecModel::from(
            c.versions
                .iter()
                .enumerate()
                .map(|(index, v)| ContentVersionRow {
                    index: index as i32,
                    title: v.title.as_str().into(),
                    details: i18n::subtitle(english, &v.details).into(),
                    game: v.game.as_str().into(),
                    recommended: v.recommended,
                    compatible: v.compatible,
                    downloaded: c.downloaded.iter().any(|(_, version)| version == &v.id),
                })
                .collect::<Vec<_>>(),
        )));
        let make_version =
            |index: usize, v: &ambrum::content::modrinth::Version| ContentVersionRow {
                index: index as i32,
                title: v.title.as_str().into(),
                details: i18n::subtitle(english, &v.details).into(),
                game: v.game.as_str().into(),
                recommended: v.recommended,
                compatible: v.compatible,
                downloaded: c.downloaded.iter().any(|(_, id)| id == &v.id),
            };
        ui.set_content_recommendations(ModelRc::new(VecModel::from(
            c.versions
                .iter()
                .enumerate()
                .filter(|(_, v)| v.recommended)
                .map(|(i, v)| make_version(i, v))
                .collect::<Vec<_>>(),
        )));
        let mut version_groups = vec![];
        for v in &c.versions {
            if !version_groups.contains(&v.game) {
                version_groups.push(v.game.clone());
            }
        }
        ui.set_content_version_groups(ModelRc::new(VecModel::from(
            version_groups
                .into_iter()
                .map(|game| ContentVersionGroup {
                    title: game.as_str().into(),
                    rows: ModelRc::new(VecModel::from(
                        c.versions
                            .iter()
                            .enumerate()
                            .filter(|(_, v)| v.game == game && !v.recommended)
                            .map(|(i, v)| make_version(i, v))
                            .collect::<Vec<_>>(),
                    )),
                })
                .collect::<Vec<_>>(),
        )));
        ui.set_content_recommended_index(
            c.versions
                .iter()
                .position(|v| v.recommended)
                .map_or(-1, |i| i as i32),
        );
        filter_installed(ui, model);
        ui.set_content_preview(
            c.preview()
                .lines()
                .map(|line| i18n::text(english, line))
                .collect::<Vec<_>>()
                .join("\n")
                .into(),
        );
        ui.set_content_optional(ModelRc::new(VecModel::from(c.plan.as_ref().map_or_else(
            Vec::new,
            |p| {
                p.optional
                    .iter()
                    .map(|o| OptionalRow {
                        title: o.title.as_str().into(),
                        icon: view.thumbnails.image(
                            c.projects
                                .iter()
                                .find(|p| p.id == o.project)
                                .and_then(|p| p.icon_url.as_deref()),
                        ),
                        has_icon: view.thumbnails.has(
                            c.projects
                                .iter()
                                .find(|p| p.id == o.project)
                                .and_then(|p| p.icon_url.as_deref()),
                        ),
                        selected: o.selected,
                        downloaded: c.downloaded.iter().any(|(p, v)| {
                            p == &o.project && (o.version.is_empty() || v == &o.version)
                        }),
                    })
                    .collect()
            },
        ))));
        ui.set_content_required(ModelRc::new(VecModel::from(c.plan.as_ref().map_or_else(
            Vec::new,
            |p| {
                p.required
                    .iter()
                    .map(|o| OptionalRow {
                        title: o.title.as_str().into(),
                        icon: view.thumbnails.image(
                            c.projects
                                .iter()
                                .find(|p| p.id == o.project)
                                .and_then(|p| p.icon_url.as_deref()),
                        ),
                        has_icon: view.thumbnails.has(
                            c.projects
                                .iter()
                                .find(|p| p.id == o.project)
                                .and_then(|p| p.icon_url.as_deref()),
                        ),
                        selected: o.selected,
                        downloaded: c.downloaded.iter().any(|(id, v)| {
                            id == &o.project && (o.version.is_empty() || v == &o.version)
                        }),
                    })
                    .collect()
            },
        ))));
        ui.set_content_skipped_required(c.plan.as_ref().map_or(0, |p| {
            p.required.iter().filter(|r| !r.selected).count() as i32
        }));
        ui.set_content_plan_files(ModelRc::new(VecModel::from(c.plan.as_ref().map_or_else(
            Vec::new,
            |p| {
                p.files
                    .iter()
                    .take(1)
                    .map(|f| PlanRow {
                        title: f.title.as_str().into(),
                        path: f.path.as_str().into(),
                        downloaded: c
                            .downloaded
                            .iter()
                            .any(|(id, v)| id == &f.project && v == &f.version),
                    })
                    .collect()
            },
        ))));
        ui.set_content_plan_summary(
            c.plan
                .as_ref()
                .map_or_else(String::new, |p| {
                    format!(
                        "{} · {} {} · {:.1} MiB",
                        i18n::text(english, "所选版本"),
                        p.files.len(),
                        i18n::text(english, "个文件"),
                        p.files.iter().map(|f| f.artifact.size).sum::<u64>() as f64 / 1048576.0
                    )
                })
                .into(),
        );
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
        ui.set_frp_active(n.frp_active());
        ui.set_frp_ready(n.frp_component);
        ui.set_frp_status(tr(&n.frp_status));
        if n.frp_revision != view.frp_revision {
            if let Some(c) = &n.frp_config {
                ui.set_frp_server(c.server.text().into());
                ui.set_frp_certificate(c.certificate.to_string_lossy().as_ref().into());
                ui.set_frp_local_port(c.local_port.into());
                ui.set_frp_remote_port(c.remote_port.into());
            }
            view.frp_revision = n.frp_revision;
        }
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
                        s.loader
                            .as_ref()
                            .map_or(i18n::text(english, "原版"), |l| l.choice.kind.as_str()),
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
        ui.set_network_status(tr(&n.status));
        ui.set_server_status(tr(&n.server_status));
        ui.set_room_status(tr(&n.room_status));
        ui.set_query_status(tr(&n.query_status));
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
                    subtitle: i18n::subtitle(english, &g.subtitle).into(),
                    icon: g.icon.as_str().into(),
                })
                .collect::<Vec<_>>(),
        )));
        let index = view
            .account
            .as_ref()
            .and_then(|id| model.accounts.iter().position(|a| &a.id == id))
            .or_else(|| {
                model
                    .preferred_account
                    .as_ref()
                    .and_then(|id| model.accounts.iter().position(|a| &a.id == id))
            })
            .or_else(|| (!model.accounts.is_empty()).then_some(0));
        view.account = index.map(|i| model.accounts[i].id.clone());
        let location = model
            .locations
            .iter()
            .position(|l| l.destination() == PathBuf::from(ui.get_install_parent().as_str()))
            .or_else(|| (!model.locations.is_empty()).then_some(0));
        ui.set_install_locations(strings(
            model
                .locations
                .iter()
                .map(|l| l.destination().display().to_string())
                .collect(),
        ));
        ui.set_install_location_index(location.map_or(-1, |i| i as i32));
        ui.set_install_parent(
            location
                .map_or_else(String::new, |i| {
                    model.locations[i].destination().display().to_string()
                })
                .into(),
        );
        let mut accounts = vec![i18n::text(english, "选择账户").into()];
        accounts.extend(model.accounts.iter().map(|a| a.name.clone()));
        ui.set_account_options(strings(accounts));
        ui.set_account_index(index.map_or(0, |i| i as i32 + 1));
        ui.set_account_label(
            index
                .map_or(i18n::text(english, "登录微软账户"), |i| {
                    model.accounts[i].name.as_str()
                })
                .into(),
        );
        view.data_revision = model.data_revision;
    }
    if view.java_revision != model.java_revision {
        let mut java = vec![i18n::text(english, "选择检测到的 Java").into()];
        java.extend(model.java.iter().map(|c| {
            format!(
                "{} · {}",
                c.java.as_ref().map_or_else(
                    || i18n::text(english, "无法使用").into(),
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
            .map_or(i18n::text(english, "你的下一段冒险"), |g| {
                g.title.as_str()
            })
            .into(),
    );
    ui.set_game_subtitle(
        i18n::subtitle(
            english,
            selected.map_or("安装游戏，或添加已有游戏库", |g| {
                g.subtitle.as_str()
            }),
        )
        .into(),
    );
    ui.set_launch_candidate(selected.is_some_and(|g| g.installation_hint));
    ui.set_import_candidate(selected.is_some_and(|g| g.copy_candidate));
    ui.set_import_modded(
        selected
            .and_then(|g| g.copy_source.as_ref())
            .and_then(|s| s.loader())
            .is_some(),
    );
    ui.set_import_shared(
        selected.is_some_and(|g| g.isolation == ambrum::library::Isolation::Shared),
    );
    ui.set_library_details(selected.map_or("", |g| g.details.as_str()).into());
    ui.set_busy(model.busy());
    ui.set_running(model.running);
    ui.set_status(tr(&model.status));
    ui.set_status_help(
        if model.busy() {
            ""
        } else {
            diagnostics::help(english, &model.status)
        }
        .into(),
    );
    ui.set_network_help(diagnostics::help(english, &model.network.status).into());
    ui.set_update_available(model.update.is_some());
    ui.set_update_info(match &model.update {
        Some(release) => format!(
            "AmberL {} · {:.1} MiB\nSHA-256: {}",
            release.version,
            release.bytes as f64 / 1048576.0,
            release.sha256
        )
        .into(),
        None if model.update_checked => i18n::text(english, "暂无可用的新正式版本").into(),
        None => i18n::text(
            english,
            "仅手动检查 AmberL 官方正式版本；不会自动下载或安装。",
        )
        .into(),
    });
    ui.set_task_log(model.log().into());
    ui.set_task_title(tr(&model.task_title));
    ui.set_task_state(model.task_state);
    ui.set_login_active(model.login_active());
    ui.set_login_status(tr(&model.login_status));
    ui.set_device_code(model.device_code.as_str().into());
    if model.device_code != view.copied_device_code {
        if !model.device_code.is_empty() {
            copy_device_code(ui, &model.device_code);
        }
        view.copied_device_code = model.device_code.clone();
    }
    ui.set_device_prompt(i18n::device_prompt(english, &model.device_prompt).into());
    view.revision = model.revision;
}
fn copy_device_code(ui: &Launcher, code: &str) {
    let notice = if clipboard::copy(ui.window(), code) {
        "设备码已复制"
    } else {
        "复制失败，请点击复制设备码重试"
    };
    ui.set_clipboard_notice(i18n::text(ui.global::<Locale>().get_english(), notice).into());
}
fn save_choice(ui: &Launcher, model: &mut Model, view: &View) -> ambrum::Result<()> {
    model.save_preferences(ambrum::preferences::Launch {
        java: ui.get_java_path().as_str().into(),
        account: view.account.clone(),
        memory_mib: u32::try_from(ui.get_memory_mib()).map_err(|_| "无效内存")?,
    })
}
fn apply_accent(ui: &Launcher, value: &str) {
    if let Some(color) = appearance::color(value) {
        let rgb = u32::from_str_radix(&value[1..], 16).unwrap_or(0x62d6b4);
        let luminance = ((rgb >> 16) & 255) * 299 + ((rgb >> 8) & 255) * 587 + (rgb & 255) * 114;
        ui.global::<Theme>().set_brand(color);
        ui.global::<Theme>().set_primary_ink(if luminance > 150000 {
            slint::Color::from_rgb_u8(16, 32, 28)
        } else {
            slint::Color::from_rgb_u8(255, 255, 255)
        });
        ui.set_accent_input(value.into());
    }
}
fn memory_values(current: i32) -> Vec<i32> {
    let mut values = vec![
        512, 1024, 2048, 3072, 4096, 6144, 8192, 12288, 16384, 24576, 32768, 49152, 65536,
    ];
    if (512..=65536).contains(&current) && !values.contains(&current) {
        values.push(current);
        values.sort_unstable();
    }
    values
}
fn filter_installed(ui: &Launcher, model: &Model) {
    let english = ui.global::<Locale>().get_english();
    let query = ui.get_installed_query().to_lowercase();
    let filter = ui.get_installed_filter();
    let groups = model
        .content
        .groups
        .iter()
        .enumerate()
        .filter(|(_, g)| {
            (query.is_empty()
                || g.title.to_lowercase().contains(&query)
                || g.details().to_lowercase().contains(&query))
                && match filter {
                    1 => g.state == "installed",
                    2 => g.state == "removed",
                    3 => g.state != "installed" && g.state != "removed",
                    _ => true,
                }
        })
        .collect::<Vec<_>>();
    ui.set_installed_total(groups.len() as i32);
    let page = ui
        .get_installed_page()
        .max(0)
        .min((groups.len().saturating_sub(1) / 20) as i32);
    ui.set_installed_page(page);
    let rows = groups
        .into_iter()
        .skip(page as usize * 20)
        .take(20)
        .map(|(index, g)| {
            let state = i18n::text(
                english,
                match g.state.as_str() {
                    "installed" => "已安装",
                    "removed" => "已移除，可恢复",
                    "pending" => "安装未完成",
                    "removing" => "移除未完成",
                    _ => "恢复未完成",
                },
            );
            ContentRow {
                title: g.title.as_str().into(),
                icon: slint::Image::default(),
                has_icon: false,
                downloaded: g.state == "installed",
                initial: g.title.chars().next().unwrap_or('•').to_string().into(),
                description: g.details().lines().next().unwrap_or("").into(),
                meta: format!(
                    "{state} · {} {}",
                    g.file_count(),
                    i18n::text(english, "个文件")
                )
                .into(),
                index: index as i32,
            }
        })
        .collect::<Vec<_>>();
    if !rows.iter().any(|r| r.index == ui.get_content_group_index()) {
        ui.set_content_group_index(-1);
        ui.set_installed_details("".into());
    }
    ui.set_content_groups(ModelRc::new(VecModel::from(rows)));
    if let Some(g) = usize::try_from(ui.get_content_group_index())
        .ok()
        .and_then(|i| model.content.groups.get(i))
    {
        ui.set_installed_details(g.details().into());
    }
}
fn content_bindings(ui: &Launcher, model: &Rc<RefCell<Model>>) {
    {
        let model = model.clone();
        ui.on_select_required(move |index, selected| {
            operation(&model, |m| m.select_required_content(index, selected))
        });
    }
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
                        u.set_content_query(model.borrow().content.query.as_str().into());
                        u.set_content_sort(
                            ["relevance", "downloads", "updated", "newest"]
                                .iter()
                                .position(|s| *s == model.borrow().content.sort)
                                .unwrap_or(0) as i32,
                        );
                        u.set_installed_query("".into());
                        u.set_installed_filter(0);
                        u.set_installed_page(0);
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
        let kind = *["mod", "resourcepack", "shader", "plugin"]
            .get(usize::try_from(u.get_content_kind_index()).map_err(|_| "请选择内容类型")?)
            .ok_or("请选择内容类型")?;
        m.search_content_page(
            kind.into(),
            u.get_content_query().to_string(),
            ambrum::content::modrinth::Browse {
                game: u.get_content_match_game(),
                loader: u.get_content_match_loader(),
            },
            0,
            ["relevance", "downloads", "updated", "newest"]
                .get(u.get_content_sort() as usize)
                .copied()
                .unwrap_or("relevance")
                .into(),
            true,
        )
    });
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_content_page(move |direction| {
            if let Some(_u) = weak.upgrade() {
                operation(&model, |m| {
                    let c = &m.content;
                    let offset = if direction < 0 {
                        c.offset.saturating_sub(20)
                    } else {
                        c.offset + 20
                    };
                    if offset >= c.total && offset != 0 {
                        return Ok(());
                    }
                    m.search_content_page(
                        c.kind.clone(),
                        c.query.clone(),
                        c.browse,
                        offset,
                        c.sort.clone(),
                        false,
                    )
                });
            }
        });
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_filter_installed(move || {
            if let Some(u) = weak.upgrade() {
                filter_installed(&u, &model.borrow());
            }
        });
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_select_installed(move |index| {
            if let Some(u) = weak.upgrade() {
                if let Some(g) = usize::try_from(index)
                    .ok()
                    .and_then(|i| model.borrow().content.groups.get(i).cloned())
                {
                    u.set_installed_details(g.details().into());
                }
            }
        });
    }
    action!(on_refresh_content, |u: &Launcher, m: &mut Model| {
        m.refresh_content()?;
        u.set_content_query("".into());
        u.set_content_sort(0);
        u.set_content_step(0);
        Ok(())
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
fn maintenance_bindings(ui: &Launcher, model: &Rc<RefCell<Model>>) {
    let weak = ui.as_weak();
    let m = model.clone();
    ui.on_open_maintenance(move || {
        if let Some(u) = weak.upgrade() {
            let result = m.borrow_mut().open_maintenance();
            match result {
                Ok(()) => {
                    u.set_maintenance_tab(0);
                    u.set_panel(9);
                }
                Err(e) => m.borrow_mut().report(e),
            }
        }
    });
    macro_rules! action {
        ($callback:ident,$f:expr) => {{
            let weak = ui.as_weak();
            let model = model.clone();
            ui.$callback(move || {
                if let Some(u) = weak.upgrade() {
                    operation(&model, |m| $f(&u, m));
                }
            });
        }};
    }
    let edit = |u: &Launcher, m: &mut Model, archived: bool| {
        let icon = ["grass", "dirt", "anvil", "pickaxe"]
            .get(usize::try_from(u.get_maintenance_icon()).map_err(|_| "请选择图标")?)
            .ok_or("请选择图标")?;
        m.edit_instance(
            u.get_maintenance_name().to_string(),
            (*icon).into(),
            archived,
        )
    };
    action!(on_save_instance, |u: &Launcher, m: &mut Model| edit(
        u, m, false
    ));
    action!(on_archive_instance, |u: &Launcher, m: &mut Model| edit(
        u, m, true
    ));
    action!(on_backup_instance, |_: &Launcher, m: &mut Model| m
        .backup_instance());
    action!(on_unarchive_instance, |u: &Launcher, m: &mut Model| m
        .unarchive_instance(u.get_archived_game_index()));
    action!(on_restore_instance, |u: &Launcher, m: &mut Model| m
        .restore_instance(
            u.get_client_backup_index(),
            u.get_client_restore_parent().as_str().into(),
            u.get_client_restore_name().to_string()
        ));
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
        on_load_frp,
        |_: &Launcher, n: &mut amberl_desktop_model::network::Network| n.load_frp()
    );
    action!(
        on_install_frp,
        |_: &Launcher, n: &mut amberl_desktop_model::network::Network| n.install_frp()
    );
    action!(
        on_stop_frp,
        |_: &Launcher, n: &mut amberl_desktop_model::network::Network| {
            n.stop_frp();
            Ok(())
        }
    );
    action!(
        on_start_frp,
        |u: &Launcher, n: &mut amberl_desktop_model::network::Network| {
            let token = u.get_frp_token().to_string();
            u.set_frp_token("".into());
            n.start_frp(
                ambrum::multiplayer::frp::Config {
                    server: ambrum::multiplayer::Address::parse(u.get_frp_server().as_str())?,
                    local_port: u16::try_from(u.get_frp_local_port())
                        .map_err(|_| "无效本地端口")?,
                    remote_port: u16::try_from(u.get_frp_remote_port())
                        .map_err(|_| "无效节点端口")?,
                    certificate: u.get_frp_certificate().as_str().into(),
                },
                token,
            )
        }
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
        use slint::winit_030::{WinitWindowAccessor, winit};
        let input_weak = ui.as_weak();
        ui.window().on_winit_window_event(move |_, event| {
            if let Some(ui) = input_weak.upgrade() {
                match event {
                    winit::event::WindowEvent::KeyboardInput { .. } => {
                        ui.global::<Theme>().set_keyboard_focus(true)
                    }
                    winit::event::WindowEvent::MouseInput { .. }
                    | winit::event::WindowEvent::Touch(_) => {
                        ui.global::<Theme>().set_keyboard_focus(false)
                    }
                    _ => {}
                }
            }
            slint::winit_030::EventResult::Propagate
        });
        let weak = ui.as_weak();
        let move_weak = ui.as_weak();
        ui.on_move_window(move || {
            if let Some(ui) = move_weak.upgrade() {
                ui.window().with_winit_window(|window| {
                    let _ = window.drag_window();
                });
            }
        });
        ui.on_resize_window(move || {
            if let Some(ui) = weak.upgrade() {
                ui.window().with_winit_window(|window| {
                    let _ = window.drag_resize_window(winit::window::ResizeDirection::SouthEast);
                });
            }
        });
    }
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
    // Respect Windows' animation preference without changing system settings.
    let mut animations: i32 = 1;
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
            windows_sys::Win32::UI::WindowsAndMessaging::SPI_GETCLIENTAREAANIMATION,
            0,
            (&mut animations as *mut i32).cast(),
            0,
        );
    }
    ui.global::<Theme>().set_motion(animations != 0);
    let glass = Rc::new(RefCell::new(glass::Glass::new()?));
    let backgrounds = Rc::new(RefCell::new(appearance::Loader::default()));
    {
        let glass = glass.clone();
        ui.global::<GlassAssets>()
            .on_tile(move |x, y, w, h, vw, vh, _revision| {
                glass.borrow_mut().tile([x, y, w, h, vw, vh])
            });
    }
    ui.global::<GlassAssets>().set_ready(true);
    let model = Rc::new(RefCell::new(Model::new(app)));
    match model.borrow().appearance() {
        Ok(saved) => {
            ui.invoke_apply_appearance(saved.light);
            apply_accent(&ui, &saved.accent);
            ui.set_background_name(
                saved
                    .background
                    .as_ref()
                    .map_or_else(
                        || i18n::text(saved.english, "默认森林背景").to_owned(),
                        |p| p.display().to_string(),
                    )
                    .into(),
            );
            if saved.background.is_some() {
                backgrounds.borrow_mut().start(saved.background)?;
                ui.set_appearance_busy(true);
            }
            slint::select_bundled_translation(if saved.english { "en_US" } else { "" })?;
            ui.global::<Locale>().set_english(saved.english);
        }
        Err(error) => eprintln!("Appearance preserved: {error}"),
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_appearance(move |light| {
            let result = model.borrow_mut().save_appearance(light);
            match result {
                Ok(()) => {
                    if let Some(ui) = weak.upgrade() {
                        ui.invoke_apply_appearance(light);
                    }
                }
                Err(error) => model.borrow_mut().report(error),
            }
        });
    }
    {
        let weak = ui.as_weak();
        let model = model.clone();
        ui.on_set_accent(move |value| {
            let result = model.borrow_mut().save_accent(value.to_string());
            match result {
                Ok(()) => {
                    if let Some(u) = weak.upgrade() {
                        apply_accent(&u, &value);
                    }
                }
                Err(e) => model.borrow_mut().report(e),
            }
        });
    }
    {
        let weak = ui.as_weak();
        let backgrounds = backgrounds.clone();
        let model = model.clone();
        ui.on_choose_background(move || {
            if let Some(u) = weak.upgrade() {
                if let Some(path) = appearance::pick(u.window()) {
                    match backgrounds.borrow_mut().start(Some(path)) {
                        Ok(()) => u.set_appearance_busy(true),
                        Err(e) => model.borrow_mut().report(e),
                    }
                }
            }
        });
    }
    {
        let weak = ui.as_weak();
        let backgrounds = backgrounds.clone();
        let model = model.clone();
        ui.on_reset_background(move || {
            if let Some(u) = weak.upgrade() {
                match backgrounds.borrow_mut().start(None) {
                    Ok(()) => u.set_appearance_busy(true),
                    Err(e) => model.borrow_mut().report(e),
                }
            }
        });
    }
    content_bindings(&ui, &model);
    maintenance_bindings(&ui, &model);
    let view = Rc::new(RefCell::new(View::default()));
    {
        let weak = ui.as_weak();
        let model = model.clone();
        let view = view.clone();
        ui.on_language(move |english| {
            let result = model.borrow_mut().save_language(english).and_then(|_| {
                slint::select_bundled_translation(if english { "en_US" } else { "" })
                    .map_err(|e| e.to_string())
            });
            match result {
                Ok(()) => {
                    if let Some(ui) = weak.upgrade() {
                        ui.global::<Locale>().set_english(english);
                        let mut view = view.borrow_mut();
                        view.revision = 0;
                        view.data_revision = 0;
                        view.java_revision = 0;
                        view.network_revision = 0;
                        view.content_revision = 0;
                        refresh(&ui, &model.borrow(), &mut view);
                    }
                }
                Err(error) => model.borrow_mut().report(error),
            }
        });
    }
    let os = windows_version();
    {
        let model = model.clone();
        let weak = ui.as_weak();
        let os = os.clone();
        ui.on_import_game(move || {
            if let Some(ui) = weak.upgrade() {
                operation(&model, |m| {
                    m.import_game(
                        PathBuf::from(ui.get_import_parent().as_str()),
                        ui.get_import_name().to_string(),
                        ui.get_import_acknowledged(),
                        ui.get_import_mods_acknowledged(),
                        PathBuf::from(ui.get_java_path().as_str()),
                        os.clone(),
                    )
                });
            }
        });
    }
    {
        let model = model.clone();
        ui.on_check_update(move || operation(&model, |m| m.check_update()));
    }
    {
        let model = model.clone();
        ui.on_open_release(move || {
            let page = model.borrow().update.as_ref().map(|r| r.page.clone());
            if let Some(page) = page {
                operation(&model, |_| open(&page));
            }
        });
    }
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
        ui.on_cancel_login(move || model.borrow_mut().cancel_login());
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_copy_device_code(move || {
            if let Some(ui) = weak.upgrade() {
                copy_device_code(&ui, &model.borrow().device_code);
            }
        });
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_choose_install_location(move |index| {
            if let (Some(ui), Some(location)) = (
                weak.upgrade(),
                usize::try_from(index)
                    .ok()
                    .and_then(|i| model.borrow().locations.get(i).cloned()),
            ) {
                ui.set_install_parent(location.destination().display().to_string().into());
            }
        });
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
        let weak = ui.as_weak();
        ui.on_choose_memory(move |index| {
            if let Some(u) = weak.upgrade() {
                if let Some(n) = usize::try_from(index)
                    .ok()
                    .and_then(|i| memory_values(u.get_memory_mib()).get(i).copied())
                {
                    u.set_memory_mib(n);
                }
            }
        });
    }
    {
        let model = model.clone();
        let view = view.clone();
        let weak = ui.as_weak();
        ui.on_choose_account(move |index| {
            let mut m = model.borrow_mut();
            if m.busy() {
                return;
            }
            let account = usize::try_from(index - 1)
                .ok()
                .and_then(|i| m.accounts.get(i))
                .cloned();
            let choice = account.as_ref().map(|a| a.id.clone());
            if let Err(error) = m.choose_account(choice.clone()) {
                m.report(error);
                return;
            }
            view.borrow_mut().account = choice;
            if let Some(ui) = weak.upgrade() {
                ui.set_account_label(
                    account
                        .as_ref()
                        .map_or(
                            i18n::text(ui.global::<Locale>().get_english(), "登录微软账户"),
                            |a| a.name.as_str(),
                        )
                        .into(),
                );
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
            if model.borrow().busy()
                || model.borrow().login_active()
                || model.borrow().network.active()
            {
                model.borrow_mut().cancel();
                model.borrow_mut().cancel_login();
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
    let launcher_root = std::env::current_exe()?
        .parent()
        .ok_or("Missing launcher folder")?
        .to_path_buf();
    operation(&model, |m| m.initialize(launcher_root));
    net(&model, |n| n.reload());
    refresh(&ui, &model.borrow(), &mut view.borrow_mut());
    let timer = slint::Timer::default();
    {
        let weak = ui.as_weak();
        let fitted = std::cell::Cell::new(false);
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(75),
            move || {
                model.borrow_mut().tick();
                if let Some(ui) = weak.upgrade() {
                    if let Some((path, result)) = backgrounds.borrow_mut().poll() {
                        ui.set_appearance_busy(false);
                        match result {
                            Ok(scene) => {
                                let saved = model.borrow_mut().save_background(path.clone());
                                match saved {
                                    Ok(()) => {
                                        ui.set_background_image(
                                            glass.borrow_mut().replace(scene.pixels, scene.blurred),
                                        );
                                        let assets = ui.global::<GlassAssets>();
                                        assets.set_revision(assets.get_revision() + 1);
                                        ui.set_background_name(
                                            path.map_or_else(
                                                || {
                                                    i18n::text(
                                                        ui.global::<Locale>().get_english(),
                                                        "默认森林背景",
                                                    )
                                                    .to_owned()
                                                },
                                                |p| p.display().to_string(),
                                            )
                                            .into(),
                                        );
                                    }
                                    Err(e) => model.borrow_mut().report(e),
                                }
                            }
                            Err(e) => model.borrow_mut().report(e),
                        }
                    }
                    // Winit creates its native window only after entering the event loop.
                    if !fitted.get() {
                        fitted.set(window::fit(ui.window()));
                    }
                    refresh(&ui, &model.borrow(), &mut view.borrow_mut());
                }
            },
        );
    }
    ui.show()?;
    slint::run_event_loop()?;
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
