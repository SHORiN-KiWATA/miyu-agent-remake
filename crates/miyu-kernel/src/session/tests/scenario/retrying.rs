//! 场景：出了错再来（`02-内核.md` 第六节「回复怎么收」第 4 条，施工 3-5 下）。什么都没收到的，等一会儿
//! 再请求；说了一半断了的，半截写成回复、跟一句被打断的提示再请求；供应商说了等多久的照它；5 次都不行、
//! 不能重试的，这一轮以出错结束，半截照样留下；在等的时候被打断、要重启；重试不算步数。
//!
//! 替身的组装把有效历史一条一行列出来，`model.called` 也在里面，所以再来的那次请求比第一次多一行。
//! 真的组装里 `model.called` 不进上下文，什么都没收到的再来一字不差，见组装的探针。

use super::*;
use crate::block::Reasoning;
use crate::event::{CallError, Status, TransientBody};

/// 推过的重试状态，照先后。
fn statuses(stage: &Stage) -> Vec<&Status> {
    stage
        .transients()
        .iter()
        .filter_map(|transient| match &transient.body {
            TransientBody::Status(status) => Some(status),
            _ => None,
        })
        .collect()
}

#[test]
fn nothing_received_is_asked_again() {
    let mut stage = stage();
    stage.model([
        Line::fails(ErrorClass::Retryable, "503 Service Unavailable"),
        Line::says("好。"),
    ]);
    stage.say("hi");
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 model.called:error kernel t3",
            "7 message.assistant model t3",
            "8 model.called:ok kernel t3",
            "9 turn.ended:completed kernel t3",
        ])
    );
    let requests = stage.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        listed_request(&requests[1].1),
        listed_request(&requests[0].1) + "6 model.called\n"
    );
    let status = statuses(&stage);
    assert_eq!(status.len(), 1);
    assert_eq!(
        (
            status[0].seen,
            status[0].retry.attempt,
            status[0].retry.limit
        ),
        (seq(5), 1, 5)
    );
    assert_eq!(status[0].retry.wait_ms, 1000, "没说等多久的，第一次等 1 秒");
    assert_eq!(status[0].retry.class, ErrorClass::Retryable);
    assert_eq!(status[0].retry.message, "503 Service Unavailable");
}

#[test]
fn a_half_reply_is_kept_and_she_is_told_it_was_cut() {
    let mut stage = stage();
    stage.model([
        Line::breaks("我先看一下", ErrorClass::Retryable, "connection reset")
            .thinking("用户要看目录"),
        Line::says("目录里有 src 和 tests。"),
    ]);
    stage.say("看看目录");
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.assistant model t3",
            "7 model.called:error kernel t3",
            "8 context.injected:reply_cut kernel t3",
            "9 message.assistant model t3",
            "10 model.called:ok kernel t3",
            "11 turn.ended:completed kernel t3",
        ])
    );
    let Body::MessageAssistant(half) = &stage.log()[5].body else {
        panic!("6 号是半截回复");
    };
    assert!(half.interrupted);
    assert_eq!(
        half.blocks,
        [
            Block::Reasoning(Reasoning {
                text: "用户要看目录".to_string(),
                private: None,
            }),
            Block::Text(Text {
                text: "我先看一下".to_string(),
            }),
        ],
        "思考和正文照收到的留下"
    );
    let Body::ContextInjected(fact) = &stage.log()[7].body else {
        panic!("8 号是被打断的那一句");
    };
    assert_eq!(fact.text, "<reply-cut/>");
    // 再来的那次请求看到了半截回复和那一句。
    assert!(
        listed_request(&stage.requests()[1].1)
            .ends_with("6 message.assistant\n7 model.called\n8 context.injected\n")
    );
}

#[test]
fn a_half_written_call_is_dropped() {
    let mut stage = stage();
    let broken = Line {
        error: Some(CallError {
            class: ErrorClass::Retryable,
            message: "connection reset".to_string(),
        }),
        ..Line::calls("我读一下。", &[("read", r#"{"pa"#)])
    };
    stage.model([broken, Line::says("好了。")]);
    stage.say("读一下 a");
    let Body::MessageAssistant(half) = &stage.log()[5].body else {
        panic!("6 号是半截回复");
    };
    assert_eq!(
        half.blocks,
        [Block::Text(Text {
            text: "我读一下。".to_string(),
        })],
        "没收全的调用不留"
    );
    assert!(stage.ran().is_empty(), "也不派");
}

#[test]
fn the_provider_says_how_long_to_wait() {
    let mut stage = stage();
    stage.model([
        Line::fails(ErrorClass::RateLimited, "429").waits(3000),
        Line::says("好。"),
    ]);
    stage.say("hi");
    assert_eq!(statuses(&stage)[0].retry.wait_ms, 3000);
    // 说要等 10 分钟的：不等，这一轮以出错结束。
    let mut stage = stage_after_one();
    stage.model([Line::fails(ErrorClass::RateLimited, "429").waits(600_000)]);
    stage.say("再来");
    let told = story(&stage);
    assert!(
        told.ends_with(&[
            "11 model.called:error kernel t10".to_string(),
            "12 turn.ended:error kernel t10".to_string(),
        ]),
        "{told:#?}"
    );
    assert!(statuses(&stage).is_empty());
}

#[test]
fn five_retries_then_it_gives_up_but_keeps_the_half() {
    let mut stage = stage();
    let mut lines = vec![Line::fails(ErrorClass::Retryable, "503"); 5];
    lines.push(Line::breaks("最后说到这", ErrorClass::Retryable, "503"));
    stage.model(lines);
    stage.say("hi");
    assert_eq!(stage.requests().len(), 6, "一次，加上 5 次重试");
    let status = statuses(&stage);
    let waits: Vec<u64> = status.iter().map(|status| status.retry.wait_ms).collect();
    assert_eq!(waits, [1000, 2000, 4000, 8000, 16_000]);
    assert!(story(&stage).ends_with(&[
        "11 message.assistant model t3".to_string(),
        "12 model.called:error kernel t3".to_string(),
        "13 turn.ended:error kernel t3".to_string(),
    ]));
    let Body::MessageAssistant(half) = &stage.log()[10].body else {
        panic!("11 号是最后的半截回复");
    };
    assert!(half.interrupted, "不再来的半截也留下");
}

#[test]
fn an_error_that_cannot_help_is_not_retried() {
    let mut stage = stage();
    stage.model([Line::fails(ErrorClass::Auth, "401 Unauthorized")]);
    stage.say("hi");
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 model.called:error kernel t3",
            "7 turn.ended:error kernel t3"
        ])
    );
    assert!(statuses(&stage).is_empty());
}

