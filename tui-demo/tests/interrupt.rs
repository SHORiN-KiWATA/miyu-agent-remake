//! 伪终端里的端到端测试：刚发出去、她还没开始做事就两下 `Esc` 打断的，撤掉这一轮、那一句放回输入框（蓝图 `tui.md`
//! 「按键」`Esc`，2026-10-07 项目主人定）。真界面连一份照剧本回话的核心。

mod support;

use std::time::{Duration, Instant};

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

#[test]
fn an_interrupt_before_she_starts_puts_the_message_back_in_the_box() {
    // `Stalls` 一个字都不出：她还没开始做事，打断以后撤掉这一轮，那一句放回输入框。
    let home = Home::new(Script::new([Play::Stalls, Play::Says("收到。")]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("等一下再问");
    tui.pump(Duration::from_millis(800));
    tui.key(b"\x1b");
    tui.pump(Duration::from_millis(100));
    tui.key(b"\x1b");
    let end = Instant::now() + support::WAIT;
    while !input(&tui).contains("等一下再问") {
        assert!(
            Instant::now() < end,
            "没放回输入框：\n{}",
            tui.lines().join("\n")
        );
        tui.pump(Duration::from_millis(100));
    }
    assert!(
        !tui.shows("┃ 等一下再问"),
        "这一轮撤掉了：\n{}",
        tui.lines().join("\n")
    );
    // 放回来的照常能再发。
    tui.key(b"\r");
    tui.wait_for("收到。");
}

#[test]
fn slash_stop_interrupts_and_keeps_the_queued_message_without_starting_a_turn() {
    // `/stop` 交给核心 `command.run`（2026-10-07 项目主人定）：打断这一轮，排着的话留在正文里、不接着发，回执弹提示。
    let home = Home::new(Script::new([Play::Holds, Play::Says("不该开这一轮。")]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("说两句");
    tui.wait_for("…");
    tui.say("排着的一句");
    tui.wait_for("↳ 排着的一句");
    tui.say("/stop");
    tui.wait_for("已全部停止");
    assert!(!tui.shows("已全部停止。"), "提示框里不要句号");
    tui.wait_for("已中断");
    tui.wait_for("┃ 排着的一句");
    tui.pump(Duration::from_millis(1500));
    let screen = tui.lines().join("\n");
    assert!(!screen.contains("不该开这一轮。"), "不接着开：\n{screen}");
    assert!(
        !screen.contains("↳ 排着的一句"),
        "不再画成排着的：\n{screen}"
    );
    let at = |word: &str| tui.lines().iter().position(|l| l.contains(word));
    assert!(
        at("已中断") < at("┃ 排着的一句"),
        "排着的话接在打断那一行后面：\n{screen}"
    );
}
