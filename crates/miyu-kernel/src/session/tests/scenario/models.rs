//! 场景：模型从配置里来以后内核多认的几样（`docs/blueprint/models.md`「事件」、「怎么走」第一条第 7 条、第五条，施工 8-6、
//! 8-9）。端口当场说完的 `no_model`：这一轮以出错结束，不再来，不推重试的状态。端口说换了端点（`failover`）的，不管分类
//! 当场再来、照它说的等，数进这一步的 5 次，推的状态带 `failover`；全在冷却的 `cooling` 照它说的等最早恢复的那一个，
//! 超过 2 分钟的不等。

use super::*;
use crate::event::{Body, Status, TransientBody};

/// 推过的重试状态。
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
fn no_model_ends_the_turn_without_retrying() {
    let mut stage = stage();
    stage.model([Line::fails(
        ErrorClass::NoModel,
        "no model configured: set models.chat",
    )]);
    stage.say("hi");
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 model.called:error kernel t3",
            "7 turn.ended:error kernel t3"
        ])
    );
    assert!(statuses(&stage).is_empty(), "不再来，不推重试");
    assert_eq!(stage.requests().len(), 1);
    let class = stage.log().iter().find_map(|event| match &event.body {
        Body::ModelCalled(called) => called.error.as_ref().map(|error| error.class.as_str()),
        _ => None,
    });
    assert_eq!(class, Some("no_model"), "照这个名字写进日志");
}

/// 推过的重试状态里要看的几格：分类、等多久、是不是换了端点。
fn retried(stage: &Stage) -> Vec<(ErrorClass, u64, bool)> {
    statuses(stage)
        .iter()
        .map(|status| {
            (
                status.retry.class.clone(),
                status.retry.wait_ms,
                status.retry.failover,
            )
        })
        .collect()
}

#[test]
fn a_failover_is_asked_again_at_once_whatever_the_class() {
    let mut stage = stage();
    stage.model([
        Line::fails(ErrorClass::Auth, "HTTP 401").fails_over(),
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
    assert_eq!(
        retried(&stage),
        [(ErrorClass::Auth, 0, true)],
        "认证失败本来不再来，换了端点的当场再来，状态带 failover"
    );
}

#[test]
fn a_failover_waits_as_long_as_the_port_says() {
    let mut stage = stage();
    stage.model([
        Line::fails(ErrorClass::RateLimited, "HTTP 429")
            .waits(3000)
            .fails_over(),
        Line::says("好。"),
    ]);
    stage.say("hi");
    assert_eq!(retried(&stage), [(ErrorClass::RateLimited, 3000, true)]);
}

#[test]
fn failovers_count_toward_the_five() {
    let mut stage = stage();
    stage.model(vec![
        Line::fails(ErrorClass::Auth, "HTTP 401").fails_over();
        6
    ]);
    stage.say("hi");
    assert_eq!(stage.requests().len(), 6, "一次，加上 5 次再来");
    let attempts: Vec<u32> = statuses(&stage)
        .iter()
        .map(|status| status.retry.attempt)
        .collect();
    assert_eq!(attempts, [1, 2, 3, 4, 5]);
    assert!(
        story(&stage)
            .last()
            .is_some_and(|line| line.contains("turn.ended:error"))
    );
}

#[test]
fn cooling_waits_for_the_earliest_and_asks_again() {
    let mut stage = stage();
    stage.model([
        Line::fails(
            ErrorClass::Cooling,
            "all candidates cooling: a/x key 1 rate_limited until 2026-10-01T08:12:30Z",
        )
        .waits(5000),
        Line::says("好。"),
    ]);
    stage.say("hi");
    assert_eq!(
        retried(&stage),
        [(ErrorClass::Cooling, 5000, false)],
        "照最早恢复的等，没换端点不写 failover"
    );
    assert!(
        story(&stage)
            .last()
            .is_some_and(|line| line.contains("turn.ended:completed"))
    );
    let class = stage.log().iter().find_map(|event| match &event.body {
        Body::ModelCalled(called) => called.error.as_ref().map(|error| error.class.as_str()),
        _ => None,
    });
    assert_eq!(class, Some("cooling"), "照这个名字写进日志");
}

#[test]
fn cooling_without_a_wait_backs_off() {
    let mut stage = stage();
    stage.model([
        Line::fails(ErrorClass::Cooling, "all candidates cooling"),
        Line::says("好。"),
    ]);
    stage.say("hi");
    assert_eq!(retried(&stage), [(ErrorClass::Cooling, 1000, false)]);
}

#[test]
fn cooling_longer_than_two_minutes_ends_the_turn() {
    let mut stage = stage();
    stage.model([Line::fails(ErrorClass::Cooling, "all candidates cooling").waits(600_000)]);
    stage.say("hi");
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 model.called:error kernel t3",
            "7 turn.ended:error kernel t3"
        ]),
        "要等的超过 2 分钟：不等，这一轮以出错结束"
    );
    assert!(statuses(&stage).is_empty());
}
