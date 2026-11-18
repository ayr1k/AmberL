// SPDX-License-Identifier: MPL-2.0
//! Suggested next steps; raw diagnostics remain available and are never reclassified as success.
pub fn help(english: bool, message: &str) -> &'static str {
    let m = message.to_lowercase();
    let contains = |values: &[&str]| values.iter().any(|v| m.contains(v));
    let pair = if contains(&["cancelled", "canceled", "已取消"]) {
        (
            "操作已取消。可回到原页面重试；已校验的下载可复用。",
            "Cancelled. Retry from the original page; verified cached downloads can be reused.",
        )
    } else if contains(&[
        "https failed",
        "https 请求失败",
        "http status",
        "timed out",
        "timeout",
        "download interrupted",
        "网络",
        "下载中断",
    ]) {
        (
            "检查网络后重试。若提示登录失效，请在微软账户页重新登录；不要关闭安全校验。",
            "Check the connection and retry. If sign-in has expired, sign in again on the Microsoft account page. Keep verification enabled.",
        )
    } else if contains(&[
        "java version",
        "java architecture",
        "java executable",
        "java 版本",
        "java 可执行",
        "兼容的 java",
        "java 路径",
    ]) {
        (
            "打开启动配置，选择与游戏兼容的 Java；也可使用下载 Java 功能。",
            "Open launch settings and choose compatible Java, or use Download Java.",
        )
    } else if contains(&["budget", "空间", "磁盘", "disk full", "os error 112"]) {
        (
            "检查目标磁盘可用空间与空间预算，再重试。不要删除来源游戏或未确认的恢复目录。",
            "Check free space and the size budget before retrying. Preserve source games and recovery directories.",
        )
    } else if contains(&[
        "expired",
        "sign in",
        "授权失效",
        "登录失效",
        "账户已失效",
        "ownership",
        "拥有 minecraft",
    ]) {
        (
            "在微软账户页重新登录，并确认该账户拥有 Minecraft Java 版。",
            "Sign in again and confirm this Microsoft account owns Minecraft Java Edition.",
        )
    } else if contains(&[
        "checksum",
        "hash mismatch",
        "校验失败",
        "校验不符",
        "integrity",
    ]) {
        (
            "文件校验未通过。重试下载；不要绕过校验或使用来源不明的替代文件。",
            "Integrity verification failed. Retry the download; do not bypass checks or substitute unknown files.",
        )
    } else if contains(&["already exists", "已存在", "同名", "collision", "目标冲突"]) {
        (
            "选择新的副本名称或空闲目标位置，再重试。已有目录不会被覆盖。",
            "Choose a new copy name or unused destination and retry. Existing destinations are not overwritten.",
        )
    } else if contains(&["changed", "已变化", "发生变化", "冲突", "重新扫描"]) {
        (
            "刷新游戏库并重新选择目标，核对隔离位置和版本后再操作。",
            "Refresh the library, select the target again, and confirm its data location and version.",
        )
    } else {
        return "";
    };
    if english { pair.1 } else { pair.0 }
}
