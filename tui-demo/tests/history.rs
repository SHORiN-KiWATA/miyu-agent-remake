//! 伪终端里的端到端测试：输入历史重启以后还在、所有会话一起（蓝图 `tui.md`「输入历史列表」第 8 条，2026-10-07 项目主人定）。

mod support;

use std::time::Duration;

use miyu_session::testkit::{Play, Script};
use support::Home;

const UP: &[u8] = b"\x1b[A";

/// 输入框里那一行。
fn input(tui: &support::Tui) -> String {
    tui.lines()
        .into_iter()
        .find(|l| l.contains('❯'))
        .unwrap_or_default()
}

#[test]
fn what_was_sent_comes_back_with_up_after_a_restart_even_in_a_new_session() {
    let home = Home::new(Script::new([Play::Says("好。"), Play::Says("嗯。")]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("重启以前说的一句");
    tui.wait_for("好。");
    drop(tui);
    // 重启：新的界面、新的会话（tui.startup 照默认开新的）。
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.key(UP);
    tui.pump(Duration::from_millis(300));
    assert!(
        input(&tui).contains("重启以前说的一句"),
        "↑ 翻得到上次的：{}",
        tui.lines().join("\n")
    );
    let saved = std::fs::read_to_string(home.root().join("state/tui/history.jsonl")).unwrap();
    assert_eq!(saved.lines().count(), 1, "记在数据根里：{saved}");
}
