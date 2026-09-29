//! 场景：隔离式回退（`docs/blueprint/compaction.md` 第三条第 7 条、第四条，施工 6-6 下）。fork 式的摘要回复里调了工具，
//! 这一次作废，落了盘改发隔离式：替身的组装里一样的清单，system 写着「isolated」。隔离式一次压缩只改走一次。
//!
//! 第一轮说一句就完（报 110，有了锚），交压缩线 100，第二轮一开头就压，替代到 8。

use super::*;
use crate::event::CallResult;
use crate::session::{Compaction, Shorten};

/// 一个会压缩的替身；`isolate` 是快照里有没有隔离式那句 system，`shorten` 是截短的数。
fn isolating(isolate: bool, shorten: Option<Shorten>) -> Stage {
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
            pause: None,
            shorten,
            isolate,
        });
        policy
    };
    Stage::new(make, environment("~/src/miyu"), at(0))
}

/// 第一轮说一句就完，再交压缩线 100。
fn one_turn_then_line(stage: &mut Stage) {
    stage.model([Line::says("好。")]);
    stage.say("hi");
    stage.limits(Some(120), None);
}

/// 摘要回复里调了工具。
fn calls_a_tool() -> Line {
    Line::calls("", &[("read", "{}")])
}

/// 摘要请求，照先后：替代到哪、是不是隔离式、清单。
fn summaries(stage: &Stage) -> Vec<(u64, bool, String)> {
    stage
        .requests()
        .iter()
        .filter(|(_, request)| listed_request(request).ends_with("summarize\n"))
        .map(|(seen, request)| {
            (
                seen.get(),
                request.system == "isolated",
                listed_request(request),
            )
        })
        .collect()
}

/// 摘要请求的记录：结果和原话。
fn summary_calls(stage: &Stage) -> Vec<(CallResult, Option<String>)> {
    stage
        .model_calls()
        .into_iter()
        .filter(|called| called.compaction.is_some())
        .map(|called| {
            (
                called.result.clone(),
                called.error.as_ref().map(|error| error.message.clone()),
            )
        })
        .collect()
}

#[test]
fn a_tool_call_in_the_summary_goes_again_isolated() {
    let mut stage = isolating(true, None);
    one_turn_then_line(&mut stage);
    stage.model([calls_a_tool(), Line::says("S"), Line::says("嗯。")]);
    stage.say("再说");
    let summaries = summaries(&stage);
    assert_eq!(
        summaries
            .iter()
            .map(|(seen, isolated, _)| (*seen, *isolated))
            .collect::<Vec<_>>(),
        [(8, false), (8, true)]
    );
    // 隔离式的清单和 fork 式的一样：只换了 system（替身的清单里没有工具面）。
    assert_eq!(summaries[0].2, summaries[1].2);
    assert_eq!(
        summary_calls(&stage),
        [
            (
                CallResult::Error,
                Some("the summary reply called a tool; trying again without tools".to_string())
            ),
            (CallResult::Ok, None),
        ]
    );
    let compacted = stage
        .log()
        .iter()
        .find(|event| matches!(event.body, Body::ContextCompacted(_)));
    assert!(compacted.is_some(), "隔离式取到了摘要，照常压");
    assert_eq!(
        story(&stage).last().map(String::as_str),
        Some(format!("{} turn.ended:completed kernel t10", stage.log().len()).as_str())
    );
}

#[test]
fn it_goes_isolated_only_once() {
    let mut stage = isolating(true, None);
    one_turn_then_line(&mut stage);
    // 隔离式里又冒出了工具调用：不再改走，照失败算。
    stage.model([calls_a_tool(), calls_a_tool()]);
    stage.say("再说");
    assert_eq!(summaries(&stage).len(), 2);
    assert_eq!(
        summary_calls(&stage),
        [
            (
                CallResult::Error,
                Some("the summary reply called a tool; trying again without tools".to_string())
            ),
            (
                CallResult::Error,
                Some("the summary reply called a tool".to_string())
            ),
        ]
    );
    assert_eq!(
        story(&stage).last().map(String::as_str),
        Some(format!("{} turn.ended:error kernel t10", stage.log().len()).as_str())
    );
}

#[test]
fn without_the_system_line_in_the_snapshot_a_tool_call_just_fails() {
    let mut stage = isolating(false, None);
    one_turn_then_line(&mut stage);
    stage.model([calls_a_tool()]);
    stage.say("再说");
    assert_eq!(summaries(&stage).len(), 1);
    assert_eq!(
        summary_calls(&stage),
        [(
            CallResult::Error,
            Some("the summary reply called a tool".to_string())
        )]
    );
    assert_eq!(
        story(&stage).last().map(String::as_str),
        Some(format!("{} turn.ended:error kernel t10", stage.log().len()).as_str())
    );
}

#[test]
fn an_isolated_request_that_is_too_long_is_shortened_and_stays_isolated() {
    let shorten = Shorten {
        tries: 3,
        percent: 20,
    };
    let mut stage = isolating(true, Some(shorten));
    one_turn_then_line(&mut stage);
    stage.model([
        calls_a_tool(),
        Line::fails(ErrorClass::ContextTooLong, "413"),
        Line::says("S"),
        Line::says("嗯。"),
    ]);
    stage.say("再说");
    let summaries = summaries(&stage);
    assert_eq!(
        summaries
            .iter()
            .map(|(seen, isolated, listed)| (
                *seen,
                *isolated,
                listed.starts_with("truncated after ")
            ))
            .collect::<Vec<_>>(),
        [(8, false, false), (8, true, false), (8, true, true)],
        "截短以后还是隔离式"
    );
}

#[test]
fn a_shortened_request_that_calls_a_tool_goes_isolated_with_the_same_cut() {
    let shorten = Shorten {
        tries: 3,
        percent: 20,
    };
    let mut stage = isolating(true, Some(shorten));
    one_turn_then_line(&mut stage);
    stage.model([
        Line::fails(ErrorClass::ContextTooLong, "413"),
        calls_a_tool(),
        Line::says("S"),
        Line::says("嗯。"),
    ]);
    stage.say("再说");
    let summaries = summaries(&stage);
    assert_eq!(summaries.len(), 3);
    assert!(!summaries[0].1 && !summaries[1].1 && summaries[2].1);
    assert_eq!(summaries[1].2, summaries[2].2, "改走隔离式照截过的那一份");
    assert!(summaries[2].2.starts_with("truncated after "));
}

#[test]
fn a_reply_without_a_summary_is_not_a_reason_to_go_isolated() {
    let mut stage = isolating(true, None);
    one_turn_then_line(&mut stage);
    // 说了，可是只有空白：取不出摘要，不是调了工具，照失败算。
    stage.model([Line::says("   ")]);
    stage.say("再说");
    assert_eq!(summaries(&stage).len(), 1);
    assert_eq!(
        summary_calls(&stage),
        [(
            CallResult::Error,
            Some("no summary in the reply".to_string())
        )]
    );
}