#[test]
fn an_interrupt_while_waiting_ends_the_turn() {
    let mut stage = stage();
    stage.hold_wakes();
    stage.model([Line::fails(ErrorClass::Retryable, "503")]);
    stage.say("hi");
    stage.interrupt(Queued::Send);
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 model.called:error kernel t3",
            "7 turn.ended:interrupted alice t3",
        ])
    );
    // 扣着的到点了再送回来，也不理。
    stage.release_wake();
    assert_eq!(stage.requests().len(), 1);
}

#[test]
fn a_restart_while_waiting_picks_up_again() {
    let mut stage = stage();
    stage.hold_wakes();
    stage.model([
        Line::fails(ErrorClass::Retryable, "503"),
        Line::says("接着来。"),
    ]);
    stage.say("hi");
    stage.restart();
    assert_eq!(
        story(&stage)[5..8],
        [
            "6 model.called:error kernel t3",
            "7 turn.ended:restarted kernel t3",
            "8 turn.started kernel t8",
        ]
    );
    assert!(
        story(&stage)
            .last()
            .is_some_and(|line| line.contains("turn.ended:completed"))
    );
}

#[test]
fn a_switch_while_waiting_is_told_before_asking_again() {
    // 等着重试的时候切成了只读：到点了先记一条权限的事实，再请求，再来的那次看得到。
    let mut stage = stage();
    stage.hold_wakes();
    stage.model([
        Line::fails(ErrorClass::Retryable, "503"),
        Line::says("好。"),
    ]);
    stage.say("hi");
    stage.set_permission(None, Some(true));
    stage.release_wake();
    assert_eq!(
        story(&stage)[5..],
        [
            "6 model.called:error kernel t3",
            "7 session.policy_changed alice t3",
            "8 context.injected:permission kernel t3",
            "9 message.assistant model t3",
            "10 model.called:ok kernel t3",
            "11 turn.ended:completed kernel t3",
        ]
    );
    assert!(listed_request(&stage.requests()[1].1).ends_with("8 context.injected\n"));
}

#[test]
fn retries_do_not_count_as_steps() {
    // 一个回合最多请求两次模型：第一次出错重试了，调完工具照样还有第二步。步数上限在调完工具时查，
    // 重试要是算了步数，这里就走到了上限。
    let mut stage = limited_stage();
    stage.model([
        Line::fails(ErrorClass::Retryable, "503"),
        Line::calls("我看看。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("看完了。"),
    ]);
    stage.tools([Play::done("A")]);
    stage.say("看看 a");
    assert_eq!(stage.requests().len(), 3);
    assert!(
        story(&stage)
            .last()
            .is_some_and(|line| line.contains("turn.ended:completed"))
    );
}

#[test]
fn the_steps_after_a_retry_still_count() {
    // 最多请求两次：第一次出错重试了，这一次不算；往下走的第二步照样算，调完工具就到了上限。
    let mut stage = limited_stage();
    stage.model([
        Line::fails(ErrorClass::Retryable, "503"),
        Line::calls("我看看。", &[("read", r#"{"path":"a"}"#)]),
        Line::calls("再看看。", &[("read", r#"{"path":"b"}"#)]),
        Line::says("看完了。"),
    ]);
    stage.tools([Play::done("A"), Play::done("B")]);
    stage.say("看看 a 和 b");
    assert_eq!(stage.requests().len(), 3);
    assert!(
        story(&stage)
            .last()
            .is_some_and(|line| line.contains("turn.ended:step_limit"))
    );
}

#[test]
fn the_count_starts_over_after_a_reply() {
    // 连着出错 3 次，说完了一步；下一步又连着出错 3 次：说完一次就从头数，照样再来，走完这一轮。
    let mut stage = stage();
    let mut lines = vec![Line::fails(ErrorClass::Retryable, "503"); 3];
    lines.push(Line::calls("我看看。", &[("read", r#"{"path":"a"}"#)]));
    lines.extend(vec![Line::fails(ErrorClass::Retryable, "503"); 3]);
    lines.push(Line::says("看完了。"));
    stage.model(lines);
    stage.tools([Play::done("A")]);
    stage.say("看看 a");
    assert_eq!(stage.requests().len(), 8);
    assert!(
        story(&stage)
            .last()
            .is_some_and(|line| line.contains("turn.ended:completed"))
    );
    let attempts: Vec<u32> = statuses(&stage)
        .iter()
        .map(|status| status.retry.attempt)
        .collect();
    assert_eq!(attempts, [1, 2, 3, 1, 2, 3]);
}

/// 先说完一轮的替身：你的下一句是 9 号。
fn stage_after_one() -> Stage {
    let mut stage = stage();
    stage.model([Line::says("好。")]);
    stage.say("hi");
    stage
}
