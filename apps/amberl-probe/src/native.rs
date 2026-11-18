// SPDX-License-Identifier: MPL-2.0
//! A small Win32 system-control baseline, not a new GUI framework or final UI selection.
#![allow(unsafe_op_in_unsafe_fn)]
use ambrum::{
    library::{Instance, Isolation, Library, Scan},
    registry::{self, Entry, Registry},
    task::Cancellation,
};
use std::{
    cell::RefCell,
    path::PathBuf,
    ptr::{null, null_mut},
    sync::mpsc,
    time::Instant,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{Com::*, LibraryLoader::*},
    UI::{Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
};

const NAV_GAME: i32 = 101;
const NAV_ONLINE: i32 = 102;
const NAV_ABOUT: i32 = 103;
const TITLE: i32 = 110;
const SUBTITLE: i32 = 111;
const PATH: i32 = 120;
const BROWSE: i32 = 121;
const SCAN: i32 = 122;
const CANCEL: i32 = 123;
const LIBRARIES: i32 = 124;
const ATTACH: i32 = 125;
const DETACH: i32 = 126;
const FILTER: i32 = 130;
const LIST: i32 = 140;
const DETAIL: i32 = 150;
const STATUS: i32 = 160;
const TASK_TIMER: usize = 1;
const FILTER_TIMER: usize = 2;

struct App {
    rows: Vec<Instance>,
    visible: Vec<usize>,
    receiver: Option<mpsc::Receiver<(ambrum::Result<WorkResult>, u128)>>,
    entries: Vec<Entry>,
    preview: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    cancel: Cancellation,
    font: HFONT,
    heading: HFONT,
    page: i32,
}
enum Action {
    Preview(PathBuf),
    Saved(String),
    Attach(PathBuf),
    Detach(String),
}
struct WorkResult {
    scan: Option<Scan>,
    entries: Option<Vec<Entry>>,
    notice: String,
}
thread_local! { static APP: RefCell<Option<App>> = const { RefCell::new(None) }; }

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
unsafe fn control(hwnd: HWND, id: i32) -> HWND {
    GetDlgItem(hwnd, id)
}
unsafe fn set(hwnd: HWND, id: i32, text: &str) {
    SetWindowTextW(control(hwnd, id), wide(text).as_ptr());
}
unsafe fn get(hwnd: HWND, id: i32) -> String {
    let h = control(hwnd, id);
    let mut buffer = vec![0; (GetWindowTextLengthW(h) + 1).max(1) as usize];
    let n = GetWindowTextW(h, buffer.as_mut_ptr(), buffer.len() as i32);
    String::from_utf16_lossy(&buffer[..n as usize])
}
unsafe fn child(hwnd: HWND, class: &str, text: &str, id: i32, style: u32, ex: u32) {
    CreateWindowExW(
        ex,
        wide(class).as_ptr(),
        wide(text).as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        0,
        0,
        1,
        1,
        hwnd,
        id as usize as HMENU,
        GetModuleHandleW(null()),
        null(),
    );
}
fn isolation(value: Isolation) -> &'static str {
    match value {
        Isolation::Isolated => "独立目录",
        Isolation::Shared => "共享目录",
        Isolation::Unknown => "待确认",
        Isolation::Conflict => "配置冲突",
    }
}

unsafe fn fonts(hwnd: HWND) {
    let dpi = GetDpiForWindow(hwnd).max(96) as i32;
    let make = |size: i32, weight: i32| {
        CreateFontW(
            -size * dpi / 96,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Segoe UI").as_ptr(),
        )
    };
    let normal = make(15, 400);
    let heading = make(28, 600);
    let old = APP.with(|cell| {
        let mut a = cell.borrow_mut();
        a.as_mut().map(|a| {
            let old = (a.font, a.heading);
            a.font = normal;
            a.heading = heading;
            old
        })
    });
    for id in [
        NAV_GAME, NAV_ONLINE, NAV_ABOUT, TITLE, SUBTITLE, PATH, BROWSE, SCAN, CANCEL, FILTER, LIST,
        DETAIL, STATUS, LIBRARIES, ATTACH, DETACH,
    ] {
        SendMessageW(
            control(hwnd, id),
            WM_SETFONT,
            if id == TITLE { heading } else { normal } as usize,
            1,
        );
    }
    if let Some((a, b)) = old {
        if !a.is_null() {
            DeleteObject(a);
        }
        if !b.is_null() {
            DeleteObject(b);
        }
    }
}

