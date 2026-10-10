//! 工作区是会话的属性（蓝图 `tui.md`「新会话：人格、工作区」第 5 条，核心 9-7 上）：接着老会话说话，她照会话记着的目录
//! 干活；终端在别的目录接上时正文末尾写一行暗色的「她在 <目录> 干活」，侧边栏「工作目录」写会话的。

mod support;

use std::time::Duration;

use miyu_session::testkit::{Play, Script};
use support::Home;

#[test]
fn resuming_from_another_directory_says_where_she_works() {
    let home = Home::with_settings(
        Script::new([Play::Says("在这。")]),
        "[ui]\nstartup = \"recent\"\n",
    );
    let mut first = home.tui_wide("zh_CN.UTF-8", 140);
    first.wait_for("工作区");
    first.say("开个会话");
    first.wait_for("在这。");
    first.pump(Duration::from_millis(300));
    assert!(!first.shows("切换到当前目录"), "在同一个目录不用说");
    drop(first);
    let elsewhere = std::env::temp_dir().join(format!("miyu-tui-elsewhere-{}", std::process::id()));
    std::fs::create_dir_all(&elsewhere).expect("建得了");
    let work = home
        .work
        .file_name()
        .expect("有名字")
        .to_string_lossy()
        .into_owned();
    let mut tui = home.tui_wide_in("zh_CN.UTF-8", 140, &elsewhere);
    tui.wait_for("在这。");
    tui.wait_for("切换到当前目录");
    let screen = tui.lines().join("\n");
    let line = tui
        .lines()
        .into_iter()
        .find(|l| l.contains("切换到当前目录"))
        .unwrap_or_default();
    assert!(
        line.contains("工作区：") && line.contains(&work),
        "{screen}"
    );
    // 侧边栏的工作区是会话的，不是终端所在的。
    assert!(screen.matches(&work).count() >= 2, "{screen}");
    assert!(!screen.contains("miyu-tui-elsewhere"), "{screen}");
    drop(std::fs::remove_dir_all(&elsewhere));
}

/// 一个用完就删的目录，名字带 `name`。
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("miyu-tui-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::canonicalize(&dir).expect("在")
}

#[test]
fn the_workspace_command_moves_an_open_session_and_says_why_it_cannot() {
    let home = Home::new(Script::new([Play::Says("在这。")]));
    let mut tui = home.tui_wide("zh_CN.UTF-8", 140);
    tui.wait_for("工作区");
    tui.say("开个会话");
    tui.wait_for("在这。");
    let other = scratch("ws-other");
    let name = other.file_name().unwrap().to_string_lossy().into_owned();
    tui.say(&format!("/workspace {}", other.display()));
    tui.wait_for("工作区已切换到");
    let screen = tui.lines().join("\n");
    assert!(
        screen.matches(&name).count() >= 2,
        "正文那一行、侧边栏都写它：\n{screen}"
    );
    // 换不成的弹提示、不换。
    tui.say("/workspace /nonexistent-miyu-dir");
    tui.wait_for("目录不存在");
    tui.say(&format!("/workspace {}", home.root().display()));
    tui.wait_for("该目录不能用作工作区");
    drop(std::fs::remove_dir_all(&other));
}

#[test]
fn before_the_first_word_the_workspace_is_where_the_new_session_opens() {
    let home = Home::new(Script::new([Play::Says("在这。")]));
    let mut tui = home.tui_wide("zh_CN.UTF-8", 140);
    tui.wait_for("工作区");
    let other = scratch("ws-new");
    let name = other.file_name().unwrap().to_string_lossy().into_owned();
    tui.say(&format!("/workspace {}", other.display()));
    tui.wait_for("新会话的工作区");
    tui.say("开个会话");
    tui.wait_for("在这。");
    tui.pump(Duration::from_millis(300));
    let screen = tui.lines().join("\n");
    assert!(
        screen.contains(&name),
        "侧边栏写新会话开在的目录：\n{screen}"
    );
    // 不带路径：弹提示，不开框（2026-10-10 项目主人：「不应该出现菜单，而是应该通知：请指定具体路径」）。
    tui.say("/workspace");
    tui.wait_for("请指定路径");
    let screen = tui.lines().join("\n");
    assert!(!screen.contains("输入路径"), "{screen}");
    drop(std::fs::remove_dir_all(&other));
}
