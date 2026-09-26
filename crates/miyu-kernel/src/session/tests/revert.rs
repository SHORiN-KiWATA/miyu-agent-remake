//! 撤销与恢复（`docs/designs/02-内核.md` 第六节「撤销与恢复」）：从选中的那一轮起往后全撤，落了盘
//! 才回应；跑着时、不在有效历史里、压缩掉的，拒绝；撤了以后说一句，请求里没有撤掉的，第一处不同
//! 记下来，撤掉的事实重新注入；恢复最近一次，一次一次地恢复，下一次请求接着撤销前那一次往下长；
//! 开了下一轮就不能恢复；载入以后照样能恢复、和不崩一样；被重启打断以后撤销过的，不接着干。

use super::executor::*;
use super::load::{Logged, load};
use super::*;
use crate::event::{ContextCompacted, ModelCalled, TurnReverted, TurnUnreverted};
use crate::id::ModuleId;

/// `n` 号命令：从 `turn` 号回合起撤销。
pub(super) fn revert(n: u64, turn: u64) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(56),
        command: Command::Revert {
            turn: TurnId::new(seq(turn)),
        },
    })
}

/// `n` 号命令：恢复最近一次撤销。
pub(super) fn unrevert(n: u64) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(57),
        command: Command::Unrevert,
    })
}

fn turns(seqs: &[u64]) -> Vec<TurnId> {
    seqs.iter().map(|&n| TurnId::new(seq(n))).collect()
}

/// 两轮都走完、都落了盘的会话：第一轮 3 号（2 到 8），第二轮 10 号（9 到 13）。
fn two_turns() -> Logged {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.say(seen, "好");
    let seen = logged.ask(2, "再来");
    logged.say(seen, "又好");
    logged
}

/// 两轮的有效历史，替身的组装列出来的样子。
const BOTH_TURNS: [(u64, &str); 13] = [
    (1, "session.created"),
    (2, "message.user"),
    (3, "turn.started"),
    (4, "context.injected"),
    (5, "context.injected"),
    (6, "message.assistant"),
    (7, "model.called"),
    (8, "turn.ended"),
    (9, "message.user"),
    (10, "turn.started"),
    (11, "message.assistant"),
    (12, "model.called"),
    (13, "turn.ended"),
];

/// 请求 `seen` 说完了：记下的 `model.called`。
fn called_after(logged: &mut Logged, seen: Seq) -> ModelCalled {
    let actions = answer(&mut logged.session, seen.get(), "好");
    let events = appended_events(&actions);
    logged.log.extend(events.clone());
    let called = events
        .iter()
        .find(|event| matches!(event.body, Body::ModelCalled(_)))
        .unwrap();
    called_of(called).clone()
}

#[test]
fn undo_takes_the_last_turn_away() {
    let mut logged = two_turns();
    let actions = logged.handle(revert(3, 10));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[14]));
    assert_eq!(
        events[0].body,
        Body::TurnReverted(TurnReverted {
            turns: turns(&[10])
        })
    );
    assert_eq!(
        (
            events[0].by.clone(),
            events[0].cause.clone(),
            events[0].turn
        ),
        (alice(), Some(id(3)), None)
    );
    assert!(replies(&actions).is_empty(), "落了盘才回应");
    assert!(
        logged
            .handle(stored(14))
            .contains(&accepted_reply(3, &[14]))
    );
    // 撤了以后说一句：请求里没有撤掉的那一轮，也没有触发它的那句；前缀断了，记下第一处不同。
    let (seen, request) = logged.open(4, "换个问法");
    let mut expected = BOTH_TURNS[..8].to_vec();
    expected.extend([(15, "message.user"), (16, "turn.started")]);
    assert_eq!(request, listed(&expected));
    assert!(called_after(&mut logged, seen).first_difference.is_some());
}

#[test]
fn undo_from_an_earlier_turn_takes_the_later_ones_too() {
    let mut logged = two_turns();
    let actions = logged.handle(revert(3, 3));
    assert_eq!(
        appended_events(&actions)[0].body,
        Body::TurnReverted(TurnReverted {
            turns: turns(&[3, 10])
        })
    );
    logged.handle(stored(14));
    // 撤掉的那一轮里注入过的环境、权限都不算了，下一轮重新注入（08 C10）。
    let (_, request) = logged.open(4, "换个问法");
    assert_eq!(
        request,
        listed(&[
            (1, "session.created"),
            (15, "message.user"),
            (16, "turn.started"),
            (17, "context.injected"),
            (18, "context.injected"),
        ])
    );
}

#[test]
fn undo_waits_for_the_turn_to_stop() {
    // 请求在路上。
    let mut logged = Logged::new();
    logged.ask(1, "hi");
    assert_eq!(
        logged.handle(revert(2, 3)),
        [rejected(id(2), Reason::TurnRunning)]
    );
    // 等人确认的时候也一样：头先打断再撤。
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    let actions = call_tools(&mut logged.session, seen, &[("write", "{}")]);
    logged.log.extend(appended_events(&actions));
    logged.handle(stored(7));
    let ask = Verdict::Ask {
        module: ModuleId::parse("permissions").unwrap(),
        access: crate::tool::Access::Write,
        rule: None,
        detail: None,
    };
    logged.handle(guarded(call(6, 1), ask));
    assert_eq!(
        logged.handle(revert(2, 3)),
        [rejected(id(2), Reason::TurnRunning)]
    );
}