unsafe fn layout(hwnd: HWND) {
    if APP.with(|a| a.borrow().is_none()) {
        return;
    }
    let mut r = RECT::default();
    GetClientRect(hwnd, &mut r);
    let scale = GetDpiForWindow(hwnd).max(96) as f64 / 96.;
    let w = (r.right as f64 / scale) as i32;
    let h = (r.bottom as f64 / scale) as i32;
    let place = |id, x, y, width: i32, height: i32| {
        MoveWindow(
            control(hwnd, id),
            (x as f64 * scale) as i32,
            (y as f64 * scale) as i32,
            (width.max(1) as f64 * scale) as i32,
            (height.max(1) as f64 * scale) as i32,
            1,
        );
    };
    place(NAV_GAME, 24, 20, 112, 34);
    place(NAV_ONLINE, 144, 20, 112, 34);
    place(NAV_ABOUT, w - 160, 20, 136, 34);
    place(TITLE, 28, 80, w - 56, 42);
    place(SUBTITLE, 28, 128, w - 56, 46);
    place(PATH, 28, 190, w - 360, 34);
    place(BROWSE, w - 320, 190, 96, 34);
    place(SCAN, w - 212, 190, 92, 34);
    place(CANCEL, w - 108, 190, 80, 34);
    place(LIBRARIES, 28, 240, w - 308, 200);
    place(ATTACH, w - 268, 240, 112, 34);
    place(DETACH, w - 144, 240, 116, 34);
    place(FILTER, 28, 290, w - 56, 34);
    let table_h = ((h - 390) * 56 / 100).max(100);
    place(LIST, 28, 340, w - 56, table_h);
    if APP.with(|cell| cell.borrow().as_ref().unwrap().page) == NAV_GAME {
        place(DETAIL, 28, 354 + table_h, w - 56, h - 410 - table_h);
    } else {
        place(DETAIL, 28, 190, w - 56, h - 248);
    }
    place(STATUS, 28, h - 40, w - 56, 30);
    for (i, factor) in [0.40, 0.20, 0.20, 0.20].iter().enumerate() {
        SendMessageW(
            control(hwnd, LIST),
            LVM_SETCOLUMNWIDTH,
            i,
            (((w - 62) as f64) * scale * factor) as isize,
        );
    }
}

unsafe fn filter(hwnd: HWND) {
    let query = get(hwnd, FILTER).to_lowercase();
    let count = APP.with(|cell| {
        let mut a = cell.borrow_mut();
        let a = a.as_mut().unwrap();
        a.visible = a
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                query.is_empty()
                    || format!("{} {} {}", r.name, r.game, r.loader)
                        .to_lowercase()
                        .contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
        a.visible.len()
    });
    SendMessageW(control(hwnd, LIST), LVM_SETITEMCOUNT, count, 0);
    InvalidateRect(control(hwnd, LIST), null(), 1);
    if APP.with(|cell| cell.borrow().as_ref().unwrap().page) == NAV_GAME {
        set(
            hwnd,
            DETAIL,
            "选择一个实例查看识别依据。此样机不会安装、启动或修改游戏文件。",
        );
    }
}

