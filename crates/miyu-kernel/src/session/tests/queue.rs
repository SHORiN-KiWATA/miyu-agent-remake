//! 排队的消息（`docs/designs/02-内核.md` 第六节「排队的消息」）：最后一步里排着的，回复完了接着
//! 开一轮；由最后一条触发；出错、到步数上限的一样；还有下一步时来的不接着开；打断接着发、退回。

use super::executor::*;
use super::*;
use crate::event::{EndReason, MessageWithdrawn};

/// 第 `k` 条是回合开始：它的回合编号和触发。
fn started(event: &Event) -> (Option<TurnId>, Seq) {
    let Body::TurnStarted(started) = &event.body else {
        panic!("应该是 turn.started：{event:?}");
    };
    (event.turn, started.trigger)
}

fn withdrawn(event: &Event) -> &MessageWithdrawn {
    let Body::MessageWithdrawn(withdrawn) = &event.body else {
        panic!("应该是 message.withdrawn：{event:?}");
    };
    withdrawn
}

#[test]
fn a_message_during_the_last_reply_opens_the_next_turn() {
    let mut session = asking();
    allowing(&mut session, sent(5));
    // 她在写最后的回答，你补了一句：排着队，这次回复看不到。
    assert_eq!(
        appended(&allowing(&mut session, send(2, "顺便也看下 tests"))),
        seqs(&[6])
    );
    let actions = answer(&mut session, 5, "src 下有 lib.rs");
    let events = appended_events(&actions);
    assert_eq!(
        appended(&actions),
        seqs(&[7, 8, 9, 10]),
        "回复、记录、结束、下一轮开始"
    );
    assert_eq!(reason_of(&events[2]), &EndReason::Completed);
    assert_eq!(started(&events[3]), (Some(TurnId::new(seq(10))), seq(6)));
    assert_eq!(
        (events[3].at, events[3].cause.clone()),
        (at(45), Some(id(2)))
    );
    // 落了盘：先跑上一轮结束的挂接点，再跑下一轮开始的。
    let actions = allowing(&mut session, stored(10));
    let end = actions
        .iter()
        .position(|action| *action == Action::RunTurnEndHooks { turn: turn3() });
    let start = actions.iter().position(|action| {
        *action
            == Action::RunTurnStartHooks {
                turn: TurnId::new(seq(10)),
            }
    });
    assert!(
        end.is_some() && start.is_some() && end < start,
        "{actions:?}"
    );
    // 下一次请求里有那句话。
    let calls = calls(&allowing(
        &mut session,
        hooks_done(TurnId::new(seq(10)), Vec::new()),
    ));
    assert_eq!(calls[0].0, seq(10));
    assert!(calls[0].1.contains("6 message.user"));
}

#[test]
fn the_last_of_several_queued_messages_triggers() {
    let mut session = asking();
    allowing(&mut session, sent(5));
    allowing(&mut session, send(2, "一"));
    allowing(&mut session, send(3, "二"));
    let events = appended_events(&answer(&mut session, 5, "好"));
    assert_eq!(started(&events[3]).1, seq(7), "由后一条触发");
    assert_eq!(events[3].cause, Some(id(3)));
}

#[test]
fn a_failed_or_limited_turn_also_goes_on() {
    // 出错结束：认证失败，不重试。
    let mut session = asking();
    allowing(&mut session, send(2, "还在吗"));
    let events = appended_events(&allowing(
        &mut session,
        failed(5, crate::event::ErrorClass::Auth, "401"),
    ));
    assert_eq!(reason_of(&events[1]), &EndReason::Error);
    assert_eq!(started(&events[2]).1, seq(6));
    // 到步数上限结束：最后一步的工具在跑时来的。
    let mut policy = policy();
    policy.step_limit = Some(1);
    let mut session = session_with(policy);
    allowing(&mut session, send(1, "hi"));
    allowing(&mut session, stored(5));
    allowing(&mut session, hooks_done(turn3(), Vec::new()));
    call_tools(&mut session, 5, &[("read", "{}")]);
    allowing(&mut session, stored(7));
    allowing(&mut session, send(2, "接着说"));
    let events = appended_events(&allowing(&mut session, done(call(6, 1), "a")));
    assert_eq!(reason_of(&events[1]), &EndReason::StepLimit);
    assert_eq!(started(&events[2]).1, seq(8));
}

