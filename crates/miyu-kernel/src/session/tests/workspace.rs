//! 换工作区（施工 9-7 上，`workspace.rs`）：变了的记一条、落了盘才回应，下一轮的 `turn.started` 照新的；和现在一样的不记、当场
//! 回应；只写变了的 `dirs`；回合进行中换的带上这个回合。

use super::*;
use crate::event::WorkspaceChanged;

/// 编号是 `n` 的命令：alice 换工作区。
fn set_workspace(n: u64, cwd: &str, dirs: Option<&[&str]>) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::SetWorkspace {
            cwd: cwd.to_string(),
            dirs: dirs.map(|dirs| dirs.iter().map(|dir| (*dir).to_string()).collect()),
        },
    })
}

fn changed(cwd: &str, dirs: Option<&[&str]>) -> Body {
    Body::WorkspaceChanged(WorkspaceChanged {
        cwd: cwd.to_string(),
        dirs: dirs.map(|dirs| dirs.iter().map(|dir| (*dir).to_string()).collect()),
    })
}

#[test]
fn a_change_is_recorded_and_the_next_turn_starts_there() {
    let mut session = session();
    let actions = session.handle(set_workspace(2, "~/src/other", Some(&["~/notes"])));
    let events = appended_events(&actions);
    assert_eq!(events[0].body, changed("~/src/other", Some(&["~/notes"])));
    assert_eq!(
        (&events[0].by, &events[0].cause, events[0].turn),
        (&alice(), &Some(id(2)), None)
    );
    assert!(replies(&actions).is_empty(), "落了盘才回应");
    assert_eq!(
        replies(&session.handle(stored(2))),
        [&accepted_reply(2, &[2])]
    );
    assert_eq!(session.cwd(), "~/src/other");
    let events = appended_events(&session.handle(send(3, "hi")));
    let Body::TurnStarted(started) = &events[1].body else {
        panic!("第 2 条该是 turn.started：{:?}", events[1]);
    };
    assert_eq!(
        (started.cwd.as_deref(), started.dirs.clone()),
        (Some("~/src/other"), vec!["~/notes".to_string()]),
        "下一轮照新的"
    );
}

#[test]
fn the_same_workspace_is_not_recorded_and_only_what_changed_is_written() {
    let mut session = session();
    assert_eq!(
        session.handle(set_workspace(2, "~/src/miyu", None)),
        [accepted_reply(2, &[])],
        "一样的：当场回应、什么都不记"
    );
    assert_eq!(
        session.handle(set_workspace(3, "~/src/miyu", Some(&[]))),
        [accepted_reply(3, &[])],
        "加进来的目录也一样"
    );
    let dirs_only =
        appended_events(&session.handle(set_workspace(4, "~/src/miyu", Some(&["~/notes"]))));
    assert_eq!(dirs_only[0].body, changed("~/src/miyu", Some(&["~/notes"])));
    session.handle(stored(2));
    let cwd_only = appended_events(&session.handle(set_workspace(5, "/work", Some(&["~/notes"]))));
    assert_eq!(cwd_only[0].body, changed("/work", None), "dirs 没变的不写");
}

#[test]
fn a_change_during_a_turn_carries_the_turn() {
    let mut session = session();
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    let events = appended_events(&session.handle(set_workspace(2, "/work", None)));
    assert_eq!(events[0].turn, Some(turn3()), "回合进行中换的带上这个回合");
}
