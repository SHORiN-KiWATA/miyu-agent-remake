//! 重做（`docs/blueprint/kernel/history.md`「重做」，施工 4-7 再补）这一层：一批追加、落了盘才回应；改过文件的先改回，
//! 改的时候拒绝命令、不算空闲，改完了 `files.restored`、重发、新的一轮同一批；同一个编号再来照上一次回应。走一整轮的
//! 场景在 `scenario/redo.rs`。

use super::load::Logged;
use super::restore::{edited, steps_of};
use super::*;
use crate::event::{FilesRestored, MessageUser};

/// `n` 号命令：alice 重做最后一轮，开这一轮的那一句换成 `words`（没有的原样）。
fn redo(n: u64, words: Option<&str>) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(56),
        command: Command::Redo {
            text: words.map(|words| {
                vec![Block::Text(Text {
                    text: words.to_string(),
                })]
            }),
            attachments: None,
        },
    })
}

/// 这几步都改回了的结局。
fn done(steps: &[Step]) -> Input {
    Input::Restored {
        at: at(58),
        files: steps.iter().map(crate::testkit::restored).collect(),
    }
}

/// 一轮（3 号，2 到 8）说完了，全落了盘。
fn one_turn() -> Logged {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.say(seen, "好");
    logged
}

#[test]
fn a_redo_is_one_batch_and_replies_when_it_is_stored() {
    let mut logged = one_turn();
    let actions = logged.handle(redo(2, Some("换个说法")));
    let events = appended_events(&actions);
    assert_eq!(
        events
            .iter()
            .map(|event| event.body.kind())
            .collect::<Vec<_>>(),
        [
            "turn.reverted",
            "message.user",
            "turn.started",
            "context.injected",
            "context.injected"
        ],
        "撤销、重发、新的一轮同一批；撤掉的那一轮的事实跟着撤了，照常再注入"
    );
    assert_eq!(appended(&actions), seqs(&[9, 10, 11, 12, 13]));
    assert_eq!(
        events[1].body,
        Body::MessageUser(MessageUser {
            blocks: vec![Block::Text(Text {
                text: "换个说法".to_string(),
            })],
        })
    );
    assert!(replies(&actions).is_empty(), "落了盘才回应");
    let actions = logged.handle(stored(13));
    assert!(
        actions.contains(&accepted_reply(2, &[9, 10])),
        "{actions:?}"
    );
    assert!(
        actions.contains(&Action::RunTurnStartHooks {
            turn: TurnId::new(seq(11)),
            model: None,
            present: Vec::new(),
        }),
        "新的一轮照常往下走"
    );
}

#[test]
fn the_same_redo_again_is_answered_as_before() {
    let mut logged = one_turn();
    logged.handle(redo(2, None));
    assert!(
        logged.handle(redo(2, None)).is_empty(),
        "还没落盘：等落了盘再回，不再生效"
    );
    let actions = logged.handle(stored(13));
    assert_eq!(
        replies(&actions),
        [&accepted_reply(2, &[9, 10]), &accepted_reply(2, &[9, 10])]
    );
    assert_eq!(logged.handle(redo(2, None)), [accepted_reply(2, &[9, 10])]);
    assert_eq!(logged.last(), 13, "什么都没再记");
}

#[test]
fn a_redo_that_changed_files_restores_them_before_saying_it_again() {
    let (mut logged, _) = edited();
    let asked = logged.log[1].clone();
    let actions = logged.handle(redo(9, None));
    let reverted = logged.last();
    assert_eq!(appended(&actions), seqs(&[reverted]), "先只记撤销");
    let steps = steps_of(&actions);
    assert_eq!(steps.len(), 1, "交出改回文件");
    logged.handle(stored(reverted));
    assert!(!logged.session.idle(), "改回文件的时候不算空闲");
    let actions = logged.handle(send(10, "hi"));
    assert!(
        matches!(
            actions.as_slice(),
            [Action::Reply {
                outcome: Outcome::Rejected {
                    reason: Reason::Restoring
                },
                ..
            }]
        ),
        "{actions:?}"
    );
    let actions = logged.handle(done(&steps));
    let events = appended_events(&actions);
    assert_eq!(
        events
            .iter()
            .map(|event| event.body.kind())
            .collect::<Vec<_>>(),
        [
            "files.restored",
            "message.user",
            "turn.started",
            "context.injected",
            "context.injected"
        ],
        "改完了：结局、重发、新的一轮同一批"
    );
    let files = steps.iter().map(crate::testkit::restored).collect();
    assert_eq!(events[0].body, Body::FilesRestored(FilesRestored { files }));
    assert_eq!(events[1].body, asked.body, "原话原样");
    assert_eq!(
        (&events[1].by, &events[1].cause, events[1].turn),
        (&alice(), &Some(id(9)), None)
    );
    for event in &events {
        assert_eq!(event.at, at(58), "时刻是改完回来的那一刻");
    }
    let (restored, resent) = (events[0].seq.get(), events[1].seq.get());
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(
        replies(&actions),
        [&accepted_reply(9, &[reverted, restored, resent])]
    );
}
