//! 场景：熔断（`docs/blueprint/compaction.md` 第十条、第二条第 5 条，施工 6-6 上）。自动压缩连续失败 3 次、压完 3 个
//! 回合内又到线连着 3 次，写 `context.compaction_paused`；暂停着到线不压，放得下的照发，放不下的记一条没发出去的
//! `model.called`、这一轮出错结束。
//!
//! 压缩线 = 窗口 − 10 − 100（输出预留的上限 10、余量 100），暂停着放不放得下看窗口 − 10：中间留出 100 的一段，好让
//! 「过了线、放得下」测得到。替身的组装一条事件约五个 token，说完了的请求默认报 110。

use super::*;
use crate::event::{CallResult, CompactTrigger, CompactionPaused, PauseReason};
use crate::session::{Compaction, Pause};

/// 熔断的数照出厂的：3、3、3。
const PAUSE: Pause = Pause {
    failures: 3,
    turns: 3,
    refills: 3,
};

/// 压缩的数；`pause` 是熔断的数，没有的不熔断。
fn compaction(pause: Option<Pause>) -> Compaction {
    Compaction {
        reserve_cap: 10,
        margin: 100,
        tail: 0,
        price: crate::estimate::Flat {
            image: 50,
            file: 50,
        },
        rebuild: None,
        pause,
        shorten: None,
    }
}

/// 一个会压缩、会熔断的替身。
fn breaking(pause: Option<Pause>) -> Stage {
    let make = move || {
        let mut policy = policy();
        policy.compaction = Some(compaction(pause));
        policy
    };
    Stage::new(make, environment("~/src/miyu"), at(0))
}

/// 交限额，压缩线正好是 `line`：窗口 = 线 + 预留 10 + 余量 100。
fn line(stage: &mut Stage, line: u64) {
    stage.limits(Some(line + 110), None);
}

/// 线高到不会压。
fn no_line(stage: &mut Stage) {
    stage.limits(Some(1_000_000), None);
}

/// 第一轮说一句就完（报 110，有了锚），再交压缩线 100：第二轮一开头就过线。
fn one_turn_then_line(stage: &mut Stage) {
    stage.model([Line::says("好。")]);
    stage.say("hi");
    line(stage, 100);
}

/// 一轮到线，摘要请求出了不再来的错：一次压缩失败。
fn failed_compaction(stage: &mut Stage, words: &str) {
    stage.model([Line::fails(ErrorClass::Auth, "denied")]);
    stage.say(words);
}

/// 第 `from` 条以后的日志，一条一行。
fn since(stage: &Stage, from: usize) -> Vec<String> {
    stage.log()[from..].iter().map(told).collect()
}

/// 日志里的暂停，照先后。
fn pauses(stage: &Stage) -> Vec<&CompactionPaused> {
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::CompactionPaused(paused) => Some(paused),
            _ => None,
        })
        .collect()
}

/// 日志里每次压缩的 `refills`，照先后。
fn refills(stage: &Stage) -> Vec<Option<u32>> {
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some(compacted.refills),
            _ => None,
        })
        .collect()
}

/// 发过几次摘要请求。
fn summaries(stage: &Stage) -> usize {
    stage
        .model_calls()
        .iter()
        .filter(|called| called.compaction.is_some())
        .count()
}

#[test]
fn the_third_failed_compaction_in_a_row_pauses_before_the_turn_ends() {
    let mut stage = breaking(Some(PAUSE));
    one_turn_then_line(&mut stage);
    failed_compaction(&mut stage, "a");
    failed_compaction(&mut stage, "b");
    assert!(pauses(&stage).is_empty(), "两次还不停");
    let from = stage.log().len();
    failed_compaction(&mut stage, "c");
    let turn = from + 2;
    assert_eq!(
        since(&stage, from),
        [
            format!("{} message.user alice", from + 1),
            format!("{turn} turn.started kernel t{turn}"),
            format!("{} model.called:error kernel t{turn}", from + 3),
            format!("{} context.compaction_paused kernel t{turn}", from + 4),
            format!("{} turn.ended:error kernel t{turn}", from + 5),
        ]
    );
    assert_eq!(
        pauses(&stage),
        [&CompactionPaused {
            reason: PauseReason::Failures,
            failures: Some(3),
            entry: None,
        }]
    );
    // 每次摘要请求的记录都写着是自动压缩的。
    let failed: Vec<_> = stage
        .model_calls()
        .into_iter()
        .filter(|called| called.result == CallResult::Error)
        .map(|called| called.compaction.clone())
        .collect();
    assert_eq!(failed, vec![Some(CompactTrigger::Auto); 3]);
}

#[test]
fn while_paused_a_request_that_fits_goes_out_without_compacting() {
    let mut stage = breaking(Some(PAUSE));
    one_turn_then_line(&mut stage);
    for words in ["a", "b", "c"] {
        failed_compaction(&mut stage, words);
    }
    let asked = stage.requests().len();
    stage.model([Line::says("好。")]);
    stage.say("d");
    assert_eq!(summaries(&stage), 3, "暂停着不再压");
    assert_eq!(stage.requests().len(), asked + 1, "过了线、放得下，照发");
    assert_eq!(
        story(&stage).last().map(String::as_str),
        Some(
            format!(
                "{} turn.ended:completed kernel t{}",
                stage.log().len(),
                stage.log().len() - 3
            )
            .as_str()
        )
    );
}

