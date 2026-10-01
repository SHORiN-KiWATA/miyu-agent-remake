//! 场景：模型从配置里来以后内核多认的几样（`docs/blueprint/models.md`「事件」、「怎么走」第一条第 7 条、第五条、第六条，
//! 施工 8-6、8-9、8-10）。端口当场说完的 `no_model`：这一轮以出错结束，不再来，不推重试的状态。端口说换了端点
//! （`failover`）的，不管分类当场再来、照它说的等，数进这一步的 5 次，推的状态带 `failover`；全在冷却的 `cooling` 照它说的
//! 等最早恢复的那一个，超过 2 分钟的不等。换模型（施工 8-10）：一样的不记，不一样的记一条、下一个回合开始时交给执行器，
//! 回合进行中换的下一轮才交，撤掉的回合里换的也算；执行器退回了默认的，记在注入前面，`models.chat` 也不行的不记。

use super::super::executor::injection;
use super::*;
use crate::event::{Body, PolicyChanged, Status, TransientBody};

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

/// 换模型的那一条：换成的、原来的。
fn changed(event: &Event) -> (Option<&str>, Option<&str>) {
    match &event.body {
        Body::PolicyChanged(PolicyChanged {
            model, replaced, ..
        }) => (model.as_deref(), replaced.as_deref()),
        body => panic!("应该是 session.policy_changed：{body:?}"),
    }
}

/// 回合开始时交出去的引用，写成字（没有的写 `-`）。
fn asked(stage: &Stage) -> Vec<&str> {
    stage
        .asked_models()
        .iter()
        .map(|model| model.as_deref().unwrap_or("-"))
        .collect()
}

#[test]
fn a_change_is_recorded_once_and_handed_over_when_the_next_turn_starts() {
    let mut stage = stage();
    stage.model([Line::says("好。"), Line::says("好。")]);
    stage.say("hi");
    let from = stage.log().len();
    let id = stage.configure("@free");
    let event = &stage.log()[from];
    assert_eq!(
        told(event),
        format!("{} session.policy_changed alice", from + 1)
    );
    assert_eq!(changed(event), (Some("@free"), None));
    assert_eq!(event.cause.as_ref(), Some(&id), "cause 是那条命令");
    assert_eq!(
        stage.outcome(&id),
        Some(&Outcome::Accepted {
            events: seqs(&[from as u64 + 1])
        })
    );
    // 换成一样的：接受，什么都不记。
    let again = stage.configure("@free");
    assert_eq!(stage.log().len(), from + 1, "一样的不记");
    assert_eq!(
        stage.outcome(&again),
        Some(&Outcome::Accepted { events: Vec::new() })
    );
    stage.say("再来");
    assert_eq!(asked(&stage), ["-", "@free"], "下一个回合开始时交出去");
    assert_eq!(stage.requests().len(), 2, "换模型不请求模型");
}

#[test]
fn a_change_during_a_turn_waits_for_the_next_one() {
    let mut stage = stage();
    stage.model([Line::says("好。").held(), Line::says("好。")]);
    stage.say("hi");
    let id = stage.configure("b/n");
    let event = stage.log().last().unwrap();
    assert_eq!(
        told(event),
        "6 session.policy_changed alice t3",
        "带上这个回合"
    );
    assert_eq!(event.cause.as_ref(), Some(&id));
    assert_eq!(asked(&stage), ["-"], "这一轮照开始时的");
    stage.release_model();
    stage.say("再来");
    assert_eq!(asked(&stage), ["-", "b/n"]);
}

#[test]
fn a_change_in_an_undone_turn_still_counts_and_survives_a_reload() {
    let mut stage = stage();
    stage.model([
        Line::says("好。").held(),
        Line::says("好。"),
        Line::says("好。"),
    ]);
    stage.say("hi");
    stage.configure("b/n");
    stage.release_model();
    stage.revert(TurnId::new(seq(3)));
    stage.say("再来");
    assert_eq!(
        asked(&stage),
        ["-", "b/n"],
        "换模型不是对话的一部分，撤掉的回合里换的也算"
    );
    stage.crash();
    stage.say("又来");
    assert_eq!(asked(&stage), ["-", "b/n", "b/n"], "载入照日志算回来");
}

#[test]
fn a_gone_reference_falls_back_before_the_injections() {
    let mut stage = stage();
    stage.configure("@free");
    stage.gone("@free", Some("a/m"));
    stage.hooks([vec![injection("memory", "<m/>")]]);
    stage.model([Line::says("好。"), Line::says("好。")]);
    stage.say("hi");
    let lines = story(&stage);
    assert_eq!(
        lines[4..8],
        [
            "5 context.injected:env kernel t4",
            "6 context.injected:permission kernel t4",
            "7 session.policy_changed kernel t4",
            "8 context.injected:memory memory t4",
        ],
        "退回的那一条记在注入前面"
    );
    let fallback = &stage.log()[6];
    assert_eq!(changed(fallback), (Some("a/m"), Some("@free")));
    assert_eq!(fallback.cause, stage.log()[3].cause, "cause 是回合的");
    stage.say("再来");
    assert_eq!(asked(&stage), ["@free", "a/m"], "以后钉在退回的上面");
}

#[test]
fn nothing_is_recorded_when_models_chat_is_gone_too() {
    let mut stage = stage();
    stage.configure("@free");
    stage.gone("@free", None);
    stage.model([Line::fails(
        ErrorClass::NoModel,
        "no model configured: set models.chat",
    )]);
    stage.say("hi");
    assert!(
        !stage
            .log()
            .iter()
            .any(|event| matches!(&event.body, Body::PolicyChanged(changed) if changed.replaced.is_some())),
        "不记"
    );
    assert!(
        story(&stage)
            .last()
            .is_some_and(|line| line.contains("turn.ended:error")),
        "这一轮的请求当场 no_model"
    );
    assert_eq!(stage.reference(), Some("@free"), "引用照旧");
}
