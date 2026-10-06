//! 伪终端里的端到端测试：刚发出去、她还没开始做事就两下 `Esc` 打断的，撤掉这一轮、那一句放回输入框（蓝图 `tui.md`
//! 「按键」`Esc`，2026-10-07 项目主人定）。真界面连一份照剧本回话的核心。

mod support;

use std::time::Duration;

use miyu_session::testkit::{Play, Script};
use support::Home;

/// 输入框里那一行。
fn input(tui: &support::Tui) -> String {
    tui.lines()
        .into_iter()
        .find(|l| l.contains('❯'))
        .unwrap_or_default()
}

#[test]
fn an_interrupt_after_she_has_spoken_keeps_the_turn() {
    // 剧本的 `Holds` 先吐一个「…」再停住：她开口了，打断以后这一轮留着，那一句不放回输入框。
    // 还没开口就打断、放回输入框的那一面，要核心的测试剧本有「不出声地等着」才测得了（已请核心加），现在靠真模型实测。
    let home = Home::new(Script::new([Play::Holds]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("说两句");
    tui.wait_for("…");
    tui.key(b"\x1b");
    tui.pump(Duration::from_millis(100));
    tui.key(b"\x1b");
    tui.wait_for("已中断");
    tui.pump(Duration::from_millis(500));
    assert!(
        tui.shows("┃ 说两句"),
        "这一轮留着：\n{}",
        tui.lines().join("\n")
    );
    assert!(!input(&tui).contains("说两句"), "不放回输入框");
}
