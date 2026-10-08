//! 场景：压缩（`docs/blueprint/compaction.md` 第二、三条，施工 6-2 上）。发主请求之前用量过了压缩线，先发一次摘要
//! 请求：替身的组装把有效历史到第 N 条列出来，最后一行 `summarize`；说完了写压缩，再照检查点以后的发主请求。
//!
//! 压缩线 = 窗口 − 10 − 10（替身的策略：输出预留的上限 10、余量 10，没报最大输出）。替身的组装一条事件约五个
//! token，说完了的请求默认报 110：有锚的请求一般都过 100 的线，没锚的（刚开头、刚压完）一般不过。

use super::*;
use crate::event::{CompactTrigger, CompactionProgress, TransientBody, Usage};
use crate::id::CallId;
use crate::session::Compaction;

/// 压缩的数：输出预留的上限、余量各 10，图片、文件各 50。尾巴的预算是 0：这几个场景只看这一轮要回应的话，尾巴在
/// `scenario/tail.rs`。
const COMPACTION: Compaction = Compaction {
    reserve_cap: 10,
    margin: 10,
    tail: 0,
    lead: 0,
    price: crate::estimate::Flat {
        image: 50,
        file: 50,
    },
    rebuild: None,
    pause: None,
    shorten: None,
    isolate: false,
};

/// 一个会压缩的替身；`step_limit` 是一个回合最多请求几次。
pub(super) fn compacting(step_limit: Option<u32>) -> Stage {
    let make = move || {
        let mut policy = policy();
        policy.step_limit = step_limit;
        policy.compaction = Some(COMPACTION);
        policy
    };
    Stage::new(make, environment("~/src/miyu"), at(0))
}

/// 交限额，压缩线正好是 `line`。
pub(super) fn line(stage: &mut Stage, line: u64) {
    stage.limits(Some(line + 20), None);
}

/// 第 `k` 次请求的清单。
fn request(stage: &Stage, k: usize) -> (u64, String) {
    let (seen, request) = &stage.requests()[k];
    (seen.get(), listed_request(request))
}

/// 推过的进度，照先后。
fn progress(stage: &Stage) -> Vec<&CompactionProgress> {
    stage
        .transients()
        .iter()
        .filter_map(|transient| match &transient.body {
            TransientBody::CompactionProgress(progress) => Some(progress),
            _ => None,
        })
        .collect()
}

/// 第一轮说一句就完：八条。
const FIRST_TURN: [&str; 3] = [
    "6 message.assistant model t3",
    "7 model.called:ok kernel t3",
    "8 turn.ended:completed kernel t3",
];

/// 第一轮说一句就完，再交压缩线 100：有了锚（报 110），第二轮一开头就过线。
fn after_one_turn(stage: &mut Stage) {
    stage.model([Line::says("好。")]);
    stage.say("hi");
    assert_eq!(story(stage), opening_then(&FIRST_TURN));
    line(stage, 100);
}

#[test]
fn over_the_line_at_the_start_of_a_turn_she_summarizes_first() {
    let mut stage = compacting(None);
    after_one_turn(&mut stage);
    stage.model([Line::says("S1"), Line::says("嗯。")]);
    stage.say("再说一句");
    let mut expected = opening_then(&FIRST_TURN);
    expected.extend(
        [
            "9 message.user alice",
            "10 turn.started kernel t10",
            "11 model.called:ok kernel t10",
            "12 context.compacted kernel t10",
            "13 context.injected:env kernel t10",
            "14 context.injected:permission kernel t10",
            "15 message.assistant model t10",
            "16 model.called:ok kernel t10",
            "17 turn.ended:completed kernel t10",
        ]
        .map(str::to_string),
    );
    assert_eq!(story(&stage), expected);
    // 摘要请求替代到触发这一轮的那句前面：日志到 8，最后一行是指令。
    let (seen, summary) = request(&stage, 1);
    assert_eq!(seen, 8);
    assert_eq!(summary, listing(&stage.log()[..8]) + "summarize\n");
    // 压完的主请求：检查点以后的，触发这一轮的那句在里面；替身的组装不列检查点本身。
    let (seen, main) = request(&stage, 2);
    assert_eq!(seen, 14);
    assert_eq!(
        main,
        "9 message.user\n10 turn.started\n11 model.called\n13 context.injected\n14 context.injected\n"
    );
    let Body::ContextCompacted(compacted) = &stage.log()[11].body else {
        panic!("12 号是压缩");
    };
    assert_eq!(compacted.upto, seq(8));
    assert_eq!(compacted.summary, "S1");
    assert_eq!(compacted.trigger, Some(CompactTrigger::Auto));
    assert_eq!(stage.log()[11].cause, Some(id(2)), "cause 是这一轮的");
    // 摘要请求的 model.called 看到的是 N；压完的第一次主请求，前缀从头变了。
    let calls = stage.model_calls();
    assert_eq!(calls[1].seen, seq(8));
    assert!(calls[2].first_difference.is_some());
}