unsafe fn start(hwnd: HWND, action: Action) {
    let data_dir = APP.with(|cell| cell.borrow().as_ref().unwrap().data_dir.clone());
    let cancellable = matches!(action, Action::Preview(_) | Action::Saved(_));
    let token = Cancellation::default();
    let worker_token = token.clone();
    let (tx, rx) = mpsc::channel();
    let admitted = APP.with(|cell| {
        let mut a = cell.borrow_mut();
        let a = a.as_mut().unwrap();
        if a.receiver.is_some() {
            return false;
        }
        a.cancel = token;
        a.receiver = Some(rx);
        true
    });
    if !admitted {
        return;
    }
    // One worker at a time; changing UI selection never changes this captured path.
    std::thread::spawn(move || {
        let start = Instant::now();
        let result = (|| -> ambrum::Result<WorkResult> {
            let open = || {
                Registry::open(
                    data_dir
                        .as_deref()
                        .ok_or("库列表不可用，请查看启动提示 / Registry unavailable")?,
                )
            };
            match action {
                Action::Preview(path) => Ok(WorkResult {
                    scan: Some(Library::open(&path)?.scan(&worker_token)?),
                    entries: None,
                    notice: String::new(),
                }),
                Action::Saved(id) => Ok(WorkResult {
                    scan: Some(open()?.scan(&id, &worker_token)?),
                    entries: None,
                    notice: String::new(),
                }),
                Action::Attach(path) => {
                    let mut registry = open()?;
                    registry.attach(&path)?;
                    Ok(WorkResult {
                        scan: None,
                        entries: Some(registry.entries().to_vec()),
                        notice: "已保存游戏库。可从列表选择；原库未改动。".into(),
                    })
                }
                Action::Detach(id) => {
                    let mut registry = open()?;
                    registry.detach(&id)?;
                    Ok(WorkResult {
                        scan: None,
                        entries: Some(registry.entries().to_vec()),
                        notice: "已移除登记，游戏文件全部保留。".into(),
                    })
                }
            }
        })();
        let _ = tx.send((result, start.elapsed().as_millis()));
    });
    EnableWindow(control(hwnd, SCAN), 0);
    EnableWindow(control(hwnd, BROWSE), 0);
    EnableWindow(control(hwnd, PATH), 0);
    for id in [ATTACH, DETACH, LIBRARIES] {
        EnableWindow(control(hwnd, id), 0);
    }
    EnableWindow(control(hwnd, CANCEL), i32::from(cancellable));
    set(
        hwnd,
        STATUS,
        if cancellable {
            "正在只读扫描…可以取消，界面保持可操作。"
        } else {
            "正在更新 AmberL 的库列表，游戏文件保持不变。"
        },
    );
    SetTimer(hwnd, TASK_TIMER, 80, None);
}

unsafe fn poll(hwnd: HWND) {
    let message = APP.with(|cell| {
        let mut a = cell.borrow_mut();
        let a = a.as_mut().unwrap();
        let message = match a.receiver.as_ref()?.try_recv() {
            Ok(m) => Some(m),
            Err(mpsc::TryRecvError::Disconnected) => Some((
                Err("扫描线程已结束，未得到结果 / Worker disconnected".into()),
                0,
            )),
            Err(mpsc::TryRecvError::Empty) => None,
        };
        if message.is_some() {
            a.receiver = None;
        }
        message.map(|(result, time)| {
            (
                if a.cancel.check().is_err() {
                    Err("扫描已取消；未修改原库。".into())
                } else {
                    result
                },
                time,
            )
        })
    });
    let Some((result, time)) = message else {
        return;
    };
    KillTimer(hwnd, TASK_TIMER);
    EnableWindow(control(hwnd, SCAN), 1);
    EnableWindow(control(hwnd, BROWSE), 1);
    EnableWindow(control(hwnd, PATH), 1);
    EnableWindow(control(hwnd, CANCEL), 0);
    EnableWindow(control(hwnd, LIBRARIES), 1);
    EnableWindow(control(hwnd, DETACH), 1);
    match result {
        Ok(work) => {
            if let Some(entries) = work.entries {
                APP.with(|cell| {
                    let mut a = cell.borrow_mut();
                    let a = a.as_mut().unwrap();
                    a.entries = entries;
                    a.preview = None;
                    a.rows.clear();
                });
                refresh_libraries(hwnd);
                filter(hwnd);
            }
            let Some(scan) = work.scan else {
                set(hwnd, STATUS, &work.notice);
                return;
            };
            let unknown = scan
                .instances
                .iter()
                .filter(|r| matches!(r.isolation, Isolation::Unknown | Isolation::Conflict))
                .count();
            let count = scan.instances.len();
            // Display the captured target, never an unrelated text-box selection.
            set(hwnd, PATH, &scan.library.root.display().to_string());
            APP.with(|cell| {
                let mut a = cell.borrow_mut();
                let a = a.as_mut().unwrap();
                a.preview = if matches!(scan.library.id, ambrum::library::LibraryId::Observed(_)) {
                    Some(scan.library.root.clone())
                } else {
                    None
                };
                a.rows = scan.instances;
                EnableWindow(
                    control(hwnd, ATTACH),
                    i32::from(a.preview.is_some() && a.data_dir.is_some()),
                );
            });
            filter(hwnd);
            set(
                hwnd,
                STATUS,
                &format!(
                    "已识别 {count} 个实例 · {unknown} 个目录待确认 · 扫描 {time} ms · 原库未修改"
                ),
            );
        }
        Err(error) => {
            APP.with(|cell| {
                let mut a = cell.borrow_mut();
                let a = a.as_mut().unwrap();
                a.rows.clear();
                a.visible.clear();
                a.preview = None;
            });
            filter(hwnd);
            set(hwnd, STATUS, &error);
            set(
                hwnd,
                DETAIL,
                &format!("{error}\r\n\r\n未修改任何游戏文件。请检查目录及配置后重试。"),
            );
        }
    }
}

