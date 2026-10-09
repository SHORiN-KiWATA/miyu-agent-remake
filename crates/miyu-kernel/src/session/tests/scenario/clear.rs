//! 场景：清空上下文（`docs/blueprint/compaction.md` 第十四条，施工 6-8 补）。人要清空，单开一轮、同一批写空的检查点和结束，
//! 不请求模型；下一轮只看到清空以后的，开头照常注入两块事实；撤掉那一轮回到清空以前，恢复了又清空；自动压缩暂停着也清，
//! 清完暂停跟着解；只有一份摘要、只有一条回报也清得掉；记在一边的回报跟着清掉；载入以后一样。
//!
//! 替身的组装一条事件一行（[`Listing`]）：清空的检查点、那一轮的开头结尾怎么渲染在 `miyu-assemble` 那边测。

use super::reports::{CHILD, dispatched};
use super::*;
use crate::event::{ChildReason, CompactTrigger, ContextCompacted};
use crate::session::{Compaction, Pause};

/// 一个会压缩的替身，尾巴的预算是 0；`pause` 是熔断的数，没有的不熔断。
fn compacting(pause: Option<Pause>) -> Stage {
    let make = move || {
        let mut policy = policy();
        policy.compaction = Some(Compaction {
            reserve_cap: 10,
            margin: 10,
            tail: 0,
            lead: 0,
            price: crate::estimate::Flat {
                image: 50,
                file: 50,
            },
            rebuild: None,
            pause,
            shorten: None,
            isolate: false,
        });
        policy
    };
    Stage::new(make, environment("~/src/miyu"), at(0))
}

/// 第一轮读一次文件再说完：日志到 11（6 回复、7 模型调用、8 结果、9 回复、10 模型调用、11 结束）。
fn after_a_read(stage: &mut Stage) {
    stage.model([
        Line::calls("我看看。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("好。"),
    ]);
    stage.tools([Play::done("A")]);
    stage.say("hi");
    assert_eq!(stage.log().len(), 11);
}

/// 日志里的压缩，照先后。
fn compactions(stage: &Stage) -> Vec<&ContextCompacted> {
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some(compacted),
            _ => None,
        })
        .collect()
}

/// 最后一次请求，照替身的组装一条一行。
fn last_request(stage: &Stage) -> String {
    let (_, request) = stage.requests().last().expect("请求过");
    listed_request(request)
}

#[test]
fn cleared_she_starts_over_with_the_facts_and_what_is_said_next() {
    let mut stage = stage();
    after_a_read(&mut stage);
    let asked = stage.request_clear();
    assert_eq!(
        story(&stage)[11..],
        [
            "12 turn.started kernel t12",
            "13 context.compacted kernel t12",
            "14 turn.ended:completed kernel t12",
        ]
    );
    assert_eq!(
        stage.outcome(&asked),
        Some(&Outcome::Accepted {
            events: seqs(&[12])
        })
    );
    assert_eq!(stage.requests().len(), 2, "不请求模型");
    let compacted = compactions(&stage)[0];
    assert_eq!(compacted.upto, seq(11));
    assert_eq!(compacted.trigger, Some(CompactTrigger::Clear));
    assert!(compacted.summary.is_empty());
    // 下一轮：清空以后的只有那一轮的开头结尾，两块事实重新注入。
    stage.model([Line::says("嗯。")]);
    stage.say("刚才说了什么？");
    assert_eq!(
        story(&stage)[14..18],
        [
            "15 message.user alice",
            "16 turn.started kernel t16",
            "17 context.injected:env kernel t16",
            "18 context.injected:permission kernel t16",
        ]
    );
    assert_eq!(
        last_request(&stage),
        "12 turn.started\n14 turn.ended\n15 message.user\n16 turn.started\n17 context.injected\n18 context.injected\n"
    );
}

#[test]
fn undoing_the_clear_brings_the_context_back_and_redoing_clears_it_again() {
    let mut stage = stage();
    after_a_read(&mut stage);
    let before = listed_request(&stage.requests()[1].1);
    stage.request_clear();
    let undone = stage.revert(TurnId::new(seq(12)));
    assert!(matches!(
        stage.outcome(&undone),
        Some(Outcome::Accepted { .. })
    ));
    let redone = stage.unrevert();
    assert!(matches!(
        stage.outcome(&redone),
        Some(Outcome::Accepted { .. })
    ));
    stage.model([Line::says("嗯。")]);
    stage.say("again");
    assert!(
        last_request(&stage).starts_with("12 turn.started\n14 turn.ended\n"),
        "恢复了，又是清空以后的：\n{}",
        last_request(&stage)
    );
    // 撤掉清空（连同刚才那一轮）：回到清空以前，前缀接得上最后一次请求。
    let undone = stage.revert(TurnId::new(seq(12)));
    assert!(matches!(
        stage.outcome(&undone),
        Some(Outcome::Accepted { .. })
    ));
    stage.model([Line::says("嗯。")]);
    stage.say("again");
    let after = last_request(&stage);
    assert!(after.starts_with(&before), "{before}\n{after}");
}

