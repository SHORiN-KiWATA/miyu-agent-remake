//! 正在跑一轮时照记下的几条（施工 O-14 下）：桥的事实、`turn.joined` 带这个回合当场记下；下一步听到的不另开；这一步说完、
//! 没再请求就结束的接着开一轮，由 `turn.joined` 触发；打断不退回、不接着开；在等人的调用作废。

use super::super::executor::*;
use super::super::question::build_question;
use super::*;
use crate::event::{ToolStatus, TurnJoined};

/// 日志里最后一条这一种的。
fn last_of(log: &[Event], kind: &str) -> Event {
    log.iter()
        .rev()
        .find(|event| event.body.kind() == kind)
        .cloned()
        .unwrap_or_else(|| panic!("没有 {kind}"))
}

fn count(log: &[Event], kind: &str) -> usize {
    log.iter().filter(|event| event.body.kind() == kind).count()
}

#[test]
fn a_join_while_running_is_recorded_and_opens_the_next_turn_if_unheard() {
    let mut logged = overheard_twice();
    let seen = logged.ask(4, "hi");
    let turn = last_of(&logged.log, "turn.started").turn;
    let actions = logged.handle(respond(9, &[3, 2]));
    let events = appended_events(&actions);
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(
        (events[0].by.clone(), events[0].turn),
        (bridge(), turn),
        "桥的事实在前"
    );
    assert_eq!(
        events[1].body,
        Body::TurnJoined(TurnJoined {
            triggers: seqs(&[2, 3])
        })
    );
    assert_eq!(
        (
            events[1].by.clone(),
            events[1].cause.clone(),
            events[1].turn
        ),
        (By::Kernel, Some(id(9)), turn)
    );
    let stored_actions = logged.handle(stored(logged.last()));
    assert_eq!(
        outcome(&stored_actions),
        Some(&Outcome::Accepted {
            events: events.iter().map(|event| event.seq).collect()
        })
    );
    // 这一步没调工具，说完就结束：她没听到，接着开一轮，由 `turn.joined` 触发。
    logged.say(seen, "嗯");
    let started = last_of(&logged.log, "turn.started");
    let Body::TurnStarted(next) = &started.body else {
        unreachable!();
    };
    assert_eq!(next.trigger, Some(events[1].seq));
    assert!(next.triggers.is_empty());
}

#[test]
fn a_join_heard_by_the_next_step_opens_no_other_turn() {
    let mut logged = overheard_twice();
    let seen = logged.ask(4, "hi");
    logged.tools(seen, &[("read", "{}")]);
    let reply = last_of(&logged.log, "message.assistant").seq.get();
    logged.handle(respond(9, &[2]));
    logged.handle(stored(logged.last()));
    logged.handle(done(call(reply, 1), "a"));
    let actions = logged.handle(stored(logged.last()));
    let (seen, request) = calls(&actions).remove(0);
    assert!(
        request.contains("turn.joined"),
        "下一次请求就有它：{request}"
    );
    logged.say(seen.get(), "好");
    assert_eq!(count(&logged.log, "turn.started"), 1, "听到了的不另开");
    assert_eq!(count(&logged.log, "turn.ended"), 1);
}

#[test]
fn an_interrupt_neither_returns_a_join_nor_opens_a_turn_for_it() {
    for (n, stop) in [interrupt(10), take_back(10)].into_iter().enumerate() {
        let mut logged = overheard_twice();
        logged.ask(4, "hi");
        logged.handle(respond(9, &[2]));
        logged.handle(stored(logged.last()));
        logged.handle(stop);
        logged.handle(stored(logged.last()));
        assert_eq!(count(&logged.log, "message.withdrawn"), 0, "{n}：不退回");
        assert_eq!(count(&logged.log, "turn.started"), 1, "{n}：不接着开");
    }
}

#[test]
fn a_join_voids_the_question_she_is_waiting_on() {
    let mut logged = overheard_twice();
    let seen = logged.ask(4, "hi");
    logged.tools(seen, &[("ask_user", "{}")]);
    let reply = last_of(&logged.log, "message.assistant").seq.get();
    logged.handle(asks(call(reply, 1), build_question()));
    logged.handle(stored(logged.last()));
    let actions = logged.handle(respond(9, &[2]));
    let voided = appended_events(&actions)
        .into_iter()
        .find(|event| matches!(event.body, Body::ToolResult(_)))
        .expect("作废了在等的那一问");
    assert_eq!(result_of(&voided).1, ToolStatus::Skipped);
    assert_eq!(stopped(&actions), [call(reply, 1)]);
}
