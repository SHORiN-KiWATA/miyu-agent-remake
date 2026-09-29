//! 场景：手动压缩（`docs/blueprint/compaction.md` 第七条，施工 6-8）。人要压，单开一轮只做压缩：摘要请求替代到收下命令时
//! 落了盘的最后一条，要求接在指令前面；取到了摘要写压缩、这一轮同一批结束；调了工具改走隔离式照样带着要求；失败不数进
//! 熔断；打断不写压缩；重启以后不接着压；自动压缩暂停着也能压，压成了自动压缩恢复。
//!
//! 尾巴的预算是 0：替代到哪只看这一轮的规矩，留尾巴在 `scenario/tail.rs`。替身的组装一条事件约五个 token，说完了的
//! 请求默认报 110。

use super::*;
use crate::event::{CompactTrigger, ContextCompacted, TransientBody};
use crate::session::{Compaction, Pause};

/// 熔断的数照出厂的：3、3、3。
const PAUSE: Pause = Pause {
    failures: 3,
    turns: 3,
    refills: 3,
};

/// 一个会压缩的替身；`pause` 是熔断的数，没有的不熔断。
fn manual(pause: Option<Pause>) -> Stage {
    let make = move || {
        let mut policy = policy();
        policy.compaction = Some(Compaction {
            reserve_cap: 10,
            margin: 10,
            tail: 0,
            price: crate::estimate::Flat {
                image: 50,
                file: 50,
            },
            rebuild: None,
            pause,
            shorten: None,
            isolate: true,
        });
        policy
    };
    Stage::new(make, environment("~/src/miyu"), at(0))
}

/// 第一轮说一句就完：日志到 8。窗口大到不会自动压。
fn after_one_turn(stage: &mut Stage) {
    stage.limits(Some(100_000), None);
    stage.model([Line::says("好。")]);
    stage.say("hi");
}

