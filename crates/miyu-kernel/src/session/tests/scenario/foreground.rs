//! 子代理前台跑（施工 T-1 下，`docs/blueprint/kernel/session.md`「调工具」第 7 条、「回报」第 5 条，设计 `30-插件框架.md`
//! 第三节第 7 条）：后台运行关着的会话（策略的 `foreground`）派出去的子代理，这一步等它报回来才齐、再请求下一次，报告排在
//! 这一步的工具结果后面；不叫醒、不另开轮；打断时停掉它，之后的回报只记下。后台的照旧（`reports.rs`）。

use super::reports::{CHILD, event, last_request};
use super::*;
use crate::event::{ChildReason, Effect};

/// 后台运行关着的会话。
fn foreground() -> Stage {
    Stage::new(
        || {
            let mut foreground = policy();
            foreground.foreground = true;
            foreground
        },
        environment("~/src/miyu"),
        at(0),
    )
}

/// 派出去一个子代理 `j1`（替身的工具面上叫 `shell`，内核不看名字，看效果）：前台的到 `tool.result` 为止，没有第二次请求。
fn waiting(mut s: Stage) -> Stage {
    s.model([
        Line::calls("派出去。", &[("shell", "{}")]),
        Line::says("看到报告了。"),
    ]);
    s.tools([Play::starts_agent(1, "查 CI", CHILD)]);
    s.say("查一下 CI");
    s
}

#[test]
fn a_foreground_step_waits_for_the_report_and_shows_it_after_the_results() {
    let mut s = waiting(foreground());
    assert_eq!(
        s.requests().len(),
        1,
        "等着它，不请求下一次：{:#?}",
        story(&s)
    );
    let last = s.log().last().expect("有事件");
    let Body::ToolResult(result) = &last.body else {
        panic!("停在工具结果上：{:#?}", story(&s));
    };
    assert!(
        matches!(&result.effects[..], [Effect::JobStarted(started)] if started.foreground),
        "记成前台的：{:?}",
        result.effects
    );
    let before = s.log().len();
    s.child_reports(1, CHILD, ChildReason::Done, "CI 红在 macOS。");
    assert_eq!(s.requests().len(), 2, "报回来了才请求：{:#?}", story(&s));
    assert!(
        !s.log()[before..]
            .iter()
            .any(|event| matches!(event.body, Body::TurnStarted(_))),
        "不另开轮：{:#?}",
        story(&s)
    );
    assert!(
        last_request(&s).contains("child.reported"),
        "报告在请求里：{}",
        last_request(&s)
    );
    assert!(matches!(
        s.log().last().map(|event| &event.body),
        Some(Body::TurnEnded(_))
    ));
}

#[test]
fn a_background_step_does_not_wait() {
    let s = waiting(stage());
    assert_eq!(s.requests().len(), 2, "后台的照旧接着请求");
    let Body::ToolResult(result) = &event(&s, 8).body else {
        panic!("{:#?}", story(&s));
    };
    assert!(
        matches!(&result.effects[..], [Effect::JobStarted(started)] if !started.foreground),
        "{:?}",
        result.effects
    );
}

#[test]
fn interrupting_stops_the_foreground_subagent_and_its_report_does_not_wake() {
    let mut s = waiting(foreground());
    s.interrupt(Queued::Return);
    assert!(
        matches!(
            s.stopping(),
            [Action::StopJobs { jobs, .. }] if jobs.iter().map(ToString::to_string).eq(["j1"])
        ),
        "停掉它：{:?}",
        s.stopping()
    );
    assert!(matches!(
        s.log().last().map(|event| &event.body),
        Some(Body::TurnEnded(_))
    ));
    let before = s.log().len();
    s.child_reports(1, CHILD, ChildReason::Stopped, "停在一半。");
    assert_eq!(s.log().len(), before + 1, "只记下：{:#?}", story(&s));
    assert_eq!(s.requests().len(), 1);
}

#[test]
fn a_message_to_a_foreground_subagent_is_waited_for_again() {
    let mut s = foreground();
    s.model([
        Line::calls("派出去。", &[("shell", "{}")]),
        Line::calls("答它。", &[("shell", "{}")]),
        Line::says("做完了。"),
    ]);
    s.tools([Play::starts_agent(1, "查 CI", CHILD), Play::messages(1)]);
    s.say("查一下 CI");
    s.child_reports(1, CHILD, ChildReason::Done, "要用哪个分支？");
    assert_eq!(s.requests().len(), 2, "留了言，等它再报：{:#?}", story(&s));
    s.child_reports(1, CHILD, ChildReason::Done, "main 上的 CI 红在 macOS。");
    assert_eq!(s.requests().len(), 3, "{:#?}", story(&s));
    assert!(matches!(
        s.log().last().map(|event| &event.body),
        Some(Body::TurnEnded(_))
    ));
}

/// 前台的回报载入以后也不记在一边（`jobs::waking`）：撤销着的时候到的，崩了重载、恢复了撤销，不由它开一轮。
#[test]
fn a_foreground_report_is_not_set_aside_across_a_reload() {
    let mut s = waiting(foreground());
    s.interrupt(Queued::Return);
    s.model([Line::says("好。")]);
    s.say("再来");
    let second = *s.turns().last().expect("有第二轮");
    s.revert(second);
    s.child_reports(1, CHILD, ChildReason::Stopped, "停在一半。");
    s.crash();
    let turns = s.turns().len();
    s.unrevert();
    assert_eq!(s.turns().len(), turns, "{:#?}", story(&s));
}
