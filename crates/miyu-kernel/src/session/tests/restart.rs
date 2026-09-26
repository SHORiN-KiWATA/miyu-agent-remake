//! 有计划的重启（`docs/designs/02-内核.md` 第六节「载入、崩溃、重启」第 3、4 条）：照打断收拾；
//! 没有回合时什么都不做；排着队的不接着开，再起来由最后一条触发；没排着队的由那条结束触发；
//! 连着 4 轮被重启打断的，不再接；中间有一轮正常走完的，从头数。

use super::executor::*;
use super::load::{Logged, ended_with, load};
use super::*;
use crate::event::{EndReason, ToolStatus};

/// 07:00:53 要重启了。
fn restarting() -> Input {
    Input::Restarting { at: at(53) }
}

#[test]
fn restarting_closes_the_turn_as_an_interrupt_would() {
    // 工具在跑：在跑的叫停，没结果的都补上。
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.tools(seen, &[("read", "{}"), ("write", "{}")]);
    let actions = logged.handle(restarting());
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8, 9, 10]));
    for (k, call_id) in [(0, call(6, 1)), (1, call(6, 2))] {
        assert_eq!(
            result_of(&events[k]),
            (
                call_id,
                ToolStatus::Cancelled,
                By::Kernel,
                "restarted".to_string()
            )
        );
    }
    assert_eq!(ended_with(&events[2]), &EndReason::Restarted);
    assert_eq!(
        (events[2].by.clone(), events[2].cause.clone()),
        (By::Kernel, Some(id(1)))
    );
    assert_eq!(stopped(&actions), [call(6, 1)], "在跑的叫停，没跑的不用");
    // 请求在路上：截下半截，叫执行器别再发。
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.handle(sent(seen));
    for piece in words(0, "说到一半") {
        logged.handle(delta(seen, 41, piece));
    }
    let actions = logged.handle(restarting());
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[6, 7, 8]));
    assert!(matches!(&events[0].body, Body::MessageAssistant(reply) if reply.interrupted));
    assert_eq!(ended_with(&events[2]), &EndReason::Restarted);
    assert!(actions.contains(&Action::CancelModel { seen: seq(seen) }));
    // 没有回合在进行：什么都不做。
    let mut logged = Logged::new();
    assert!(logged.handle(restarting()).is_empty());
}

#[test]
fn after_a_planned_restart_the_turn_is_picked_up() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.tools(seen, &[("read", "{}")]);
    logged.handle(restarting());
    logged.handle(stored(9));
    let (mut loaded, actions) = load(logged.log.clone());
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[10]), "环境和权限都没变");
    let Body::TurnStarted(started) = &events[0].body else {
        panic!("{events:?}");
    };
    assert_eq!(
        (started.trigger, events[0].cause.clone(), events[0].at),
        (seq(9), Some(id(1)), at(55)),
        "没有排着队的，由那条结束触发"
    );
    // 落了盘跑挂接点；请求里有那条结束，她看得到为什么停了。
    assert_eq!(hooks(&loaded.handle(stored(10))), [TurnId::new(seq(10))]);
    let (_, request) =
        calls(&loaded.handle(hooks_done(TurnId::new(seq(10)), Vec::new()))).remove(0);
    assert!(request.contains("9 turn.ended"), "{request}");
}

#[test]
fn queued_messages_wait_for_the_restart_and_start_the_next_turn() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.tools(seen, &[("read", "{}")]);
    logged.handle(send(2, "顺便看看 README"));
    let actions = logged.handle(restarting());
    let events = appended_events(&actions);
    assert_eq!(
        appended(&actions),
        seqs(&[9, 10]),
        "排着队的不接着开：要关了"
    );
    assert_eq!(ended_with(&events[1]), &EndReason::Restarted);
    logged.handle(stored(10));
    let (_, actions) = load(logged.log.clone());
    let events = appended_events(&actions);
    let Body::TurnStarted(started) = &events[0].body else {
        panic!("{events:?}");
    };
    assert_eq!(
        (events[0].seq, started.trigger, events[0].cause.clone()),
        (seq(11), seq(8), Some(id(2))),
        "由排着队的最后一条触发"
    );
}

#[test]
fn four_restarts_in_a_row_are_not_picked_up() {
    let mut logged = Logged::new();
    logged.ask(1, "hi");
    logged.handle(restarting());
    let mut log = logged.log.clone();
    for round in 1..=3 {
        let (mut loaded, actions) = load(log.clone());
        assert!(
            !appended(&actions).is_empty(),
            "第 {round} 次重启以后接着干"
        );
        log.extend(appended_events(&actions));
        let last = log.last().unwrap().seq.get();
        loaded.handle(stored(last));
        log.extend(appended_events(&loaded.handle(restarting())));
    }
    let (_, actions) = load(log);
    assert!(actions.is_empty(), "连着 4 轮被重启打断，不再接");
}

#[test]
fn a_turn_that_finishes_starts_the_count_again() {
    let mut logged = Logged::new();
    logged.ask(1, "hi");
    logged.handle(restarting());
    // 接着干的那一轮正常走完了。
    let (session, actions) = load(logged.log.clone());
    let mut logged = Logged {
        session,
        log: logged.log,
    };
    logged.log.extend(appended_events(&actions));
    logged.handle(stored(logged.last()));
    let resumed = TurnId::new(seq(logged.last()));
    let seen = calls(&logged.handle(hooks_done(resumed, Vec::new())))[0]
        .0
        .get();
    logged.say(seen, "做完了");
    // 从头数：再被重启打断 3 轮，都接着干。
    logged.ask(2, "再来");
    logged.handle(restarting());
    let mut log = logged.log;
    for round in 1..=3 {
        let (mut loaded, actions) = load(log.clone());
        assert!(
            !appended(&actions).is_empty(),
            "从头数的第 {round} 次重启以后接着干"
        );
        log.extend(appended_events(&actions));
        let last = log.last().unwrap().seq.get();
        loaded.handle(stored(last));
        log.extend(appended_events(&loaded.handle(restarting())));
    }
}

#[test]
fn after_you_speak_the_count_starts_again() {
    // 连着 4 轮被重启打断，不再接。
    let mut logged = Logged::new();
    logged.ask(1, "hi");
    logged.handle(restarting());
    let mut log = logged.log.clone();
    for _ in 1..=3 {
        let (mut loaded, actions) = load(log.clone());
        log.extend(appended_events(&actions));
        let last = log.last().unwrap().seq.get();
        loaded.handle(stored(last));
        log.extend(appended_events(&loaded.handle(restarting())));
    }
    let (session, actions) = load(log.clone());
    assert!(actions.is_empty(), "连着 4 轮被重启打断，不再接");
    // 你开口开的那一轮，从头数：它被重启打断了，照样接着干。
    let mut logged = Logged { session, log };
    logged.ask(2, "接着来");
    logged.handle(restarting());
    let (_, actions) = load(logged.log.clone());
    assert!(
        !appended(&actions).is_empty(),
        "你开口以后开的那一轮从头数，被重启打断了照样接着干"
    );
}