/// 第一轮那三条，接着这几条。
fn first_turn_then(rest: &[&str]) -> Vec<String> {
    let mut lines = vec![
        "6 message.assistant model t3",
        "7 model.called:ok kernel t3",
        "8 turn.ended:completed kernel t3",
    ];
    lines.extend(rest);
    opening_then(&lines)
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

/// 日志里写过几次暂停。
fn pauses(stage: &Stage) -> usize {
    stage
        .log()
        .iter()
        .filter(|event| matches!(event.body, Body::CompactionPaused(_)))
        .count()
}

/// 第 `k` 次请求：看到了第几条为止，和替身的组装列出来的清单。
fn request(stage: &Stage, k: usize) -> (u64, String) {
    let (seen, request) = &stage.requests()[k];
    (seen.get(), listed_request(request))
}

#[test]
fn asked_to_compact_she_does_it_in_a_turn_of_its_own() {
    let mut stage = manual(None);
    after_one_turn(&mut stage);
    stage.model([Line::says("S1")]);
    let asked = stage.request_compaction(Some("keep the plan"));
    assert_eq!(
        story(&stage),
        first_turn_then(&[
            "9 turn.started kernel t9",
            "10 model.called:ok kernel t9",
            "11 context.compacted kernel t9",
            "12 turn.ended:completed kernel t9",
        ])
    );
    assert_eq!(
        stage.outcome(&asked),
        Some(&Outcome::Accepted {
            events: vec![seq(9)]
        })
    );
    let started = &stage.log()[8];
    assert!(matches!(&started.body, Body::TurnStarted(started) if started.trigger.is_none()));
    assert_eq!(started.cause, Some(asked.clone()), "cause 是这个命令");
    // 摘要请求替代到收下命令时落了盘的最后一条，要求接在指令前面。
    assert_eq!(
        request(&stage, 1),
        (
            8,
            listing(&stage.log()[..8]) + "instructions: keep the plan\nsummarize\n"
        )
    );
    assert_eq!(
        stage.model_calls()[1].compaction,
        Some(CompactTrigger::Manual)
    );
    let compacted = compactions(&stage)[0];
    assert_eq!(compacted.upto, seq(8));
    assert_eq!(compacted.summary, "S1");
    assert_eq!(compacted.trigger, Some(CompactTrigger::Manual));
    assert_eq!(compacted.instructions.as_deref(), Some("keep the plan"));
    assert_eq!(compacted.refills, None, "手动的不算压完很快又到线");
    assert_eq!(stage.log()[10].cause, Some(asked.clone()));
    assert_eq!(stage.log()[11].cause, Some(asked));
    // 推给头的压好了带着是哪一种压缩。
    let done = stage
        .transients()
        .iter()
        .find_map(|transient| match &transient.body {
            TransientBody::CompactionDone(done) => Some((transient.turn, done)),
            _ => None,
        })
        .unwrap();
    assert_eq!(done.0, Some(TurnId::new(seq(9))));
    assert_eq!(done.1.seen, seq(8));
    assert_eq!(done.1.trigger, CompactTrigger::Manual);
    // 下一轮开头照常比着注入事实：检查点后面还没有。
    stage.model([Line::says("嗯。")]);
    stage.say("again");
    assert_eq!(
        story(&stage)[12..16],
        [
            "13 message.user alice",
            "14 turn.started kernel t14",
            "15 context.injected:env kernel t14",
            "16 context.injected:permission kernel t14",
        ]
    );
}

#[test]
fn what_is_said_during_the_compaction_opens_the_next_turn() {
    let mut stage = manual(None);
    after_one_turn(&mut stage);
    stage.model([Line::says("S1").held(), Line::says("嗯。")]);
    stage.request_compaction(None);
    stage.say("next");
    stage.release_model();
    assert_eq!(
        story(&stage)[8..],
        [
            "9 turn.started kernel t9",
            "10 message.user alice t9",
            "11 model.called:ok kernel t9",
            "12 context.compacted kernel t9",
            "13 turn.ended:completed kernel t9",
            "14 turn.started kernel t14",
            "15 context.injected:env kernel t14",
            "16 context.injected:permission kernel t14",
            "17 message.assistant model t14",
            "18 model.called:ok kernel t14",
            "19 turn.ended:completed kernel t14",
        ]
    );
    assert_eq!(compactions(&stage)[0].upto, seq(8), "后来的那句不压进去");
    assert!(
        matches!(&stage.log()[13].body, Body::TurnStarted(started) if started.trigger == Some(seq(10)))
    );
}

#[test]
fn a_failed_request_is_retried_with_the_same_cut_and_instructions() {
    let mut stage = manual(None);
    after_one_turn(&mut stage);
    stage.model([Line::fails(ErrorClass::Retryable, "503"), Line::says("S1")]);
    stage.request_compaction(Some("keep the plan"));
    assert_eq!(
        story(&stage)[8..],
        [
            "9 turn.started kernel t9",
            "10 model.called:error kernel t9",
            "11 model.called:ok kernel t9",
            "12 context.compacted kernel t9",
            "13 turn.ended:completed kernel t9",
        ]
    );
    assert_eq!(
        request(&stage, 1),
        request(&stage, 2),
        "照同一个 N、同样的要求再发"
    );
}

/// 摘要回复里调了工具（施工 6-6 下）：改走隔离式，同一个 N、同样的要求，记的还是手动的。
#[test]
fn a_tool_call_in_the_summary_goes_again_isolated_with_the_instructions() {
    let mut stage = manual(None);
    after_one_turn(&mut stage);
    stage.model([Line::calls("", &[("read", "{}")]), Line::says("S1")]);
    stage.request_compaction(Some("keep the plan"));
    assert_eq!(
        story(&stage)[8..],
        [
            "9 turn.started kernel t9",
            "10 model.called:error kernel t9",
            "11 model.called:ok kernel t9",
            "12 context.compacted kernel t9",
            "13 turn.ended:completed kernel t9",
        ]
    );
    assert_eq!(stage.requests()[2].1.system, "isolated");
    assert_eq!(
        request(&stage, 1),
        request(&stage, 2),
        "替代到的、要求都一样"
    );
    assert!(
        stage.model_calls()[1..]
            .iter()
            .all(|called| called.compaction == Some(CompactTrigger::Manual))
    );
    let compacted = compactions(&stage)[0];
    assert_eq!(compacted.trigger, Some(CompactTrigger::Manual));
    assert_eq!(compacted.instructions.as_deref(), Some("keep the plan"));
}

#[test]
fn a_failed_manual_compaction_ends_the_turn_and_never_pauses() {
    let mut stage = manual(Some(PAUSE));
    after_one_turn(&mut stage);
    for _ in 0..3 {
        stage.model([Line::fails(ErrorClass::Auth, "denied")]);
        stage.request_compaction(None);
    }
    assert_eq!(
        story(&stage)[8..11],
        [
            "9 turn.started kernel t9",
            "10 model.called:error kernel t9",
            "11 turn.ended:error kernel t9",
        ]
    );
    assert_eq!(pauses(&stage), 0, "手动的失败不数");
    assert!(
        stage
            .model_calls()
            .iter()
            .skip(1)
            .all(|called| called.compaction == Some(CompactTrigger::Manual))
    );
}

#[test]
fn a_failed_manual_compaction_does_not_count_among_the_automatic_ones() {
    let mut stage = manual(Some(PAUSE));
    after_one_turn(&mut stage);
    // 线压到 100：有锚（报 110），往后每一轮一开头就过线。
    stage.limits(Some(120), None);
    for words in ["a", "b"] {
        stage.model([Line::fails(ErrorClass::Auth, "denied")]);
        stage.say(words);
    }
    stage.model([Line::fails(ErrorClass::Auth, "denied")]);
    stage.request_compaction(None);
    assert_eq!(pauses(&stage), 0, "两次自动的加一次手动的，不停");
    stage.model([Line::fails(ErrorClass::Auth, "denied")]);
    stage.say("c");
    assert_eq!(pauses(&stage), 1, "第三次自动的失败才停");
}

#[test]
fn an_interrupted_manual_compaction_writes_no_checkpoint() {
    let mut stage = manual(None);
    after_one_turn(&mut stage);
    stage.model([Line::says("S1").held()]);
    stage.request_compaction(None);
    stage.interrupt(Queued::Send);
    assert_eq!(
        story(&stage)[8..],
        [
            "9 turn.started kernel t9",
            "10 model.called:interrupted kernel t9",
            "11 turn.ended:interrupted alice t9",
        ]
    );
    assert!(compactions(&stage).is_empty());
}

#[test]
fn after_a_restart_she_does_not_go_on_compacting() {
    let mut stage = manual(None);
    after_one_turn(&mut stage);
    stage.model([Line::says("S1").held()]);
    stage.request_compaction(None);
    stage.say("later");
    stage.restart();
    assert_eq!(
        story(&stage)[8..],
        [
            "9 turn.started kernel t9",
            "10 message.user alice t9",
            "11 model.called:interrupted kernel t9",
            "12 turn.ended:restarted kernel t9",
        ],
        "再起来不接着压，排着的留着等人开口"
    );
    // 人开口，她连同排着的那句一起听到。
    stage.model([Line::says("嗯。")]);
    stage.say("again");
    assert!(request(&stage, 2).1.contains("10 message.user\n"));
}

#[test]
fn right_after_a_compaction_there_is_nothing_to_compact() {
    let mut stage = manual(None);
    after_one_turn(&mut stage);
    stage.model([Line::says("S1")]);
    stage.request_compaction(None);
    let before = stage.log().len();
    let again = stage.request_compaction(None);
    assert_eq!(
        stage.outcome(&again),
        Some(&Outcome::Rejected {
            reason: Reason::NothingToCompact
        })
    );
    assert_eq!(stage.log().len(), before, "拒绝的什么都不写");
}

#[test]
fn paused_she_still_compacts_by_hand_and_then_on_her_own_again() {
    let mut stage = manual(Some(PAUSE));
    after_one_turn(&mut stage);
    stage.limits(Some(120), None);
    for words in ["a", "b", "c"] {
        stage.model([Line::fails(ErrorClass::Auth, "denied")]);
        stage.say(words);
    }
    assert_eq!(pauses(&stage), 1, "连续失败三次，暂停了");
    stage.model([Line::says("S1")]);
    stage.request_compaction(None);
    assert_eq!(compactions(&stage).len(), 1, "暂停着也压");
    // 压完的第一轮用量小，照发，报 110 当锚；再下一轮过线，自动压缩回来了。
    stage.model([Line::says("嗯。"), Line::says("S2"), Line::says("好。")]);
    stage.say("d");
    stage.say("e");
    let compacted = compactions(&stage);
    assert_eq!(compacted.len(), 2);
    assert_eq!(compacted[1].trigger, Some(CompactTrigger::Auto));
    assert_eq!(pauses(&stage), 1);
}

#[test]
fn undoing_the_compaction_turn_brings_the_context_back() {
    // 手动压缩单开的那一轮也能撤（施工 6-9）：撤掉它，压缩跟着撤掉，下一次请求又是压缩前的样子。
    let mut stage = manual(None);
    after_one_turn(&mut stage);
    stage.model([Line::says("S1")]);
    stage.request_compaction(None);
    let before = request(&stage, 0).1;
    stage.revert(TurnId::new(seq(9)));
    assert!(matches!(
        stage.outcome(&id(3)),
        Some(Outcome::Accepted { .. })
    ));
    stage.model([Line::says("嗯。")]);
    stage.say("again");
    let (_, after) = stage
        .requests()
        .last()
        .map(|(seen, request)| (*seen, listed_request(request)))
        .unwrap();
    assert!(
        after.starts_with(&before),
        "压缩前的前缀回来了：\n{before}\n{after}"
    );
}
