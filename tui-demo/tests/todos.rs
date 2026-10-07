//! 伪终端里的端到端测试：待办接真的核心（蓝图 `tui.md`「后台命令、子代理和侧边栏」，核心 D-3）。她调 `todowrite`，
//! 核心推 `todos.changed`，待办那一块照它画。

mod support;

use miyu_session::testkit::{Play, Script};
use support::Home;

#[test]
fn todowrite_shows_up_as_the_todo_list() {
    let args = serde_json::json!({"todos": [
        {"content": "读一遍代码", "status": "completed"},
        {"content": "改掉那个错", "status": "in_progress"},
        {"content": "跑测试", "status": "pending"}
    ]})
    .to_string();
    let script = Script::new([
        Play::Calls(vec![("todowrite".into(), args)]),
        Play::Says("列好了。"),
    ]);
    let home = Home::with_tools(script, "");
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("排一下要做的");
    tui.wait_for("列好了。");
    tui.wait_for("待办 1/3");
    tui.wait_for("改掉那个错");
    tui.wait_for("跑测试");
}

#[test]
fn coming_back_to_the_session_brings_the_todos_from_the_subscribe_reply() {
    // `todos.changed` 是瞬时的，补发里没有：回到会话时照订阅回应里的 `todos` 画（核心 D-3）。
    let args =
        serde_json::json!({"todos": [{"content": "跑测试", "status": "in_progress"}]}).to_string();
    let script = Script::new([
        Play::Calls(vec![("todowrite".into(), args)]),
        Play::Says("列好了。"),
    ]);
    let home = Home::with_tools(script, "[ui]\nstartup = \"recent\"\n");
    let mut first = home.tui("zh_CN.UTF-8");
    first.wait_for("工作区");
    first.say("排一下");
    first.wait_for("列好了。");
    drop(first);
    let mut again = home.tui("zh_CN.UTF-8");
    again.wait_for("列好了。");
    again.wait_for("待办 0/1");
    again.wait_for("跑测试");
}

#[test]
fn a_finished_list_stays_ticked_a_moment_then_folds() {
    // 2026-10-07 项目主人：todo 做完之后可以保留一会。核心 D-3 补：清空时带 `done`。
    let start = serde_json::json!({"todos": [
        {"content": "读代码", "status": "in_progress"},
        {"content": "跑测试", "status": "pending"}
    ]})
    .to_string();
    let finish = serde_json::json!({"todos": [
        {"content": "读代码", "status": "completed"},
        {"content": "跑测试", "status": "completed"}
    ]})
    .to_string();
    let script = Script::new([
        Play::Calls(vec![("todowrite".into(), start)]),
        Play::Calls(vec![("todowrite".into(), finish)]),
        Play::Says("都做完了。"),
    ]);
    let home = Home::with_tools(script, "");
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("开工");
    tui.wait_for("都做完了。");
    tui.wait_for("待办 2/2");
    let shown = std::time::Instant::now();
    let end = shown + support::WAIT;
    while tui.shows("待办 2/2") {
        assert!(std::time::Instant::now() < end, "一直不收");
        tui.pump(std::time::Duration::from_millis(100));
    }
    assert!(
        shown.elapsed() >= std::time::Duration::from_secs(3),
        "留了一会才收：{:?}",
        shown.elapsed()
    );
}
