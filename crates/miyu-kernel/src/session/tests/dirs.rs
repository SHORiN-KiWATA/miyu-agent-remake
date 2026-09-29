//! 加进来的目录跟着回合走（施工 5-10 上，`docs/blueprint/kernel/session.md`、`kernel/events-bodies.md`）：开回合时取
//! 会话现在的环境，这一轮里不变；`turn.started` 带着它，没有就不写；判权限、派工具的两个动作都带上。

use super::executor::*;
use super::*;

/// 东九区、`~/src/miyu`，加进来的是 `dirs`。
fn with_dirs(dirs: &[&str]) -> Environment {
    Environment {
        dirs: dirs.iter().map(|dir| (*dir).to_string()).collect(),
        ..environment("~/src/miyu")
    }
}

/// 判权限、派工具的动作里带的加进来的目录，照先后。
fn dirs_of(actions: &[Action]) -> Vec<(&'static str, Vec<String>)> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::GuardTool { dirs, .. } => Some(("guard", dirs.clone())),
            Action::RunTool { dirs, .. } => Some(("run", dirs.clone())),
            _ => None,
        })
        .collect()
}

fn owned(dirs: &[&str]) -> Vec<String> {
    dirs.iter().map(|dir| (*dir).to_string()).collect()
}

/// 环境里加进来的是 `dirs`，开一轮、第一次请求发出去了。
fn asking_with(dirs: &[&str]) -> Session {
    let mut session = session();
    session.handle(Input::Environment(with_dirs(dirs)));
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    assert_eq!(
        calls(&session.handle(hooks_done(turn3(), Vec::new()))).len(),
        1
    );
    session
}

#[test]
fn the_turn_started_carries_the_added_directories() {
    let mut session = session();
    session.handle(Input::Environment(with_dirs(&["/work/extra", "~/notes"])));
    let events = appended_events(&session.handle(send(1, "hi")));
    let Body::TurnStarted(started) = &events[1].body else {
        panic!("第 2 条该是 turn.started：{:?}", events[1]);
    };
    assert_eq!(started.dirs, owned(&["/work/extra", "~/notes"]));
}

#[test]
fn without_added_directories_the_turn_started_is_written_as_before() {
    let mut session = session();
    let events = appended_events(&session.handle(send(1, "hi")));
    let Body::TurnStarted(started) = &events[1].body else {
        panic!("第 2 条该是 turn.started：{:?}", events[1]);
    };
    assert!(started.dirs.is_empty());
    assert_eq!(
        serde_json::to_string(started).unwrap(),
        r#"{"trigger":2,"cwd":"~/src/miyu"}"#,
        "没有加进来的目录就不写这一格：原来的日志一个字节不变"
    );
}

#[test]
fn guarding_and_running_a_call_carry_the_added_directories() {
    let mut session = asking_with(&["/work/extra"]);
    call_tools(&mut session, 5, &[("read", "{}")]);
    assert_eq!(
        dirs_of(&allowing(&mut session, stored(7))),
        [
            ("guard", owned(&["/work/extra"])),
            ("run", owned(&["/work/extra"]))
        ]
    );
}

#[test]
fn a_turn_keeps_the_directories_it_started_with() {
    let mut session = asking_with(&["/work/extra"]);
    session.handle(Input::Environment(with_dirs(&["/elsewhere"])));
    call_tools(&mut session, 5, &[("read", "{}")]);
    assert_eq!(
        dirs_of(&allowing(&mut session, stored(7))),
        [
            ("guard", owned(&["/work/extra"])),
            ("run", owned(&["/work/extra"]))
        ],
        "回合中途报来的，下一轮才用"
    );
}
