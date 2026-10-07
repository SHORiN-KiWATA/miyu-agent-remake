//! 改标题、置顶（施工 3-8 三补，`meta.rs`）：改了的记一条、只写改了的格、落了盘才回应；和现在一样的不记、当场回应；
//! 去掉标题记成空的；回合进行中的带上回合；载入以后照日志算回来，撤掉的回合里改的也算。删不删得了（`deletable`）：空闲
//! 的删得了，有回合在进行、`turn.ended` 没落盘的是有回合在进行，改回文件的时候是正在改回。

use super::executor::*;
use super::load::{Logged, load};
use super::restore::{edited, steps_of};
use super::revert::revert;
use super::*;
use crate::event::MetaChanged;

/// 编号是 `n` 的命令：alice 改标题、置顶，改哪样写哪样。
fn set_meta(n: u64, title: Option<&str>, pinned: Option<bool>) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::SetMeta {
            title: title.map(str::to_string),
            pinned,
        },
    })
}

/// 一条 `session.meta_changed` 写的两格。
fn meta(title: Option<&str>, pinned: Option<bool>) -> Body {
    Body::MetaChanged(MetaChanged {
        title: title.map(str::to_string),
        pinned,
    })
}

/// 接受了、什么都没记：当场回应，序号是空的。
fn nothing(actions: &[Action], n: u64) {
    assert_eq!(actions, [accepted_reply(n, &[])], "当场回应、什么都不记");
}

#[test]
fn renaming_is_recorded_and_answered_once_stored() {
    let mut session = session();
    let actions = session.handle(set_meta(2, Some("整理 src 目录"), None));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[2]));
    assert!(replies(&actions).is_empty(), "落了盘才回应");
    assert_eq!(events[0].body, meta(Some("整理 src 目录"), None));
    assert_eq!(
        (&events[0].by, &events[0].cause, events[0].turn),
        (&alice(), &Some(id(2)), None)
    );
    assert_eq!(
        replies(&session.handle(stored(2))),
        [&accepted_reply(2, &[2])]
    );
    // 同一个编号再来：照上一次回应，不再记。
    assert_eq!(
        session.handle(set_meta(2, Some("整理 src 目录"), None)),
        [accepted_reply(2, &[2])]
    );
}

#[test]
fn only_what_changes_is_written() {
    let mut session = session();
    nothing(&session.handle(set_meta(2, Some(""), None)), 2);
    nothing(&session.handle(set_meta(3, None, Some(false))), 3);
    nothing(&session.handle(set_meta(4, None, None)), 4);
    let actions = session.handle(set_meta(5, Some("发版"), Some(true)));
    assert_eq!(
        appended_events(&actions)[0].body,
        meta(Some("发版"), Some(true))
    );
    session.handle(stored(2));
    nothing(&session.handle(set_meta(6, Some("发版"), Some(true))), 6);
    let actions = session.handle(set_meta(7, Some("发版"), Some(false)));
    assert_eq!(appended_events(&actions)[0].body, meta(None, Some(false)));
    let actions = session.handle(set_meta(8, Some("新名字"), Some(false)));
    assert_eq!(
        appended_events(&actions)[0].body,
        meta(Some("新名字"), None)
    );
    // 去掉标题记成空的；去掉以后再去掉，一样的不记。
    let actions = session.handle(set_meta(9, Some(""), None));
    assert_eq!(appended_events(&actions)[0].body, meta(Some(""), None));
    nothing(&session.handle(set_meta(10, Some(""), None)), 10);
    // 一样的也记下编号：再来照上一次回应。
    assert_eq!(
        session.handle(set_meta(6, Some("别的"), None)),
        [accepted_reply(6, &[])]
    );
}

#[test]
fn renaming_during_a_turn_carries_the_turn() {
    let mut session = asking();
    let actions = session.handle(set_meta(9, Some("等着的"), None));
    let events = appended_events(&actions);
    assert_eq!(events[0].turn, Some(turn3()));
    assert!(!session.idle(), "回合照常");
}

#[test]
fn loading_counts_every_change_even_in_undone_turns() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.handle(set_meta(20, Some("发版"), Some(true)));
    logged.say(seen, "好");
    let last = logged.last();
    logged.handle(revert(21, 3));
    logged.handle(stored(last + 1));
    logged.handle(set_meta(22, Some("新名字"), None));
    logged.handle(stored(logged.last()));
    let (mut loaded, _) = load(logged.log.clone());
    nothing(&loaded.handle(set_meta(30, Some("新名字"), Some(true))), 30);
    let actions = loaded.handle(set_meta(31, Some(""), Some(false)));
    assert_eq!(
        appended_events(&actions)[0].body,
        meta(Some(""), Some(false))
    );
}

#[test]
fn only_an_idle_session_can_be_deleted() {
    let mut session = session();
    assert_eq!(session.deletable(), Ok(()), "刚造出来");
    session.handle(send(1, "hi"));
    assert_eq!(session.deletable(), Err(Reason::TurnRunning), "开了回合");
    session.handle(stored(5));
    session.handle(hooks_done(turn3(), Vec::new()));
    answer(&mut session, 5, "你好");
    assert_eq!(
        session.deletable(),
        Err(Reason::TurnRunning),
        "turn.ended 还没落盘"
    );
    session.handle(stored(8));
    assert_eq!(session.deletable(), Ok(()));
}

#[test]
fn a_session_restoring_files_cannot_be_deleted() {
    let (mut logged, _) = edited();
    let actions = logged.handle(revert(9, 3));
    assert!(!steps_of(&actions).is_empty(), "要改回文件");
    logged.handle(stored(logged.last()));
    assert_eq!(logged.session.deletable(), Err(Reason::Restoring));
}

/// 记下用了一个斜杠命令（施工 O-6）：记一条 `command.ran`，`by`、`cause` 照命令，落了盘回应、附上它；同一个编号再来照上一次
/// 回应。不进她的请求由 `miyu-assemble` 守着。
#[test]
fn a_slash_command_is_noted_and_kept_out_of_the_request() {
    let mut logged = Logged::new();
    let ran = Input::Command(Received {
        id: id(2),
        by: alice(),
        at: at(2),
        command: Command::Ran {
            text: "/reset".to_string(),
            command: "clear".to_string(),
        },
    });
    let actions = logged.handle(ran.clone());
    let events = appended_events(&actions);
    assert_eq!(
        events[0].body,
        Body::CommandRan(crate::event::CommandRan {
            text: "/reset".to_string(),
            command: "clear".to_string(),
        })
    );
    assert_eq!((&events[0].by, &events[0].cause), (&alice(), &Some(id(2))));
    assert_eq!(
        replies(&logged.handle(stored(2))),
        [&accepted_reply(2, &[2])]
    );
    assert_eq!(logged.handle(ran), [accepted_reply(2, &[2])], "只记一次");
}