unsafe fn refresh_libraries(hwnd: HWND) {
    let combo = control(hwnd, LIBRARIES);
    SendMessageW(combo, CB_RESETCONTENT, 0, 0);
    SendMessageW(
        combo,
        CB_ADDSTRING,
        0,
        wide("已添加的游戏库（选择后扫描）").as_ptr() as isize,
    );
    let labels = APP.with(|cell| {
        cell.borrow()
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .map(|e| format!("{} — {}", e.label, e.path.display()))
            .collect::<Vec<_>>()
    });
    for label in labels {
        SendMessageW(combo, CB_ADDSTRING, 0, wide(&label).as_ptr() as isize);
    }
    SendMessageW(combo, CB_SETCURSEL, 0, 0);
}

unsafe fn selected_library(hwnd: HWND) -> Option<String> {
    let index = SendMessageW(control(hwnd, LIBRARIES), CB_GETCURSEL, 0, 0);
    if index <= 0 {
        return None;
    }
    APP.with(|cell| {
        cell.borrow()
            .as_ref()
            .unwrap()
            .entries
            .get(index as usize - 1)
            .map(|e| e.id.clone())
    })
}

unsafe fn browse(hwnd: HWND) {
    let title = wide("选择游戏库：AMBER 数据目录或 .minecraft（仅预览，不修改）");
    let mut display = [0u16; 260];
    let info = BROWSEINFOW {
        hwndOwner: hwnd,
        pszDisplayName: display.as_mut_ptr(),
        lpszTitle: title.as_ptr(),
        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_EDITBOX,
        ..Default::default()
    };
    let selected = SHBrowseForFolderW(&info);
    if !selected.is_null() {
        let mut path = [0u16; 32768];
        if SHGetPathFromIDListEx(selected, path.as_mut_ptr(), path.len() as u32, 0) != 0 {
            let len = path.iter().position(|c| *c == 0).unwrap_or(path.len());
            set(hwnd, PATH, &String::from_utf16_lossy(&path[..len]));
        }
        CoTaskMemFree(selected.cast());
    }
}

