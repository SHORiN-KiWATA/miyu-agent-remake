//! 伪终端里的端到端测试：交给核心 `command.run` 的斜杠命令（核心 O-6，蓝图 `tui.md`「斜杠命令」）。真界面连一份照剧本
//! 回话的核心。

mod support;

use std::time::Duration;

use miyu_session::testkit::{Play, Script};
use support::Home;

#[test]
fn slash_clear_goes_through_the_core_and_its_receipt_is_not_shown() {
    // 2026-10-07 项目主人要各个头统一走 `command.run`；`/clear` 的回执不弹，正文里有那一行。
    let home = Home::new(Script::new([Play::Says("好。")]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("在吗");
    tui.wait_for("好。");
    tui.say("/clear");
    tui.wait_for("上下文已清空");
    tui.pump(Duration::from_millis(500));
    let screen = tui.lines().join("\n");
    assert!(!screen.contains("已清空上下文。"), "回执不弹：\n{screen}");
}

#[test]
fn slash_stop_before_any_session_says_nothing_is_running() {
    let home = Home::new(Script::new([Play::Says("好。")]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("/stop");
    tui.wait_for("没在运行");
}
