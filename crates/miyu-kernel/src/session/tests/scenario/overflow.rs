//! 场景：被动压缩（`docs/blueprint/compaction.md` 第六条、第十条第 3 条，施工 6-7）。主请求被供应商报上下文超长，一个
//! 字都没收到的，落了盘先压（`trigger` 是 `overflow`）再重发一次；重发还超长照出错结束，算一次失败。
//!
//! 窗口给得很大，本地估算不到线，只有供应商报的超长才压；要看自动压缩的时候再把线收紧。

use super::*;
use crate::event::{CompactTrigger, CompactionPaused, PauseReason};
use crate::session::{Compaction, Pause};

/// 熔断照出厂的 3、3、3。
const PAUSE: Pause = Pause {
    failures: 3,
    turns: 3,
    refills: 3,
};

/// 一个会被动压缩的替身：尾巴的预算 3，只够留最后一句人的话（替身的事件一条几个 token，预算大了整段都成了尾巴，前面
/// 没有能压的）。
fn overflowing() -> Stage {
    overflowing_with(PAUSE)
}

/// 同上，熔断的数是 `pause`。
fn overflowing_with(pause: Pause) -> Stage {
    overflowing_limited(pause, None)
}

/// 同上，一个回合最多请求 `step_limit` 次。
fn overflowing_limited(pause: Pause, step_limit: Option<u32>) -> Stage {
    let make = move || {
        let mut policy = policy();
        policy.step_limit = step_limit;
        policy.compaction = Some(Compaction {
            reserve_cap: 10,
            margin: 10,
            tail: 3,
            lead: 0,
            price: crate::estimate::Flat {
                image: 50,
                file: 50,
            },
            rebuild: None,
            pause: Some(pause),
            shorten: None,
            isolate: false,
        });
        policy
    };
    Stage::new(make, environment("~/src/miyu"), at(0))
}

/// 窗口很大：本地估算不到线。
fn wide(stage: &mut Stage) {
    stage.limits(Some(1_000_000), None);
}

/// 压缩线 100。
fn narrow(stage: &mut Stage) {
    stage.limits(Some(120), None);
}

/// 供应商报上下文超长。
fn too_long() -> Line {
    Line::fails(
        ErrorClass::ContextTooLong,
        "maximum context length exceeded",
    )
}

/// 日志里的压缩，照先后：替代到哪、为什么压。
fn compactions(stage: &Stage) -> Vec<(u64, Option<CompactTrigger>)> {
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => {
                Some((compacted.upto.get(), compacted.trigger.clone()))
            }
            _ => None,
        })
        .collect()
}

/// 日志里的暂停。
fn pauses(stage: &Stage) -> Vec<CompactionPaused> {
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::CompactionPaused(paused) => Some(paused.clone()),
            _ => None,
        })
        .collect()
}

/// 最后一条是不是出错结束了这一轮。
fn ended_with_error(stage: &Stage) -> bool {
    story(stage)
        .last()
        .is_some_and(|line| line.contains("turn.ended:error"))
}

#[test]
fn a_main_request_that_is_too_long_is_compacted_and_sent_again() {
    let mut stage = overflowing();
    wide(&mut stage);
    stage.model([Line::says("好。")]);
    stage.say("hi");
    let asked = stage.requests().len();
    stage.model([too_long(), Line::says("S"), Line::says("嗯。")]);
    stage.say("再说");
    // 主请求、摘要请求、重发的主请求。
    assert_eq!(stage.requests().len(), asked + 3);
    assert_eq!(compactions(&stage), [(8, Some(CompactTrigger::Overflow))]);
    let summary = stage
        .model_calls()
        .into_iter()
        .find(|called| called.compaction.is_some())
        .map(|called| called.compaction.clone());
    assert_eq!(summary, Some(Some(CompactTrigger::Overflow)));
    assert!(
        story(&stage)
            .last()
            .is_some_and(|line| line.contains("turn.ended:completed")),
        "重发的说完了"
    );
}

