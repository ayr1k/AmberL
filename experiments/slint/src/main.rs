// SPDX-License-Identifier: MPL-2.0
#![cfg_attr(windows, windows_subsystem = "windows")]
use amberl_ui_model::Model;
use slint::{ComponentHandle, ModelRc, StandardListViewItem, VecModel};
use std::{cell::RefCell, rc::Rc, time::Duration};

slint::slint! {
    import { VerticalBox, HorizontalBox, Button, LineEdit, TextEdit, StandardListView, ProgressIndicator, AboutSlint } from "std-widgets.slint";
    export component Comparison inherits Window {
        title: "AmberL · Slint 1.18.1 comparison";
        preferred-width: 1120px; preferred-height: 820px; min-width: 840px; min-height: 680px;
        default-font-size: 14px;
        in-out property <string> library-path;
        in property <[StandardListViewItem]> rows;
        in property <string> detail;
        in property <string> status;
        in property <string> log;
        in property <bool> scanning;
        in property <bool> task-running;
        in property <float> progress;
        in-out property <int> selection: -1;
        private property <bool> about: false;
        callback scan(); callback cancel-scan(); callback filter(string); callback select(int);
        callback start-task(); callback cancel-task();
        VerticalBox {
            padding: 20px; spacing: 10px;
            HorizontalBox {
                Text { text: "AmberL / 游戏库 Library"; font-size: 24px; font-weight: 600; horizontal-stretch: 1; }
                Button { text: "关于 / About"; horizontal-stretch: 0; clicked => { root.about = !root.about; } }
            }
            Text { text: "Slint 比较样机 · 只读核心扫描 · 非最终界面"; }
            Text { text: "测试库路径 / Test library path"; }
            HorizontalBox {
                LineEdit { text <=> root.library-path; accessible-label: "测试库路径 Test library path"; }
                Button { text: "扫描 / Scan"; enabled: !root.scanning; clicked => { root.scan(); } }
                Button { text: "取消 / Cancel"; enabled: root.scanning; clicked => { root.cancel-scan(); } }
            }
            LineEdit { placeholder-text: "筛选名称、版本、加载器 / Filter"; accessible-label: "筛选 Filter"; edited(value) => { root.filter(value); } }
            HorizontalBox {
                StandardListView { model: root.rows; current-item <=> root.selection; accessible-label: "游戏库条目 Library entries"; horizontal-stretch: 3; current-item-changed(index) => { root.select(index); } }
                TextEdit { text: root.detail; read-only: true; accessible-label: "条目详情 Instance details"; horizontal-stretch: 2; }
            }
            Text { text: root.status; wrap: word-wrap; }
            Text { text: "中文输入与编辑 / IME, selection, undo and keyboard test"; }
            TextEdit { height: 82px; placeholder-text: "在这里测试中文预编辑、选择、撤销和键盘导航。"; accessible-label: "中文输入测试 Chinese input test"; }
            HorizontalBox {
                Button { text: "模拟任务 / Test task"; enabled: !root.task-running; clicked => { root.start-task(); } }
                Button { text: "取消任务 / Cancel task"; enabled: root.task-running; clicked => { root.cancel-task(); } }
                ProgressIndicator { progress: root.progress; horizontal-stretch: 1; }
            }
            TextEdit { height: 125px; text: root.log; read-only: true; accessible-label: "有界模拟日志 Bounded synthetic log"; }
            if root.about: HorizontalBox {
                Text { text: "Ambrum 核心 · 软件渲染 · 单进程技术比较\nMPL-2.0 source / Slint Royalty-free 2.0"; wrap: word-wrap; }
                AboutSlint { }
            }
        }
    }
}

#[derive(Default)]
struct View {
    revision: u64,
    rows_revision: u64,
    ids: Vec<String>,
}
fn refresh(ui: &Comparison, model: &Rc<RefCell<Model>>, view: &Rc<RefCell<View>>) {
    let snapshot = model.borrow().snapshot(false);
    if snapshot.revision == view.borrow().revision {
        return;
    }
    if snapshot.rows_revision != view.borrow().rows_revision {
        let rows = model.borrow().snapshot(true).rows;
        let items = rows
            .iter()
            .map(|r| StandardListViewItem::from(r.text.as_str()))
            .collect::<Vec<_>>();
        {
            let mut view = view.borrow_mut();
            view.ids = rows.into_iter().map(|r| r.id).collect();
            view.rows_revision = snapshot.rows_revision;
        }
        ui.set_rows(ModelRc::new(VecModel::from(items)));
        let selection = snapshot
            .selected_id
            .as_ref()
            .and_then(|id| {
                view.borrow()
                    .ids
                    .iter()
                    .position(|candidate| candidate == id)
            })
            .map(|index| index as i32)
            .unwrap_or(-1);
        ui.set_selection(selection);
    }
    ui.set_detail(snapshot.detail.into());
    ui.set_status(snapshot.status.into());
    ui.set_log(snapshot.log.into());
    ui.set_scanning(snapshot.scanning);
    ui.set_task_running(snapshot.task_running);
    ui.set_progress(snapshot.task_progress);
    view.borrow_mut().revision = snapshot.revision;
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = Comparison::new()?;
    let model = Rc::new(RefCell::new(Model::default()));
    let view = Rc::new(RefCell::new(View::default()));
    let path = std::env::args().nth(1).unwrap_or_default();
    ui.set_library_path(path.into());
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_scan(move || {
            if let Some(ui) = weak.upgrade() {
                if let Err(e) = model
                    .borrow_mut()
                    .scan(ui.get_library_path().as_str().into())
                {
                    ui.set_status(e.into());
                }
            }
        });
    }
    {
        let model = model.clone();
        ui.on_cancel_scan(move || model.borrow_mut().cancel_scan());
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_filter(move |query| {
            if let Err(e) = model.borrow_mut().filter(&query) {
                if let Some(ui) = weak.upgrade() {
                    ui.set_status(e.into());
                }
            }
        });
    }
    {
        let model = model.clone();
        let view = view.clone();
        ui.on_select(move |index| {
            let id = usize::try_from(index)
                .ok()
                .and_then(|i| view.borrow().ids.get(i).cloned());
            if let Some(id) = id {
                let _ = model.borrow_mut().select(&id);
            }
        });
    }
    {
        let model = model.clone();
        ui.on_start_task(move || model.borrow_mut().start_task());
    }
    {
        let model = model.clone();
        ui.on_cancel_task(move || model.borrow_mut().cancel_task());
    }
    refresh(&ui, &model, &view);
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            model.borrow_mut().tick();
            if let Some(ui) = weak.upgrade() {
                refresh(&ui, &model, &view);
            }
        },
    );
    ui.run()?;
    Ok(())
}
