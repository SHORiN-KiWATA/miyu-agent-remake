//! 换模型（施工 8-10，`configure.rs`）：换了的记一条、落了盘才回应、同一个编号再来不再记；和现在一样的当场回应、什么都
//! 不记，编号照样记下；回合进行中的带上回合、回合照常；造会话时记下的引用就是现在的，回合开始交出去；载入照整份日志算
//! 回来，撤掉的回合里换的也算；改回文件的时候拒绝。退回默认的对不上不记。

use super::executor::*;
use super::load::{Logged, load};
use super::restore::edited;
use super::revert::revert;
use super::*;
use crate::event::PolicyChanged;

/// 编号是 `n` 的命令：alice 换成 `model`。
fn configure(n: u64, model: &str) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::Configure {
            model: model.to_string(),
        },
    })
}

/// 换成 `model` 的那一条的 `body`。
fn to(model: &str) -> Body {
    Body::PolicyChanged(PolicyChanged {
        model: Some(model.to_string()),
        ..PolicyChanged::default()
    })
}

/// 叫跑回合开始的挂接点时交出去的引用。
fn handed(actions: &[Action]) -> Vec<Option<String>> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::RunTurnStartHooks { model, .. } => Some(model.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_change_is_recorded_and_answered_once_stored() {
    let mut session = session();
    let actions = session.handle(configure(2, "@free"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[2]));
    assert!(replies(&actions).is_empty(), "落了盘才回应");
    assert_eq!(events[0].body, to("@free"));
    assert_eq!(
        (&events[0].by, &events[0].cause, events[0].turn),
        (&alice(), &Some(id(2)), None)
    );
    assert_eq!(
        replies(&session.handle(stored(2))),
        [&accepted_reply(2, &[2])]
    );
    assert_eq!(session.reference(), Some("@free"));
    // 同一个编号再来：照上一次回应，不再记，换成别的也一样。
    assert_eq!(
        session.handle(configure(2, "b/n")),
        [accepted_reply(2, &[2])]
    );
    // 和现在一样的：当场回应，什么都不记，编号照样记下。
    assert_eq!(
        session.handle(configure(3, "@free")),
        [accepted_reply(3, &[])]
    );
    assert_eq!(
        session.handle(configure(3, "b/n")),
        [accepted_reply(3, &[])]
    );
    assert_eq!(session.reference(), Some("@free"));
}

#[test]
fn the_model_the_session_was_made_with_is_handed_over_when_a_turn_starts() {
    let mut created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    created.model = Some("a/m".to_string());
    let (mut session, _) = Session::create(
        session_id(),
        id(0),
        alice(),
        at(0),
        created,
        policy(),
        environment("~/src/miyu"),
    );
    session.handle(stored(1));
    assert_eq!(session.reference(), Some("a/m"));
    assert_eq!(
        session.handle(configure(2, "a/m")),
        [accepted_reply(2, &[])],
        "一样的不记"
    );
    session.handle(send(3, "hi"));
    let actions = session.handle(stored(5));
    assert_eq!(handed(&actions), [Some("a/m".to_string())]);
    // 没记下引用的会话交出去的是没有：跟着 `models.chat`。
    let mut plain = super::session();
    plain.handle(send(2, "hi"));
    assert_eq!(handed(&plain.handle(stored(5))), [None]);
}

#[test]
fn a_change_during_a_turn_carries_the_turn_and_the_turn_goes_on() {
    let mut session = asking();
    let actions = session.handle(configure(9, "b/n"));
    let events = appended_events(&actions);
    assert_eq!(events[0].turn, Some(turn3()));
    assert!(!session.idle(), "回合照常");
}

#[test]
fn loading_counts_every_change_even_in_undone_turns() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.handle(configure(20, "@free"));
    logged.say(seen, "好");
    let last = logged.last();
    logged.handle(revert(21, 3));
    logged.handle(stored(last + 1));
    assert_eq!(
        logged.session.reference(),
        Some("@free"),
        "撤掉的回合里换的也算"
    );
    let (mut loaded, _) = load(logged.log.clone());
    assert_eq!(loaded.reference(), Some("@free"), "载入照日志算回来");
    assert_eq!(
        loaded.handle(configure(30, "@free")),
        [accepted_reply(30, &[])]
    );
}

#[test]
fn a_change_is_refused_while_files_are_being_restored() {
    let (mut logged, _) = edited();
    logged.handle(revert(9, 3));
    logged.handle(stored(logged.last()));
    let actions = logged.handle(configure(10, "b/n"));
    assert!(appended_events(&actions).is_empty());
    assert!(
        matches!(
            replies(&actions)[..],
            [Action::Reply {
                outcome: Outcome::Rejected {
                    reason: Reason::Restoring
                },
                ..
            }]
        ),
        "{actions:?}"
    );
}

#[test]
fn a_fallback_that_does_not_match_the_reference_is_ignored() {
    let mut session = session();
    session.handle(configure(2, "@free"));
    session.handle(stored(2));
    session.handle(send(3, "hi"));
    session.handle(stored(6));
    // 交来的原来的不是现在的引用：不记。
    let actions = session.handle(Input::TurnStartHooksDone {
        policy: None,
        prepare: false,
        at: at(30),
        turn: TurnId::new(seq(4)),
        injected: Vec::new(),
        replaced: Some(Replaced {
            from: "b/n".to_string(),
            to: "a/m".to_string(),
        }),
    });
    assert!(appended_events(&actions).is_empty(), "对不上的不记");
    assert_eq!(session.reference(), Some("@free"));
    assert_eq!(calls(&actions).len(), 1, "照常请求");
}