#[test]
fn a_clear_works_while_paused_and_lifts_the_pause() {
    let pause = Pause {
        failures: 3,
        turns: 3,
        refills: 3,
    };
    let mut stage = compacting(Some(pause));
    stage.limits(Some(100_000), None);
    stage.model([Line::says("好。")]);
    stage.say("hi");
    // 线压到 100：有锚（报 110），往后每一轮一开头就过线，摘要请求连着失败三次，暂停了。
    stage.limits(Some(120), None);
    for words in ["a", "b", "c"] {
        stage.model([Line::fails(ErrorClass::Auth, "denied")]);
        stage.say(words);
    }
    let paused = |stage: &Stage| {
        stage
            .log()
            .iter()
            .filter(|event| matches!(event.body, Body::CompactionPaused(_)))
            .count()
    };
    assert_eq!(paused(&stage), 1);
    let asked = stage.request_clear();
    assert!(matches!(
        stage.outcome(&asked),
        Some(Outcome::Accepted { .. })
    ));
    // 清完的第一轮用量小，照发，报 110 当锚；再下一轮过线，自动压缩回来了。
    stage.model([Line::says("嗯。"), Line::says("S2"), Line::says("好。")]);
    stage.say("d");
    stage.say("e");
    let triggers: Vec<_> = compactions(&stage)
        .iter()
        .map(|compacted| compacted.trigger.clone())
        .collect();
    assert_eq!(
        triggers,
        [Some(CompactTrigger::Clear), Some(CompactTrigger::Auto)]
    );
    assert_eq!(paused(&stage), 1, "没有再暂停");
}

#[test]
fn a_summary_alone_is_something_to_clear() {
    let mut stage = compacting(None);
    stage.limits(Some(100_000), None);
    stage.model([Line::says("好。"), Line::says("S1")]);
    stage.say("hi");
    stage.request_compaction(None);
    // 手动压缩压得一条不剩：检查点后面只有那一轮的开头结尾，可摘要还在上下文里。
    let asked = stage.request_clear();
    assert!(matches!(
        stage.outcome(&asked),
        Some(Outcome::Accepted { .. })
    ));
    let again = stage.request_clear();
    assert_eq!(
        stage.outcome(&again),
        Some(&Outcome::Rejected {
            reason: Reason::NothingToClear,
            about: Vec::new(),
        })
    );
}

#[test]
fn a_cleared_session_loads_the_same() {
    let mut stage = stage();
    after_a_read(&mut stage);
    stage.request_clear();
    stage.crash();
    stage.model([Line::says("嗯。")]);
    stage.say("again");
    assert!(
        last_request(&stage).starts_with("12 turn.started\n14 turn.ended\n15 message.user\n"),
        "{}",
        last_request(&stage)
    );
    let again = stage.request_clear();
    assert!(matches!(
        stage.outcome(&again),
        Some(Outcome::Accepted { .. })
    ));
}

/// 回报也是上下文（施工 6-8 补）：清空以后子代理回报了，由它开的那一轮没等回复就打断了，检查点后面只有这条回报，照样清。
#[test]
fn a_report_alone_is_something_to_clear() {
    let mut s = dispatched(stage());
    s.request_clear();
    // 一个字都没说就停住：打断不写回复。
    s.model([Line::says("").held()]);
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    s.interrupt(Queued::Send);
    assert!(
        !s.log()[12..]
            .iter()
            .any(|event| matches!(event.body, Body::MessageUser(_) | Body::MessageAssistant(_))),
        "{:#?}",
        story(&s)
    );
    let asked = s.request_clear();
    assert!(
        matches!(s.outcome(&asked), Some(Outcome::Accepted { .. })),
        "{:?}",
        s.outcome(&asked)
    );
}

/// 清空那一轮也清掉记在一边的回报（施工 7-2 的规矩）：撤掉清空再恢复，不由它们另开一轮。
#[test]
fn a_clear_turn_also_clears_the_waiting_reports() {
    let mut s = dispatched(stage());
    s.model([Line::says("好。")]);
    s.say("再来");
    let latest = *s.turns().last().unwrap();
    s.revert(latest);
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    s.request_clear();
    let clear = *s.turns().last().unwrap();
    s.revert(clear);
    let turns = s.turns().len();
    s.unrevert();
    assert_eq!(
        s.turns().len(),
        turns,
        "清空那一轮开了，记在一边的清掉：恢复以后不由它开"
    );
}