unsafe fn page(hwnd: HWND, value: i32) {
    APP.with(|cell| cell.borrow_mut().as_mut().unwrap().page = value);
    let game = value == NAV_GAME;
    for id in [
        PATH, BROWSE, SCAN, CANCEL, FILTER, LIST, LIBRARIES, ATTACH, DETACH,
    ] {
        ShowWindow(control(hwnd, id), if game { SW_SHOW } else { SW_HIDE });
    }
    match value {
        NAV_GAME => {
            set(hwnd, TITLE, "你的游戏");
            set(
                hwnd,
                SUBTITLE,
                "从已有游戏库开始，查看版本、加载器与隔离状态。\r\n当前为只读技术样机，尚未提供安装和启动。",
            );
            filter(hwnd);
            layout(hwnd);
        }
        NAV_ONLINE => {
            set(hwnd, TITLE, "一起玩");
            set(
                hwnd,
                SUBTITLE,
                "服务端和通道分别管理；只有验证过的状态才显示为成功。",
            );
            set(
                hwnd,
                DETAIL,
                "联机功能尚未实现。\r\n\r\n规划范围：创建服务端、控制台、正常停止、模组与插件、FRP、加入联机和连接分享。\r\n服务端运行、服务就绪、通道已连接和外部可达将分别呈现。",
            );
        }
        _ => {
            set(hwnd, TITLE, "关于这个样机");
            set(
                hwnd,
                SUBTITLE,
                "AmberL · Ambrum\r\nRust 核心 + Windows 系统控件；无 WebView、无 Node 后台。",
            );
            set(
                hwnd,
                DETAIL,
                &format!(
                    "0.1.0-dev.1 / 原生技术验证\r\n\r\n真实能力：只读库扫描、目录隔离证据、搜索、取消、键盘导航。\r\n尚未实现：登录、安装、启动、内容下载、备份恢复和联机。\r\n\r\n微软登录将沿用 AMBER 的公开应用标识：\r\n{}\r\n账号凭据和应用配置保持独立。\r\n\r\n此 Win32 控件样机仅作为输入、DPI 和资源基线。Slint / WinUI 尚未完成对比，不代表最终界面。\r\n项目源码采用 MPL-2.0。",
                    ambrum::MICROSOFT_CLIENT_ID
                ),
            );
        }
    }
    if !game {
        let mut r = RECT::default();
        GetClientRect(hwnd, &mut r);
        let d = GetDpiForWindow(hwnd).max(96) as i32;
        MoveWindow(
            control(hwnd, DETAIL),
            28 * d / 96,
            190 * d / 96,
            r.right - 56 * d / 96,
            r.bottom - 248 * d / 96,
            1,
        );
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            layout(hwnd);
            0
        }
        WM_GETMINMAXINFO => {
            let m = &mut *(lp as *mut MINMAXINFO);
            let dpi = GetDpiForWindow(hwnd).max(96) as i32;
            m.ptMinTrackSize = POINT {
                x: 720 * dpi / 96,
                y: 680 * dpi / 96,
            };
            0
        }
        WM_DPICHANGED => {
            let rect = &*(lp as *const RECT);
            SetWindowPos(
                hwnd,
                null_mut(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            fonts(hwnd);
            layout(hwnd);
            0
        }
        WM_COMMAND => {
            let id = (wp & 0xffff) as i32;
            let code = (wp >> 16) as u32;
            match id {
                SCAN if code == BN_CLICKED => {
                    let path = get(hwnd, PATH);
                    if !path.trim().is_empty() {
                        start(hwnd, Action::Preview(PathBuf::from(path.trim())));
                    }
                }
                ATTACH if code == BN_CLICKED => {
                    let path = APP.with(|cell| cell.borrow().as_ref().unwrap().preview.clone());
                    if let Some(path) = path {
                        start(hwnd, Action::Attach(path));
                    }
                }
                LIBRARIES if code == CBN_SELCHANGE => {
                    if let Some(id) = selected_library(hwnd) {
                        start(hwnd, Action::Saved(id));
                    }
                }
                DETACH if code == BN_CLICKED => {
                    if let Some(id) = selected_library(hwnd) {
                        start(hwnd, Action::Detach(id));
                    }
                }
                BROWSE if code == BN_CLICKED => browse(hwnd),
                CANCEL if code == BN_CLICKED => {
                    APP.with(|cell| cell.borrow().as_ref().unwrap().cancel.cancel());
                    set(hwnd, STATUS, "正在取消扫描…");
                }
                NAV_GAME | NAV_ONLINE | NAV_ABOUT if code == BN_CLICKED => page(hwnd, id),
                FILTER if code == EN_CHANGE => {
                    SetTimer(hwnd, FILTER_TIMER, 160, None);
                }
                PATH if code == EN_CHANGE => {
                    APP.with(|cell| {
                        if let Some(a) = cell.borrow_mut().as_mut() {
                            a.preview = None;
                        }
                    });
                    EnableWindow(control(hwnd, ATTACH), 0);
                }
                _ => {}
            }
            0
        }
        WM_TIMER => {
            match wp {
                TASK_TIMER => poll(hwnd),
                FILTER_TIMER => {
                    KillTimer(hwnd, FILTER_TIMER);
                    filter(hwnd);
                }
                _ => {}
            }
            0
        }
        WM_NOTIFY => {
            let header = &*(lp as *const NMHDR);
            if header.idFrom == LIST as usize && header.code == LVN_GETDISPINFOW {
                let info = &mut *(lp as *mut NMLVDISPINFOW);
                let text = APP.with(|cell| {
                    let a = cell.borrow();
                    let a = a.as_ref()?;
                    let row = a.rows.get(*a.visible.get(info.item.iItem as usize)?)?;
                    Some(match info.item.iSubItem {
                        0 => row.name.clone(),
                        1 => row.game.clone(),
                        2 => row.loader.clone(),
                        _ => isolation(row.isolation).into(),
                    })
                });
                if info.item.mask & LVIF_TEXT != 0
                    && !info.item.pszText.is_null()
                    && info.item.cchTextMax > 0
                {
                    let text = wide(text.as_deref().unwrap_or(""));
                    let n = (text.len() - 1).min(info.item.cchTextMax as usize - 1);
                    std::ptr::copy_nonoverlapping(text.as_ptr(), info.item.pszText, n);
                    *info.item.pszText.add(n) = 0;
                }
            } else if header.idFrom == LIST as usize && header.code == LVN_ITEMCHANGED {
                let item = &*(lp as *const NMLISTVIEW);
                if item.uNewState & LVIS_SELECTED != 0 {
                    let text = APP.with(|cell| {
                        let a = cell.borrow();
                        let a = a.as_ref()?;
                        let row = a.rows.get(*a.visible.get(item.iItem as usize)?)?;
                        Some(format!(
                            "{} · {} · {}\r\n目录状态：{}\r\n实际游戏目录：{}\r\n依据：{}\r\n{}",
                            row.name,
                            row.game,
                            row.loader,
                            isolation(row.isolation),
                            row.game_dir
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| "尚未确认；不会猜测目标路径".into()),
                            if row.evidence.is_empty() {
                                "未找到明确配置".into()
                            } else {
                                row.evidence.join("；")
                            },
                            row.issue
                                .as_deref()
                                .unwrap_or("识别完成不代表此组合已通过游戏启动验证。")
                        ))
                    });
                    if let Some(text) = text {
                        set(hwnd, DETAIL, &text);
                    }
                }
            }
            0
        }
        WM_CLOSE => {
            APP.with(|cell| {
                if let Some(a) = cell.borrow().as_ref() {
                    a.cancel.cancel();
                }
            });
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, TASK_TIMER);
            KillTimer(hwnd, FILTER_TIMER);
            APP.with(|cell| {
                if let Some(a) = cell.borrow_mut().take() {
                    DeleteObject(a.font);
                    DeleteObject(a.heading);
                }
            });
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub fn run() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let com = CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
        InitCommonControlsEx(&INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES,
        });
        let class = wide("AmberL.NativeProbe");
        let instance = GetModuleHandleW(null());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class.as_ptr(),
            wide("AmberL · 原生技术样机").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1140,
            820,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            return;
        }
        for (id, label) in [
            (NAV_GAME, "游戏"),
            (NAV_ONLINE, "联机"),
            (NAV_ABOUT, "关于 / 状态"),
            (BROWSE, "选择目录"),
            (SCAN, "扫描库"),
            (CANCEL, "取消"),
            (ATTACH, "添加此库"),
            (DETACH, "移除登记"),
        ] {
            child(
                hwnd,
                "BUTTON",
                label,
                id,
                WS_TABSTOP | BS_PUSHBUTTON as u32,
                0,
            );
        }
        for (id, label) in [
            (TITLE, "你的游戏"),
            (SUBTITLE, ""),
            (STATUS, "就绪 · 只读扫描，不写入游戏库"),
        ] {
            child(hwnd, "STATIC", label, id, 0, 0);
        }
        child(
            hwnd,
            "EDIT",
            "",
            PATH,
            WS_TABSTOP | ES_AUTOHSCROLL as u32,
            WS_EX_CLIENTEDGE,
        );
        child(
            hwnd,
            "COMBOBOX",
            "",
            LIBRARIES,
            WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
            0,
        );
        child(
            hwnd,
            "EDIT",
            "",
            FILTER,
            WS_TABSTOP | ES_AUTOHSCROLL as u32,
            WS_EX_CLIENTEDGE,
        );
        SendMessageW(
            control(hwnd, PATH),
            EM_SETCUEBANNER,
            0,
            wide("游戏库路径，例如 D:\\Games\\.minecraft").as_ptr() as isize,
        );
        SendMessageW(
            control(hwnd, FILTER),
            EM_SETCUEBANNER,
            0,
            wide("搜索实例、版本或加载器…").as_ptr() as isize,
        );
        child(
            hwnd,
            "SysListView32",
            "",
            LIST,
            WS_TABSTOP | LVS_REPORT | LVS_OWNERDATA | LVS_SINGLESEL | LVS_SHOWSELALWAYS,
            WS_EX_CLIENTEDGE,
        );
        SendMessageW(
            control(hwnd, LIST),
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            0,
            (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize,
        );
        for (i, title) in ["实例", "游戏版本", "加载器", "游戏目录"]
            .iter()
            .enumerate()
        {
            let mut title = wide(title);
            let col = LVCOLUMNW {
                mask: LVCF_TEXT | LVCF_WIDTH,
                cx: 180,
                pszText: title.as_mut_ptr(),
                ..Default::default()
            };
            SendMessageW(
                control(hwnd, LIST),
                LVM_INSERTCOLUMNW,
                i,
                &col as *const _ as isize,
            );
        }
        child(
            hwnd,
            "EDIT",
            "",
            DETAIL,
            WS_TABSTOP
                | ES_MULTILINE as u32
                | ES_READONLY as u32
                | ES_AUTOVSCROLL as u32
                | WS_VSCROLL,
            WS_EX_CLIENTEDGE,
        );
        APP.with(|cell| {
            *cell.borrow_mut() = Some(App {
                rows: vec![],
                visible: vec![],
                receiver: None,
                entries: vec![],
                preview: None,
                data_dir: None,
                cancel: Cancellation::default(),
                font: null_mut(),
                heading: null_mut(),
                page: NAV_GAME,
            })
        });
        EnableWindow(control(hwnd, CANCEL), 0);
        EnableWindow(control(hwnd, ATTACH), 0);
        fonts(hwnd);
        page(hwnd, NAV_GAME);
        layout(hwnd);
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        let directory = if let Some(i) = args.iter().position(|a| a == "--data-dir") {
            args.get(i + 1)
                .map(PathBuf::from)
                .ok_or_else(|| "Missing --data-dir path".into())
        } else {
            registry::default_directory()
        };
        let initialization = directory.and_then(|path| {
            let entries = Registry::open(&path)?.entries().to_vec();
            APP.with(|cell| {
                let mut a = cell.borrow_mut();
                let a = a.as_mut().unwrap();
                a.entries = entries;
                a.data_dir = Some(path);
            });
            Ok(())
        });
        refresh_libraries(hwnd);
        if let Err(error) = initialization {
            set(hwnd, STATUS, &error);
        }
        ShowWindow(
            hwnd,
            if args.iter().any(|a| a == "--automation-hidden") {
                SW_HIDE
            } else {
                SW_SHOW
            },
        );
        UpdateWindow(hwnd);
        let mut message = MSG::default();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if com >= 0 {
            CoUninitialize();
        }
    }
}