#[test]
fn over_the_line_in_the_middle_of_a_turn_it_is_compacted_up_to_the_last_event() {
    let mut stage = compacting(None);
    line(&mut stage, 100);
    stage.model([
        Line::calls("", &[("read", "{}")]),
        Line::says("S1"),
        Line::says("看完了。").reports(10),
    ]);
    stage.tools([Play::Done("lib.rs".to_string())]);
    stage.say("看看");
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 tool.result:ok tool call_6_1 t3",
            "9 model.called:ok kernel t3",
            "10 context.compacted kernel t3",
            "11 context.injected:env kernel t3",
            "12 context.injected:permission kernel t3",
            "13 message.assistant model t3",
            "14 model.called:ok kernel t3",
            "15 turn.ended:completed kernel t3",
        ])
    );
    // 第一次没锚，不过线；有了锚（报 110）过线，替代到最后一条。
    let (seen, summary) = request(&stage, 1);
    assert_eq!(seen, 8);
    assert_eq!(summary, listing(&stage.log()[..8]) + "summarize\n");
    // 压完了，环境、权限两块事实比不到，再注入一次，排在检查点后面。
    let (_, main) = request(&stage, 2);
    assert_eq!(
        main,
        "9 model.called\n11 context.injected\n12 context.injected\n"
    );
}

#[test]
fn a_message_queued_in_the_middle_of_a_turn_stays_after_the_checkpoint() {
    let mut stage = compacting(None);
    line(&mut stage, 100);
    stage.model([
        Line::calls("", &[("read", "{}")]),
        Line::says("S1"),
        Line::says("好。").reports(10),
    ]);
    stage.tools([Play::Done("lib.rs".to_string()).held()]);
    stage.say("看看");
    let call = stage.log()[5].seq;
    stage.say("顺便看看 tests");
    stage.release_tool(CallId::new(call, 1).unwrap());
    // 排着的那句是 8，在 6 号回复和它的结果 9 之间：一组不拆，退到回复前面。
    let (seen, _) = request(&stage, 1);
    assert_eq!(seen, 5);
    let (_, main) = request(&stage, 2);
    assert!(
        main.starts_with("6 message.assistant\n7 model.called\n8 message.user\n9 tool.result\n"),
        "回复、它的结果、排着的那句都在检查点后面：{main}"
    );
}

#[test]
fn a_turn_whose_request_failed_without_a_reply_still_keeps_its_trigger() {
    let mut stage = compacting(None);
    after_one_turn(&mut stage);
    // 线放宽，第二轮第一次请求不压；它出错了，等着再来的时候线收紧。
    line(&mut stage, 1000);
    stage.hold_wakes();
    stage.model([
        Line::fails(ErrorClass::Retryable, "503"),
        Line::says("S1"),
        Line::says("嗯。"),
    ]);
    stage.say("再说一句");
    line(&mut stage, 100);
    stage.release_wake();
    // 那次出错的请求看到过触发的那句，可是没回复：照回合开头算，替代到它前面。
    let (seen, _) = request(&stage, 2);
    assert_eq!(seen, 8);
    let (_, main) = request(&stage, 3);
    assert!(main.starts_with("9 message.user\n"), "{main}");
}

#[test]
fn nothing_to_compress_before_the_first_message_is_not_compressed() {
    let mut stage = compacting(None);
    line(&mut stage, 1);
    stage.model([Line::says("好。")]);
    stage.say("hi");
    assert_eq!(story(&stage), opening_then(&FIRST_TURN));
    assert_eq!(stage.requests().len(), 1);
}

#[test]
fn once_compacted_it_does_not_compact_again_when_nothing_new_came() {
    let mut stage = compacting(None);
    after_one_turn(&mut stage);
    // 线低到压完也还在线上：替代到的还是触发前那一条，不再压，照发。
    line(&mut stage, 1);
    stage.model([Line::says("S1"), Line::says("嗯。")]);
    stage.say("再说一句");
    let summaries = stage
        .requests()
        .iter()
        .filter(|(_, request)| listed_request(request).ends_with("summarize\n"))
        .count();
    assert_eq!(summaries, 1);
    assert_eq!(stage.requests().len(), 3);
}