#[test]
fn a_message_heard_by_a_later_step_does_not_reopen() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("read", "{}")]);
    allowing(&mut session, stored(7));
    // 工具在跑时来的：下一次请求就有它。
    allowing(&mut session, send(2, "只看 .rs"));
    allowing(&mut session, done(call(6, 1), "a"));
    let calls = calls(&allowing(&mut session, stored(9)));
    assert!(calls[0].1.contains("8 message.user"));
    let actions = answer(&mut session, 9, "好");
    assert_eq!(
        appended(&actions),
        seqs(&[10, 11, 12]),
        "听到过了，不再接着开"
    );
}

#[test]
fn interrupting_with_send_opens_a_turn_for_the_queued() {
    let mut session = asking();
    allowing(&mut session, send(2, "别查了，先看 README"));
    let actions = allowing(&mut session, stop_with(3, at(47), Queued::Send));
    let events = appended_events(&actions);
    assert_eq!(
        appended(&actions),
        seqs(&[7, 8, 9]),
        "记录、结束、下一轮开始"
    );
    assert_eq!(reason_of(&events[1]), &EndReason::Interrupted);
    assert_eq!(started(&events[2]).1, seq(6));
    assert!(actions.contains(&Action::CancelModel { seen: seq(5) }));
}

#[test]
fn interrupting_with_return_takes_the_queued_back() {
    let mut session = asking();
    allowing(&mut session, send(2, "顺便把 README 也看了"));
    allowing(&mut session, send(4, "还有 Cargo.toml"));
    let actions = allowing(&mut session, stop_with(5, at(47), Queued::Return));
    let events = appended_events(&actions);
    assert_eq!(
        appended(&actions),
        seqs(&[8, 9, 10]),
        "记录、撤回、结束；不接着开"
    );
    assert_eq!(withdrawn(&events[1]).messages, seqs(&[6, 7]));
    assert_eq!(
        (events[1].by.clone(), events[1].cause.clone()),
        (alice(), Some(id(5)))
    );
    assert_eq!(events[1].turn, Some(turn3()), "撤回记在这一轮里");
    assert_eq!(reason_of(&events[2]), &EndReason::Interrupted);
    assert!(
        session
            .handle(stored(10))
            .contains(&accepted_reply(5, &[8, 9, 10])),
        "回应附上撤回那一条，头照着把字放回输入框"
    );
    // 撤回的话不再出现在请求里。
    allowing(&mut session, send(6, "重新来"));
    allowing(&mut session, stored(12));
    let calls = calls(&allowing(
        &mut session,
        hooks_done(TurnId::new(seq(12)), Vec::new()),
    ));
    assert!(!calls[0].1.contains("6 message.user") && !calls[0].1.contains("7 message.user"));
    assert!(!calls[0].1.contains("message.withdrawn"));
}

#[test]
fn returning_with_nothing_queued_writes_no_withdrawal() {
    let mut session = asking();
    let actions = allowing(&mut session, take_back(2));
    assert!(
        appended_events(&actions)
            .iter()
            .all(|event| !matches!(event.body, Body::MessageWithdrawn(_)))
    );
}

#[test]
fn the_trigger_is_not_queued() {
    // 开头还没落盘就按了退回：触发它的那句开了这一轮，不在队里。
    let mut session = session();
    allowing(&mut session, send(1, "hi"));
    let events = appended_events(&allowing(&mut session, take_back(2)));
    assert_eq!(events.len(), 1);
    assert_eq!(reason_of(&events[0]), &EndReason::Interrupted);
}
