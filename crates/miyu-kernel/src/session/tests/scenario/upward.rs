//! 向上回报（施工 7-6，`docs/blueprint/kernel/session.md`「向上回报」，`agents.md` 第二条）：父会话开的、进过父会话留言的
//! 那一轮结束时报；人自己开的、它自己的子代理开的不报；孙代理都报完了、被叫醒的几轮也结束了再报；报最后说的话，超长的
//! 截头尾，人插过话的注明；出错、到步数上限的照报 `done`，没说话的正文空着。主会话从来不报。
//!
//! 载入、崩了、重启、打断、父会话认出重交的在 `upward_load.rs`。

use super::*;
use crate::event::ChildReason;
use crate::session::Upward;

/// 派出这个子会话的父会话。
pub(super) const PARENT: &str = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10";
/// 它派的两个孙代理。
pub(super) const GRANDCHILD: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d92";
pub(super) const GRANDCHILD_2: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d93";

/// 父会话派出来的子会话的替身：`say` 说的都是父会话的话。
pub(super) fn child() -> Stage {
    Stage::child(policy, environment("~/src/miyu"), at(0), PARENT)
}

/// 最近开的那一轮。
pub(super) fn last_turn(s: &Stage) -> TurnId {
    *s.turns().last().unwrap()
}

/// 一份回报。
pub(super) fn up(
    turn: TurnId,
    reason: ChildReason,
    text: &str,
    truncated: bool,
    person: bool,
) -> Upward {
    Upward {
        turn,
        reason,
        text: text.to_string(),
        truncated,
        person,
    }
}

