// SPDX-License-Identifier: MPL-2.0
//! Presentation strings only. Never translate identities, user names or provider titles.
pub fn text(english: bool, value: &str) -> &str {
    if !english {
        return value;
    }
    match value {
        "操作完成" => "Completed",
        "正在处理…" => "Working…",
        "选择或安装一个游戏" => "Choose or install a game",
        "你的下一段冒险" => "Your next adventure",
        "安装游戏，或添加已有游戏库" => "Install a game or add an existing library",
        "选择账户" => "Choose account",
        "登录微软账户" => "Sign in with Microsoft",
        "选择检测到的 Java" => "Choose detected Java",
        "无法使用" => "Unavailable",
        "原版" => "Vanilla",
        "已安装" => "Installed",
        "安装未完成" => "Installation incomplete",
        "移除未完成" => "Removal incomplete",
        "已移除，可恢复" => "Removed; recoverable",
        "恢复未完成" => "Restoration incomplete",
        "创建服务端，或使用邀请加入房间" => {
            "Create a server or join an invitation room"
        }
        "未启动" => "Not started",
        "未加入房间" => "Not in a room",
        "尚未查询" => "Not queried",
        "未启动 FRP" => "FRP not started",
        "正在校验 FRP 配置与组件" => "Checking FRP settings and component",
        "正在停止 FRP" => "Stopping FRP",
        "正在处理联机配置…" => "Updating multiplayer settings…",
        "请先选择游戏" => "Choose a game first",
        "请先选择账户" => "Choose an account first",
        "请等待当前操作完成" => "Wait for the current operation to finish",
        "请先返回游戏页选择游戏" => "Choose a game on the Games page first",
        "请选择 AmberL 创建的游戏" => "Choose an AmberL-created game",
        "请选择备份" => "Choose a backup",
        "请选择已移除游戏" => "Choose a removed game",
        "请选择图标" => "Choose an icon",
        "无效内存" => "Invalid memory value",
        "独立目录" => "Isolated folder",
        "共享目录" => "Shared folder",
        "隔离待确认" => "Isolation unknown",
        "隔离冲突" => "Isolation conflict",
        "安装记录待核验" => "Installation check pending",
        "不匹配当前实例" => "Not compatible with this instance",
        "正在校验并启动" => "Verifying and starting",
        "正在请求保存并停止，等待进程退出" => {
            "Requesting save and stop; waiting for exit"
        }
        "目标可达性尚未检查" => "Reachability has not been checked",
        "正在校验并启动联机组件" => "Verifying and starting room component",
        "正在离开，等待联机进程退出" => "Leaving; waiting for room process to exit",
        "正在查询 Minecraft 状态…" => "Querying Minecraft status…",
        "FRP 进程运行中，会尝试重连；外部可达性尚未确认" => {
            "FRP running; reconnects automatically. External reachability unconfirmed"
        }
        "服务端报告就绪；外部可达性尚未确认" => {
            "Server reports ready; external reachability unconfirmed"
        }
        "正在保存并停止" => "Saving and stopping",
        "联机进程运行中；尚不能证明对方可达" => {
            "Room process running; peer reachability unconfirmed"
        }
        "联机配置已完成" => "Multiplayer settings completed",
        "服务端已正常退出" => "Server exited normally",
        "已离开房间" => "Left the room",
        "FRP 已停止" => "FRP stopped",
        "正在核验游戏文件…" => "Verifying game files…",
        "正在刷新并验证微软账户…" => "Refreshing and verifying Microsoft account…",
        "已请求游戏正常退出，等待保存和结束…" => {
            "Normal exit requested; waiting for saves and exit…"
        }
        "游戏已正常退出" => "Game exited normally",
        "先选择项目与版本，再预览安装内容。" => {
            "Choose a project and version, then preview the installation."
        }
        "可选依赖可在下方选择；更改后会重新校验整份清单。" => {
            "Choose optional dependencies below. Changes recheck the entire plan."
        }
        "移除会按此安装组处理，并保留可恢复副本；其他安装组仍在使用的依赖会保留。" => {
            "Removal applies to this group and keeps recoverable copies. Dependencies used by other groups are retained."
        }
        _ => value,
    }
}
pub fn message(english: bool, value: &str) -> String {
    if !english {
        return value.into();
    }
    for (prefix, suffix, before, after) in [
        (
            "游戏进程已启动（",
            "）；进入世界状态未知",
            "Game process ",
            " started; world entry unknown",
        ),
        (
            "进程 ",
            " 已运行，正在准备世界",
            "Process ",
            " running; preparing world",
        ),
        (
            "游戏退出，代码 ",
            "。请查看游戏日志。",
            "Game exited with code ",
            ". Check the game log.",
        ),
        (
            "正在下载并校验 Java ",
            "",
            "Downloading and verifying Java ",
            "",
        ),
        ("下载与校验：", " 个文件", "Download and verify: ", " files"),
        ("安装：", " 个文件", "Installing: ", " files"),
        ("正在准备 ", "…", "Preparing ", "…"),
        ("尚未确认可达：", "", "Reachability unconfirmed: ", ""),
        ("备份已创建 · ", "", "Backup created · ", ""),
        (
            "已恢复为独立副本：",
            "",
            "Restored as a separate copy: ",
            "",
        ),
    ] {
        if let Some(middle) = value
            .strip_prefix(prefix)
            .and_then(|s| s.strip_suffix(suffix))
        {
            return format!("{before}{middle}{after}");
        }
    }
    if let Some(query) = value.strip_prefix("本次查询 ") {
        return format!(
            "Query {}",
            query
                .replace("：Minecraft ", ": Minecraft ")
                .replace(" 可响应 · ", " responded · ")
                .replace(" 人 · ", " players · ")
                .replace(
                    "；入服仍需游戏确认",
                    "; joining still needs game confirmation"
                )
        );
    }
    text(true, value).into()
}
pub fn device_prompt(english: bool, value: &str) -> String {
    if !english || !value.starts_with("在微软官方页面输入设备码：\n") {
        return value.into();
    }
    value
        .replace(
            "在微软官方页面输入设备码：",
            "Enter this device code on Microsoft's official page:",
        )
        .replace("有效期 ", "Expires in ")
        .replace(
            " 分钟；请勿将密码或验证码发送给他人。",
            " minutes. Do not share passwords or verification codes.",
        )
}
pub fn subtitle(english: bool, value: &str) -> String {
    match value.rsplit_once(" · ") {
        Some((prefix, state)) => format!("{prefix} · {}", text(english, state)),
        None => text(english, value).into(),
    }
}
pub fn content_target(english: bool, value: &str) -> String {
    if english {
        for (prefix, translated) in [("游戏 · ", "Game · "), ("服务端 · ", "Server · ")] {
            if let Some(name) = value.strip_prefix(prefix) {
                return format!("{translated}{name}");
            }
        }
    }
    value.into()
}