#[test]
fn while_paused_a_request_that_cannot_fit_is_not_sent_and_the_turn_ends_with_the_reason() {
    let mut stage = breaking(Some(PAUSE));
    one_turn_then_line(&mut stage);
    for words in ["a", "b", "c"] {
        failed_compaction(&mut stage, words);
    }
    // 放得下的这一轮报 300：窗口 210，下一轮用量加预留一定超。
    stage.model([Line::says("好。").reports(300)]);
    stage.say("d");
    let asked = stage.requests().len();
    let from = stage.log().len();
    stage.say("e");
    assert_eq!(stage.requests().len(), asked, "明知放不下，不发");
    let turn = from + 2;
    let lines = since(&stage, from);
    assert_eq!(
        lines[lines.len() - 2],
        format!(
            "{} model.called:error kernel t{turn}",
            stage.log().len() - 1
        )
    );
    assert_eq!(
        lines[lines.len() - 1],
        format!("{} turn.ended:error kernel t{turn}", stage.log().len())
    );
    let refused = stage.model_calls().last().copied().cloned().unwrap();
    assert_eq!(refused.endpoint, None);
    assert_eq!(refused.model, None);
    assert_eq!(refused.request, None);
    assert_eq!(refused.compaction, None);
    assert_eq!(refused.seen.get() as usize, stage.log().len() - 2);
    let error = refused.error.unwrap();
    assert_eq!(error.class, ErrorClass::CompactionPaused);
    assert!(
        error.message.starts_with("the request would not fit: ")
            && error
                .message
                .ends_with(" tokens used + 10 reserved for output > window 210"),
        "{}",
        error.message
    );
}

/// 暂停着（连续失败 3 次），下一轮报 `report`，再下一轮发不发：没发的，交回原话里的用量。
fn paused_then(report: u64) -> Option<u64> {
    let mut stage = breaking(Some(PAUSE));
    one_turn_then_line(&mut stage);
    for words in ["a", "b", "c"] {
        failed_compaction(&mut stage, words);
    }
    stage.model([Line::says("好。").reports(report)]);
    stage.say("d");
    stage.model([Line::says("好。")]);
    stage.say("e");
    let last = stage.model_calls().last().copied().cloned().unwrap();
    let error = last
        .error
        .filter(|error| error.class == ErrorClass::CompactionPaused)?;
    let used = error.message.strip_prefix("the request would not fit: ")?;
    used.split(' ').next()?.parse().ok()
}

#[test]
fn a_request_that_just_fits_the_window_still_goes_out() {
    // 用量是锚（报的）加锚以后新加的：先报一个一定放不下的，量出新加的有多少。
    let added = paused_then(300).expect("报 300 的放不下") - 300;
    // 用量加预留 10 正好是窗口 210：放得下，照发；多一个就不发。
    assert_eq!(paused_then(200 - added), None);
    assert_eq!(paused_then(201 - added), Some(201));
}

#[test]
fn a_successful_compaction_starts_the_count_again() {
    let mut stage = breaking(Some(PAUSE));
    one_turn_then_line(&mut stage);
    failed_compaction(&mut stage, "a");
    failed_compaction(&mut stage, "b");
    stage.model([Line::says("S1"), Line::says("好。")]);
    stage.say("c");
    failed_compaction(&mut stage, "d");
    failed_compaction(&mut stage, "e");
    // 那两轮她没真看到的话留在了尾巴里，失败也跟着在检查点后面，照样不算：只看写在压缩那一条前面还是后面。
    assert!(pauses(&stage).is_empty(), "压成了一次，前面的两次不算");
    failed_compaction(&mut stage, "f");
    assert_eq!(pauses(&stage).len(), 1);
}

#[test]
fn only_failed_automatic_compactions_that_end_the_turn_count() {
    let mut stage = breaking(Some(PAUSE));
    one_turn_then_line(&mut stage);
    failed_compaction(&mut stage, "a");
    // 摘要请求出了能再来的错，再来一次成了不再来的错：一轮只算一次。
    stage.model([
        Line::fails(ErrorClass::Retryable, "busy"),
        Line::fails(ErrorClass::Auth, "denied"),
    ]);
    stage.say("b");
    // 摘要请求被打断：不算。
    stage.model([Line::says("S").held()]);
    stage.say("c");
    stage.interrupt(Queued::Send);
    // 摘要请求出了能再来的错，等着再来时被打断：最后一条是出错的摘要请求，可这一轮不是出错结束的，不算。
    stage.hold_wakes();
    stage.model([Line::fails(ErrorClass::Retryable, "busy")]);
    stage.say("c2");
    stage.interrupt(Queued::Send);
    stage.unhold_wakes();
    // 没到线，主请求出错：不算。
    no_line(&mut stage);
    stage.model([Line::fails(ErrorClass::Auth, "denied")]);
    stage.say("d");
    line(&mut stage, 100);
    assert!(pauses(&stage).is_empty(), "只算了两次");
    failed_compaction(&mut stage, "e");
    assert_eq!(
        pauses(&stage),
        [&CompactionPaused {
            reason: PauseReason::Failures,
            failures: Some(3),
            entry: None,
        }]
    );
}

