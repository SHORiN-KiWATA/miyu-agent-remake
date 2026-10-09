//! 是谁要她做的（施工 O-2 下，`asked.rs`）：派工具带上她这时在回应的那一条的 `by`。开回合时是触发的那一条（照记下的几条开的，
//! 是最后一条）；并进来的、排着队的被下一次请求看到以后换上它。

use super::super::executor::*;
use super::*;
use crate::id::{ExternalId, VenueId};
use crate::origin::External;

/// 群里的 `n` 号人。
fn member(n: u32) -> By {
    By::External(External {
        venue: VenueId::parse("qq:group:1").unwrap(),
        id: ExternalId::parse(&format!("qq:{n}")).unwrap(),
        account: None,
        role: None,
    })
}

/// 编号是 `n` 的命令：`by` 说的一句旁听。
fn heard(n: u64, by: By) -> Input {
    let Input::Command(mut received) = overheard(n) else {
        unreachable!();
    };
    received.by = by;
    Input::Command(received)
}

/// 请求 `seen` 调了一件 `read`，说完了，回复落了盘，链都放行：交回派出去的调用带的 `asked`。
fn asked_by_the_next_call(logged: &mut Logged, seen: u64) -> Vec<Option<By>> {
    let actions = call_tools(&mut logged.session, seen, &[("read", "{}")]);
    logged.log.extend(appended_events(&actions));
    logged
        .allowing(stored(logged.last()))
        .iter()
        .filter_map(|action| match action {
            Action::RunTool { asked, .. } => Some(asked.clone()),
            _ => None,
        })
        .collect()
}

/// 落了盘的会话，旁听记下了 1 号到 `n` 号人各一句（第 2 条起）。
fn members(n: u32) -> Logged {
    let mut logged = Logged::new();
    for k in 1..=n {
        logged.handle(heard(u64::from(k), member(k)));
    }
    logged.handle(stored(logged.last()));
    logged
}

/// 回合开头的挂接点跑完，交回请求的 `seen`。
fn hooked(logged: &mut Logged) -> u64 {
    let turn = logged
        .log
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| TurnId::new(event.seq))
        .unwrap();
    let actions = logged.handle(hooks_done(turn, Vec::new()));
    calls(&actions).remove(0).0.get()
}

#[test]
fn a_message_that_opens_the_turn_is_who_asked() {
    let mut logged = Logged::new();
    let seen = logged.ask(4, "hi");
    assert_eq!(asked_by_the_next_call(&mut logged, seen), [Some(alice())]);
}

#[test]
fn responding_to_several_is_asked_by_the_last_one() {
    let mut logged = members(2);
    logged.handle(respond(5, &[3, 2]));
    logged.handle(stored(logged.last()));
    let seen = hooked(&mut logged);
    assert_eq!(asked_by_the_next_call(&mut logged, seen), [Some(member(2))]);
}

#[test]
fn a_join_heard_by_the_next_request_is_who_asks_from_then_on() {
    let mut logged = members(3);
    logged.handle(respond(5, &[2]));
    logged.handle(stored(logged.last()));
    let seen = hooked(&mut logged);
    assert_eq!(asked_by_the_next_call(&mut logged, seen), [Some(member(1))]);
    let reply = logged
        .log
        .iter()
        .rev()
        .find(|event| event.body.kind() == "message.assistant")
        .unwrap()
        .seq
        .get();
    logged.handle(respond(9, &[4, 3]));
    logged.handle(stored(logged.last()));
    logged.handle(done(call(reply, 1), "a"));
    let actions = logged.handle(stored(logged.last()));
    let seen = calls(&actions).remove(0).0.get();
    assert_eq!(
        asked_by_the_next_call(&mut logged, seen),
        [Some(member(3))],
        "并进来的几条照最后一条"
    );
}

#[test]
fn a_queued_message_is_who_asks_once_the_request_sees_it() {
    let mut logged = Logged::new();
    let seen = logged.ask(4, "hi");
    let reply_actions = call_tools(&mut logged.session, seen, &[("read", "{}")]);
    logged.log.extend(appended_events(&reply_actions));
    logged.allowing(stored(logged.last()));
    let reply = logged
        .log
        .iter()
        .rev()
        .find(|event| event.body.kind() == "message.assistant")
        .unwrap()
        .seq
        .get();
    let Input::Command(mut queued) = send(9, "也看看这个") else {
        unreachable!();
    };
    queued.by = member(3);
    logged.handle(Input::Command(queued));
    logged.handle(stored(logged.last()));
    logged.handle(done(call(reply, 1), "a"));
    let actions = logged.handle(stored(logged.last()));
    let seen = calls(&actions).remove(0).0.get();
    assert_eq!(asked_by_the_next_call(&mut logged, seen), [Some(member(3))]);
}

/// 后台命令、子代理的回报是她自己派出去的事，请求看到它也不换；别的 harness 发来的话换，几条里照最新的那一条。
#[test]
fn her_own_jobs_do_not_change_who_asked_but_a_harness_does() {
    use crate::id::JobId;
    use crate::session::jobs::{Arrived, Waker};
    let mut logged = members(2);
    logged.ask(4, "hi");
    let arrived = |seq: u64, waker: Waker| Arrived {
        seq: Seq::new(seq).unwrap(),
        cause: None,
        waker,
    };
    let turn = logged.session.turn.as_mut().unwrap();
    turn.reports
        .push(arrived(3, Waker::Job(JobId::parse("j1").unwrap())));
    assert_eq!(logged.session.newly_asked(), None, "回报不换");
    if let Some(turn) = logged.session.turn.as_mut() {
        turn.reports = vec![arrived(3, Waker::Harness), arrived(2, Waker::Harness)];
    }
    assert_eq!(
        logged.session.newly_asked(),
        Some(member(2)),
        "别的 harness 发来的换，照最新的那一条"
    );
}