#[test]
fn a_half_said_reply_is_not_compacted_or_sent_again() {
    let mut stage = overflowing();
    wide(&mut stage);
    stage.model([Line::says("好。")]);
    stage.say("hi");
    stage.model([Line::breaks(
        "我先",
        ErrorClass::ContextTooLong,
        "maximum context length exceeded",
    )]);
    stage.say("再说");
    assert!(compactions(&stage).is_empty());
    assert!(ended_with_error(&stage));
}

#[test]
fn with_nothing_to_compact_it_just_fails() {
    let mut stage = overflowing();
    wide(&mut stage);
    // 第一轮的第一次请求就超长：前面只有造会话那一条，没有能压的。
    stage.model([too_long()]);
    stage.say("hi");
    assert!(compactions(&stage).is_empty());
    assert!(ended_with_error(&stage));
    assert_eq!(stage.requests().len(), 1);
}

#[test]
fn the_last_group_stays_even_when_it_is_bigger_than_the_tail() {
    let mut stage = overflowing();
    wide(&mut stage);
    stage.model([Line::says("好。")]);
    stage.say("hi");
    // 第二轮调一次工具，结果很长（比尾巴的预算 3 大得多）；下一次请求超长。
    stage.model([
        Line::calls("", &[("read", "{}")]),
        too_long(),
        Line::says("S"),
        Line::says("看完了。"),
    ]);
    stage.tools([Play::Done("x".repeat(400))]);
    stage.say("看看");
    let reply = stage
        .log()
        .iter()
        .filter(|event| matches!(event.body, Body::MessageAssistant(_)))
        .nth(1)
        .map(|event| event.seq.get())
        .unwrap();
    // 替代到调工具的那条回复前面：回复和它很长的结果都留在尾巴里。
    assert_eq!(
        compactions(&stage),
        [(reply - 1, Some(CompactTrigger::Overflow))]
    );
}

#[test]
fn a_resend_that_is_still_too_long_counts_as_a_failure() {
    let mut stage = overflowing();
    wide(&mut stage);
    stage.model([Line::says("好。")]);
    stage.say("hi");
    // 被动压完，重发还是超长：出错结束，算一次失败。
    stage.model([too_long(), Line::says("S"), too_long()]);
    stage.say("a");
    assert!(ended_with_error(&stage));
    assert_eq!(compactions(&stage).len(), 1, "一步只压一次");
    // 接着两轮到线自动压、摘要请求出错：连同上面那一次数到 3，暂停。
    stage.model([Line::says("好。").reports(500)]);
    wide(&mut stage);
    stage.say("b");
    narrow(&mut stage);
    stage.model([Line::fails(ErrorClass::Auth, "denied")]);
    stage.say("c");
    assert!(pauses(&stage).is_empty(), "只数到 2");
    stage.model([Line::fails(ErrorClass::Auth, "denied")]);
    stage.say("d");
    assert_eq!(
        pauses(&stage),
        [CompactionPaused {
            reason: PauseReason::Failures,
            failures: Some(3),
            entry: None,
        }]
    );
}

/// 重发还超长的这一次正好数到次数（熔断的数是 1）：暂停排在这一轮的结束前面。它没有摘要请求，`after_failure` 看是
/// 哪一种压缩时照数（施工 6-8：手动的才不数）。
#[test]
fn a_resend_that_reaches_the_count_pauses_before_the_turn_ends() {
    let mut stage = overflowing_with(Pause {
        failures: 1,
        ..PAUSE
    });
    wide(&mut stage);
    stage.model([Line::says("好。")]);
    stage.say("hi");
    stage.model([too_long(), Line::says("S"), too_long()]);
    stage.say("a");
    assert!(ended_with_error(&stage));
    assert_eq!(
        pauses(&stage),
        [CompactionPaused {
            reason: PauseReason::Failures,
            failures: Some(1),
            entry: None,
        }]
    );
}

#[test]
fn while_paused_a_too_long_request_is_not_compacted() {
    let mut stage = overflowing();
    wide(&mut stage);
    stage.model([Line::says("好。").reports(500)]);
    stage.say("hi");
    narrow(&mut stage);
    for words in ["a", "b", "c"] {
        stage.model([Line::fails(ErrorClass::Auth, "denied")]);
        stage.say(words);
    }
    assert_eq!(pauses(&stage).len(), 1);
    wide(&mut stage);
    let asked = stage.requests().len();
    stage.model([too_long()]);
    stage.say("d");
    assert_eq!(stage.requests().len(), asked + 1, "暂停着不压，不重发");
    assert!(compactions(&stage).is_empty());
    assert!(ended_with_error(&stage));
}