#[test]
fn the_task_turn_reports_its_last_answer() {
    let mut s = child();
    s.model([
        Line::calls("先读一下。", &[("read", r#"{"path":"Cargo.toml"}"#)]),
        Line::says("有三个 crate。"),
    ]);
    s.tools([Play::done("[workspace]")]);
    s.say("读 Cargo.toml，说有几个 crate");
    assert_eq!(
        s.reported_up(),
        [up(
            turn(),
            ChildReason::Done,
            "有三个 crate。",
            false,
            false
        )],
        "报的是那一轮最后说的话，只报一次"
    );
}

#[test]
fn a_main_session_never_reports() {
    let mut s = stage();
    s.model([Line::says("好。")]);
    s.say("你好");
    assert!(s.reported_up().is_empty());
}

#[test]
fn a_turn_the_person_opens_alone_does_not_report() {
    let mut s = child();
    s.model([Line::says("做完了。"), Line::says("我看了看。")]);
    s.say("做这件事");
    s.person_says("你在做什么");
    assert_eq!(s.reported_up().len(), 1, "人自己开的那一轮不报");
}

#[test]
fn a_parent_message_in_a_turn_the_person_opened_reports_it_with_the_person() {
    let mut s = child();
    s.model([
        Line::says("做完了。"),
        Line::says("我看了看。").held(),
        Line::says("留言收到。"),
    ]);
    s.say("做这件事");
    s.person_says("你在做什么");
    s.say("再补一句");
    s.release_model();
    let reported = s.reported_up();
    assert_eq!(reported.len(), 2);
    assert_eq!(
        reported[1],
        up(last_turn(&s), ChildReason::Done, "留言收到。", false, true),
        "人开的那一轮里进过父会话的留言：报，注明人也在"
    );
}

#[test]
fn a_person_talking_during_the_task_is_noted() {
    let mut s = child();
    s.model([
        Line::says("做了一半。").held(),
        Line::says("照你说的改了。"),
    ]);
    s.say("做这件事");
    s.person_says("换个做法");
    s.release_model();
    assert_eq!(
        s.reported_up(),
        [up(
            last_turn(&s),
            ChildReason::Done,
            "照你说的改了。",
            false,
            true
        )],
        "接着开的那一轮结束才报，人插过话"
    );
}

#[test]
fn a_long_answer_keeps_its_head_and_tail() {
    let mut s = child();
    let answer = "0123456789".repeat(5);
    s.model([Line::says(&answer)]);
    s.say("说长一点");
    let text = format!("{}\n[10 cut]\n{}", &answer[..20], &answer[30..]);
    assert_eq!(
        s.reported_up(),
        [up(turn(), ChildReason::Done, &text, true, false)]
    );
}

#[test]
fn a_turn_ending_in_error_reports_what_it_last_said() {
    let mut s = child();
    s.model([Line::breaks("说到一半", ErrorClass::Auth, "401")]);
    s.say("做这件事");
    assert_eq!(
        s.reported_up(),
        [up(turn(), ChildReason::Done, "说到一半", false, false)]
    );
}

#[test]
fn a_turn_that_said_nothing_reports_an_empty_text() {
    let mut s = child();
    s.model([Line::fails(ErrorClass::Auth, "401")]);
    s.say("做这件事");
    assert_eq!(
        s.reported_up(),
        [up(turn(), ChildReason::Done, "", false, false)]
    );
}

#[test]
fn the_step_limit_reports_what_it_last_said() {
    let limited = || {
        let mut limited = policy();
        limited.step_limit = Some(2);
        limited
    };
    let mut s = Stage::child(limited, environment("~/src/miyu"), at(0), PARENT);
    s.model([
        Line::calls("先读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::calls("", &[("read", r#"{"path":"b"}"#)]),
    ]);
    s.tools([Play::done("a"), Play::done("b")]);
    s.say("做这件事");
    assert_eq!(
        s.reported_up(),
        [up(turn(), ChildReason::Done, "先读一下。", false, false)],
        "后面那次只调了工具、没说话：报的还是最后说的那句"
    );
}

#[test]
fn a_later_turn_that_said_nothing_does_not_repeat_the_old_answer() {
    let mut s = child();
    s.model([Line::says("做完了。"), Line::fails(ErrorClass::Auth, "401")]);
    s.say("做这件事");
    s.say("再做一件");
    assert_eq!(
        s.reported_up()[1],
        up(last_turn(&s), ChildReason::Done, "", false, false),
        "报的是那一轮的话：那一轮没说话，正文空着"
    );
}

/// 子会话第一轮派出两个孙代理，说完了；这时不报。
pub(super) fn dispatched_two() -> Stage {
    let mut s = child();
    s.model([
        Line::calls("派两个。", &[("shell", "{}"), ("shell", "{}")]),
        Line::says("等它们回来。"),
    ]);
    s.tools([
        Play::starts_agent(1, "一", GRANDCHILD),
        Play::starts_agent(2, "二", GRANDCHILD_2),
    ]);
    s.say("分两半查");
    assert!(s.reported_up().is_empty(), "孙代理都没报");
    s
}

#[test]
fn it_reports_after_its_grandchildren_and_the_turns_they_wake() {
    let mut s = dispatched_two();
    s.model([Line::says("回来一个。"), Line::says("两半都查完了。")]);
    s.child_reports(1, GRANDCHILD, ChildReason::Done, "前一半");
    assert!(s.reported_up().is_empty(), "还有一个没报");
    s.child_reports(2, GRANDCHILD_2, ChildReason::Done, "后一半");
    assert_eq!(
        s.reported_up(),
        [up(
            last_turn(&s),
            ChildReason::Done,
            "两半都查完了。",
            false,
            false
        )],
        "都报完了、被叫醒的那一轮结束了，报一次"
    );
}

#[test]
fn a_grandchild_that_crashed_releases_the_report() {
    let mut s = dispatched_two();
    s.model([Line::says("回来一个。")]);
    s.child_reports(1, GRANDCHILD, ChildReason::Done, "前一半");
    // 崩了的回报不叫醒她：闲着、都报过了，当场报最近那一轮最后说的话。
    s.child_reports(2, GRANDCHILD_2, ChildReason::Aborted, "");
    assert_eq!(
        s.reported_up(),
        [up(
            last_turn(&s),
            ChildReason::Done,
            "回来一个。",
            false,
            false
        )]
    );
}

#[test]
fn a_turn_its_own_child_opens_does_not_report() {
    let mut s = child();
    s.model([
        Line::says("做完了。"),
        Line::calls("派一个。", &[("shell", "{}")]),
        Line::says("派出去了。"),
        Line::says("孙代理回来了。"),
    ]);
    s.tools([Play::starts_agent(1, "一", GRANDCHILD)]);
    s.say("做这件事");
    s.person_says("再派一个去看看");
    s.child_reports(1, GRANDCHILD, ChildReason::Done, "看过了");
    assert_eq!(
        s.reported_up().len(),
        1,
        "人开的、孙代理的回报开的都不欠父会话"
    );
}

/// 第一轮：3 号。
fn turn() -> TurnId {
    TurnId::new(seq(3))
}