#[test]
fn it_is_not_compacted_without_limits_a_window_or_the_numbers() {
    // 没交限额。
    let mut stage = compacting(None);
    stage.model([Line::says("好。"), Line::says("嗯。")]);
    stage.say("hi");
    stage.say("再说一句");
    assert_eq!(stage.requests().len(), 2);
    // 交了，没有窗口。
    let mut stage = compacting(None);
    stage.limits(None, Some(5));
    stage.model([Line::says("好。"), Line::says("嗯。")]);
    stage.say("hi");
    stage.say("再说一句");
    assert_eq!(stage.requests().len(), 2);
    // 窗口小到算不出线。
    let mut stage = compacting(None);
    stage.limits(Some(20), None);
    stage.model([Line::says("好。"), Line::says("嗯。")]);
    stage.say("hi");
    stage.say("再说一句");
    assert_eq!(stage.requests().len(), 2);
    // 策略里没有压缩的数。
    let mut stage = scenario_stage();
    stage.limits(Some(120), None);
    stage.model([Line::says("好。"), Line::says("嗯。")]);
    stage.say("hi");
    stage.say("再说一句");
    assert_eq!(stage.requests().len(), 2);
}

/// 照 [`policy`] 造的替身，策略里没有压缩的数。
fn scenario_stage() -> Stage {
    super::stage()
}

#[test]
fn exactly_on_the_line_is_not_over_it() {
    // 先量出第二轮第一次请求的用量：锚是第一轮报的。
    let mut probe = compacting(None);
    after_one_turn(&mut probe);
    line(&mut probe, 100_000);
    probe.model([Line::says("嗯。")]);
    probe.say("再说一句");
    let used = used(&probe, 1);
    // 线正好是它，不压；比它少一，压。
    for (at_line, compacted) in [(used, false), (used - 1, true)] {
        let mut stage = compacting(None);
        stage.model([Line::says("好。")]);
        stage.say("hi");
        line(&mut stage, at_line);
        let lines = match compacted {
            true => vec![Line::says("S1"), Line::says("嗯。")],
            false => vec![Line::says("嗯。")],
        };
        stage.model(lines);
        stage.say("再说一句");
        assert_eq!(
            stage.requests().len(),
            if compacted { 3 } else { 2 },
            "线 {at_line}"
        );
    }
}

/// 第 `k` 次请求算出的用量：照那一刻的日志挑锚，和内核的算法一样。
fn used(stage: &Stage, k: usize) -> u64 {
    let (seen, request) = &stage.requests()[k];
    let mut history = History::default();
    for event in stage.log().iter().filter(|event| event.seq <= *seen) {
        history.append(event.clone());
    }
    let anchor = crate::estimate::anchor(&history);
    crate::estimate::usage(
        request,
        anchor.as_ref(),
        &crate::testkit::model(),
        &COMPACTION.price,
    )
}

#[test]
fn the_summary_request_does_not_count_as_a_step() {
    let mut stage = compacting(Some(3));
    line(&mut stage, 100);
    stage.model([
        Line::calls("", &[("read", "{}")]),
        Line::says("S1"),
        Line::calls("", &[("read", "{}")]).reports(10),
        Line::says("看完了。").reports(10),
    ]);
    stage.tools([
        Play::Done("lib.rs".to_string()),
        Play::Done("main.rs".to_string()),
    ]);
    stage.say("看看");
    let ended = stage.log().last().map(told);
    assert_eq!(
        ended.as_deref(),
        Some("18 turn.ended:completed kernel t3"),
        "三步里的摘要请求不算：{:#?}",
        story(&stage)
    );
}

#[test]
fn progress_is_pushed_instead_of_the_summary_text() {
    let mut stage = compacting(None);
    after_one_turn(&mut stage);
    stage.model([Line::says("摘要").thinking("先想想"), Line::says("嗯。")]);
    stage.say("再说一句");
    // 替身一个字一段增量：思考不数，正文两段两个字。
    let pushed: Vec<(u64, u64, u64)> = progress(&stage)
        .iter()
        .map(|p| (p.seen.get(), p.written, p.expected))
        .collect();
    // 发出去时先一条 0 字的（施工 6-3 下）。
    assert_eq!(pushed, [(8, 0, 20_000), (8, 2, 20_000)]);
    let deltas = stage
        .transients()
        .iter()
        .filter(|transient| {
            matches!(&transient.body, TransientBody::ModelDelta(delta) if delta.seen == seq(8))
        })
        .count();
    assert_eq!(deltas, 0, "摘要请求不推增量");
}

