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
    // 重启：新的界面、新的会话（ui.startup 照默认开新的）。
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.key(UP);
    tui.pump(Duration::from_millis(300));
    assert!(
        input(&tui).contains("重启以前说的一句"),
        "↑ 翻得到上次的：{}",
        tui.lines().join("\n")
    );
    // 记在终端这个软件包的状态目录里（核心 9-1 上，`package.list` 的 `state`）。
    let saved =
        std::fs::read_to_string(home.root().join("state/packages/tui/history.jsonl")).unwrap();
    assert_eq!(saved.lines().count(), 1, "记在包的状态目录里：{saved}");
}

#[test]
fn history_from_the_old_place_moves_into_the_package_state_dir() {
    // 9-1 以前记在数据根的 `state/tui/`：新位置还没有的，先搬过去再读。
    let home = Home::new(Script::new([Play::Says("好。")]));
    let old = home.root().join("state/tui/history.jsonl");
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(&old, "{\"at\":1,\"text\":\"老地方记的一句\"}\n").unwrap();
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.pump(Duration::from_millis(500));
    tui.key(UP);
    tui.pump(Duration::from_millis(300));
    assert!(
        input(&tui).contains("老地方记的一句"),
        "{}",
        tui.lines().join("\n")
    );
    assert!(!old.exists(), "老位置的搬走了");
    let moved =
        std::fs::read_to_string(home.root().join("state/packages/tui/history.jsonl")).unwrap();
    assert!(moved.contains("老地方记的一句"));
}