#[test]
fn quick_refills_count_for_passive_compactions_too() {
    let mut stage = overflowing();
    wide(&mut stage);
    stage.model([Line::says("好。")]);
    stage.say("hi");
    for words in ["a", "b", "c"] {
        stage.model([too_long(), Line::says("S"), Line::says("嗯。")]);
        stage.say(words);
    }
    let refills: Vec<_> = stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some(compacted.refills),
            _ => None,
        })
        .collect();
    assert_eq!(refills, [None, Some(1), Some(2)]);
    // 第四次：不压，写暂停，这一轮照出错结束。
    stage.model([too_long()]);
    stage.say("d");
    assert_eq!(compactions(&stage).len(), 3);
    assert_eq!(
        pauses(&stage).first().map(|paused| paused.reason.clone()),
        Some(PauseReason::TooLarge)
    );
    assert!(ended_with_error(&stage));
}

#[test]
fn a_passive_summary_that_must_be_retried_is_still_the_same_compaction() {
    let mut stage = overflowing();
    wide(&mut stage);
    stage.model([Line::says("好。")]);
    stage.say("hi");
    // 被动压缩的摘要请求出了能再来的错：到点了照样再压（它不看压缩线），压完才重发主请求。
    stage.model([
        too_long(),
        Line::fails(ErrorClass::Retryable, "503"),
        Line::says("S"),
        Line::says("嗯。"),
    ]);
    stage.say("再说");
    let summaries: Vec<_> = stage
        .model_calls()
        .into_iter()
        .filter_map(|called| called.compaction.clone())
        .collect();
    assert_eq!(
        summaries,
        [CompactTrigger::Overflow, CompactTrigger::Overflow]
    );
    assert_eq!(compactions(&stage), [(8, Some(CompactTrigger::Overflow))]);
    assert!(
        story(&stage)
            .last()
            .is_some_and(|line| line.contains("turn.ended:completed"))
    );
}

#[test]
fn each_step_may_compact_once() {
    let mut stage = overflowing();
    wide(&mut stage);
    stage.model([Line::says("好。")]);
    stage.say("hi");
    // 第一步报超长，压了重发，这一次调工具；下一步又报超长：新的一步，照样压一次再重发。
    stage.model([
        too_long(),
        Line::says("S1"),
        Line::calls("", &[("read", "{}")]),
        too_long(),
        Line::says("S2"),
        Line::says("看完了。"),
    ]);
    stage.tools([Play::Done("a.rs".to_string())]);
    stage.say("看看");
    assert_eq!(compactions(&stage).len(), 2);
    assert!(
        story(&stage)
            .last()
            .is_some_and(|line| line.contains("turn.ended:completed"))
    );
}

#[test]
fn the_resend_after_a_passive_compaction_is_the_same_step() {
    // 一个回合最多请求两次：第一次报超长，压完重发的还是第一步（第六条第 1 条「这一步改成先压缩，再重发一次」）；重发的
    // 调了工具，第二步照发，说完了这一轮照常结束，不是到了步数上限。
    let mut stage = overflowing_limited(PAUSE, Some(2));
    wide(&mut stage);
    stage.model([Line::says("好。")]);
    stage.say("hi");
    let asked = stage.requests().len();
    stage.disk("~/src/miyu/a.txt", "a");
    stage.tools([Play::read("~/src/miyu/a.txt", "a")]);
    stage.model([
        too_long(),
        Line::says("S"),
        Line::calls("", &[("read", "{}")]),
        Line::says("读完了。"),
    ]);
    stage.say("读 a");
    assert_eq!(
        stage.requests().len(),
        asked + 4,
        "主请求、摘要请求、重发、第二步"
    );
    let story = story(&stage);
    assert!(
        story
            .last()
            .is_some_and(|line| line.contains("turn.ended:completed")),
        "{story:#?}"
    );
}