#[test]
fn the_expected_length_is_the_usage_kept_between_the_bounds() {
    // 用量是报的加上锚以后新增的几条：五万出头的照原样，九万的夹到八万。
    for (reported, expected) in [(90_000, 80_000..=80_000), (50_000, 50_001..=50_100)] {
        let mut stage = compacting(None);
        stage.model([Line::says("好。").reports(reported)]);
        stage.say("hi");
        line(&mut stage, 100);
        stage.model([Line::says("S1"), Line::says("嗯。")]);
        stage.say("再说一句");
        let p = progress(&stage);
        assert!(!p.is_empty());
        assert!(
            p.iter().all(|p| expected.contains(&p.expected)),
            "报 {reported}：{:?}",
            p.iter().map(|p| p.expected).collect::<Vec<_>>()
        );
    }
}

#[test]
fn a_summary_that_calls_a_tool_or_says_nothing_usable_ends_the_turn() {
    for line_of_summary in [
        Line::calls("S1", &[("read", "{}")]),
        Line::says("").thinking("只想了想"),
        Line::says("   "),
    ] {
        let mut stage = compacting(None);
        after_one_turn(&mut stage);
        stage.model([line_of_summary.clone()]);
        stage.say("再说一句");
        let tail: Vec<String> = story(&stage)[8..].to_vec();
        assert_eq!(
            tail,
            [
                "9 message.user alice",
                "10 turn.started kernel t10",
                "11 model.called:error kernel t10",
                "12 turn.ended:error kernel t10",
            ],
            "{line_of_summary:?}"
        );
        let calls = stage.model_calls();
        let error = calls[1].error.as_ref().unwrap();
        assert_eq!(error.class, ErrorClass::BadSummary, "{line_of_summary:?}");
    }
}

#[test]
fn a_retryable_error_asks_for_the_summary_again_and_the_next_request_is_a_step() {
    let mut stage = compacting(Some(2));
    after_one_turn(&mut stage);
    stage.model([
        Line::fails(ErrorClass::Retryable, "503"),
        Line::says("S1"),
        Line::calls("", &[("read", "{}")]).reports(10),
        Line::calls("", &[("read", "{}")]).reports(10),
    ]);
    stage.tools([
        Play::Done("lib.rs".to_string()),
        Play::Done("main.rs".to_string()),
    ]);
    stage.say("再说一句");
    let seens: Vec<u64> = stage
        .requests()
        .iter()
        .map(|(seen, _)| seen.get())
        .collect();
    // 摘要请求再来还替代到 8；压完的两次主请求各算一步，两步用完，这一轮到上限。
    assert_eq!(seens[1..], [8, 8, 15, 18]);
    assert_eq!(
        stage.log().last().map(told).as_deref(),
        Some("22 turn.ended:step_limit kernel t10"),
        "{:#?}",
        story(&stage)
    );
}

#[test]
fn interrupting_the_summary_writes_no_compaction() {
    let mut stage = compacting(None);
    after_one_turn(&mut stage);
    stage.model([Line::says("S1").held()]);
    stage.say("再说一句");
    stage.interrupt(Queued::Send);
    let tail: Vec<String> = story(&stage)[8..].to_vec();
    assert_eq!(
        tail,
        [
            "9 message.user alice",
            "10 turn.started kernel t10",
            "11 model.called:interrupted kernel t10",
            "12 turn.ended:interrupted alice t10",
        ]
    );
}

#[test]
fn a_restart_during_the_summary_ends_the_turn_as_restarted() {
    let mut stage = compacting(None);
    after_one_turn(&mut stage);
    stage.model([
        Line::says("S1").held(),
        Line::says("S1"),
        Line::says("嗯。"),
    ]);
    stage.say("再说一句");
    stage.restart();
    let told: Vec<String> = story(&stage);
    assert!(told.contains(&"11 model.called:interrupted kernel t10".to_string()));
    assert!(told.contains(&"12 turn.ended:restarted kernel t10".to_string()));
    // 再起来接着干的那一轮，照样先压（限额再交过一次）。
    assert!(
        told.iter()
            .any(|line| line.ends_with("context.compacted kernel t13")),
        "{told:#?}"
    );
}

#[test]
fn reported_usage_reads_the_same_as_the_kernel_uses_it() {
    // 替身报的用量四格加起来就是锚：没命中的、读缓存、写缓存、输出。
    let usage = Usage {
        uncached: 1,
        cache_read: 2,
        cache_write: 3,
        output: 4,
    };
    let line = Line::says("x").reports(10);
    assert_eq!(line.usage.map(|u| u.uncached), Some(10));
    assert_eq!(
        usage.uncached + usage.cache_read + usage.cache_write + usage.output,
        10
    );
}