#[test]
fn undo_of_a_turn_not_in_history_is_refused() {
    let mut logged = two_turns();
    // 没有这一轮；2 号是消息，不是回合。
    for turn in [99, 2] {
        assert_eq!(
            logged.handle(revert(3, turn)),
            [rejected(id(3), Reason::UnknownTurn)]
        );
    }
    logged.handle(revert(3, 10));
    logged.handle(stored(14));
    assert_eq!(
        logged.handle(revert(4, 10)),
        [rejected(id(4), Reason::UnknownTurn)],
        "撤过的不再撤"
    );
}

#[test]
fn undo_of_a_compacted_turn_is_refused() {
    // 两轮之后压缩到 13，两轮都进了摘要；载入以后再走一轮，16 号。
    let logged = two_turns();
    let mut log = logged.log;
    log.push(Event {
        seq: seq(14),
        at: at(54),
        turn: None,
        by: By::Kernel,
        cause: None,
        body: Body::ContextCompacted(ContextCompacted {
            upto: seq(13),
            summary: "…".to_string(),
        }),
    });
    let (session, _) = load(log.clone());
    let mut logged = Logged { session, log };
    let seen = logged.ask(3, "接着来");
    logged.say(seen, "好");
    for turn in [3, 10, 13] {
        assert_eq!(
            logged.handle(revert(4, turn)),
            [rejected(id(4), Reason::Compacted)]
        );
    }
    assert_eq!(
        appended(&logged.handle(revert(4, 16))).len(),
        1,
        "压缩以后的照样能撤"
    );
}

#[test]
fn redo_brings_the_turns_back_and_the_next_request_grows_on() {
    let mut logged = two_turns();
    logged.handle(revert(3, 10));
    logged.handle(stored(14));
    let actions = logged.handle(unrevert(4));
    let events = appended_events(&actions);
    assert_eq!(
        events[0].body,
        Body::TurnUnreverted(TurnUnreverted {
            turns: turns(&[10])
        })
    );
    assert_eq!(
        (events[0].by.clone(), events[0].cause.clone()),
        (alice(), Some(id(4)))
    );
    assert!(
        logged
            .handle(stored(15))
            .contains(&accepted_reply(4, &[15]))
    );
    // 撤掉的都回来了；请求接着撤销前那一次往下长，前缀没断。
    let (seen, request) = logged.open(5, "换个问法");
    let mut expected = BOTH_TURNS.to_vec();
    expected.extend([(16, "message.user"), (17, "turn.started")]);
    assert_eq!(request, listed(&expected));
    assert_eq!(called_after(&mut logged, seen).first_difference, None);
}

#[test]
fn two_undos_come_back_one_at_a_time() {
    let mut logged = two_turns();
    logged.handle(revert(3, 10));
    logged.handle(revert(4, 3));
    logged.handle(stored(15));
    let first = appended_events(&logged.handle(unrevert(5)));
    assert_eq!(
        first[0].body,
        Body::TurnUnreverted(TurnUnreverted { turns: turns(&[3]) })
    );
    let second = appended_events(&logged.handle(unrevert(6)));
    assert_eq!(
        second[0].body,
        Body::TurnUnreverted(TurnUnreverted {
            turns: turns(&[10])
        })
    );
    assert_eq!(
        logged.handle(unrevert(7)),
        [rejected(id(7), Reason::NothingToUnrevert)]
    );
}

#[test]
fn after_the_next_turn_there_is_nothing_to_redo() {
    let mut logged = two_turns();
    assert_eq!(
        logged.handle(unrevert(3)),
        [rejected(id(3), Reason::NothingToUnrevert)],
        "没撤过"
    );
    logged.handle(revert(3, 10));
    logged.handle(stored(14));
    let seen = logged.ask(4, "换个问法");
    assert_eq!(
        logged.handle(unrevert(5)),
        [rejected(id(5), Reason::NothingToUnrevert)],
        "下一轮开始了"
    );
    logged.say(seen, "好");
    assert_eq!(
        logged.handle(unrevert(5)),
        [rejected(id(5), Reason::NothingToUnrevert)]
    );
}

#[test]
fn undos_load_and_go_on_the_same() {
    let mut logged = two_turns();
    logged.handle(revert(3, 3));
    logged.handle(unrevert(4));
    logged.handle(revert(5, 10));
    logged.handle(stored(16));
    // 还能恢复的时候重启了：载入以后什么都不补，照样能恢复。
    logged.handle(Input::Restarting { at: at(53) });
    let (session, actions) = load(logged.log.clone());
    assert!(actions.is_empty());
    let mut loaded = Logged {
        session,
        log: logged.log.clone(),
    };
    for side in [&mut logged, &mut loaded] {
        assert_eq!(appended(&side.handle(unrevert(6))), seqs(&[17]));
        side.handle(stored(17));
    }
    assert_eq!(loaded.open(7, "接着来"), logged.open(7, "接着来"));
}

#[test]
fn a_restarted_turn_that_was_undone_is_not_picked_up() {
    let mut logged = Logged::new();
    logged.ask(1, "hi");
    logged.handle(Input::Restarting { at: at(53) });
    logged.handle(stored(logged.last()));
    // 关之前你把它撤了：再起来不接着干。
    logged.handle(revert(2, 3));
    let (_, actions) = load(logged.log.clone());
    assert!(actions.is_empty(), "撤销过的不接着干");
}