#[test]
fn without_the_numbers_in_the_snapshot_nothing_pauses() {
    let mut stage = breaking(None);
    one_turn_then_line(&mut stage);
    for words in ["a", "b", "c", "d"] {
        failed_compaction(&mut stage, words);
    }
    assert!(pauses(&stage).is_empty());
    assert_eq!(summaries(&stage), 4, "照旧每轮都压");
}

#[test]
fn undoing_the_turn_that_paused_resumes_automatic_compaction() {
    let mut stage = breaking(Some(PAUSE));
    one_turn_then_line(&mut stage);
    for words in ["a", "b", "c"] {
        failed_compaction(&mut stage, words);
    }
    let paused_turn = *stage.turns().last().unwrap();
    stage.revert(paused_turn);
    stage.model([Line::says("S1"), Line::says("好。")]);
    stage.say("d");
    assert_eq!(summaries(&stage), 4, "撤掉了暂停，又压了");
}

#[test]
fn after_a_restart_it_is_still_paused() {
    let mut stage = breaking(Some(PAUSE));
    one_turn_then_line(&mut stage);
    for words in ["a", "b", "c"] {
        failed_compaction(&mut stage, words);
    }
    stage.restart();
    line(&mut stage, 100);
    stage.model([Line::says("好。")]);
    stage.say("d");
    assert_eq!(summaries(&stage), 3, "载入以后照日志算，还暂停着");
}

/// 一轮到线压一次，压完照常答。
fn compacting_turn(stage: &mut Stage, words: &str) {
    stage.model([Line::says("S"), Line::says("好。")]);
    stage.say(words);
}

#[test]
fn compacting_again_within_three_turns_counts_up_and_the_third_time_pauses() {
    let mut stage = breaking(Some(PAUSE));
    one_turn_then_line(&mut stage);
    compacting_turn(&mut stage, "a");
    compacting_turn(&mut stage, "b");
    compacting_turn(&mut stage, "c");
    assert_eq!(refills(&stage), [None, Some(1), Some(2)]);
    assert!(pauses(&stage).is_empty());
    // 第四次：不压，写暂停，是估得最大的那一条；暂停着放得下，照发。
    let long = "x".repeat(200);
    let asked = stage.requests().len();
    let from = stage.log().len();
    stage.model([Line::says("好。")]);
    stage.say(&long);
    let turn = from + 2;
    assert_eq!(
        since(&stage, from),
        [
            format!("{} message.user alice", from + 1),
            format!("{turn} turn.started kernel t{turn}"),
            format!("{} context.compaction_paused kernel t{turn}", from + 3),
            format!("{} message.assistant model t{turn}", from + 4),
            format!("{} model.called:ok kernel t{turn}", from + 5),
            format!("{} turn.ended:completed kernel t{turn}", from + 6),
        ]
    );
    assert_eq!(
        pauses(&stage),
        [&CompactionPaused {
            reason: PauseReason::TooLarge,
            failures: None,
            entry: Some(seq(from as u64 + 1)),
        }]
    );
    assert_eq!(stage.requests().len(), asked + 1);
    assert_eq!(refills(&stage).len(), 3);
}

#[test]
fn of_two_equally_large_entries_the_pause_names_the_earlier() {
    let mut stage = breaking(Some(PAUSE));
    one_turn_then_line(&mut stage);
    compacting_turn(&mut stage, "a");
    compacting_turn(&mut stage, "b");
    // 触发第三次压缩的这句留在检查点后面，和第四轮的一样长。
    let long = "x".repeat(200);
    let earlier = stage.log().len() as u64 + 1;
    compacting_turn(&mut stage, &long);
    stage.model([Line::says("好。")]);
    stage.say(&long);
    assert_eq!(
        pauses(&stage).first().and_then(|paused| paused.entry),
        Some(seq(earlier))
    );
}

#[test]
fn the_third_turn_after_a_compaction_is_still_quick_and_the_fourth_is_not() {
    for (quiet, expected) in [(1, Some(1)), (2, None)] {
        let mut stage = breaking(Some(PAUSE));
        one_turn_then_line(&mut stage);
        compacting_turn(&mut stage, "a");
        // 中间几轮线高，不压；最后一轮再到线。
        no_line(&mut stage);
        for k in 0..quiet {
            stage.model([Line::says("好。")]);
            stage.say(&format!("quiet {k}"));
        }
        line(&mut stage, 100);
        compacting_turn(&mut stage, "b");
        assert_eq!(refills(&stage), [None, expected], "中间隔了 {quiet} 轮");
    }
}
