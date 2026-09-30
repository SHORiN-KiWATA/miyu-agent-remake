//! 向上回报：载入、崩了、重启、打断（施工 7-6，`docs/blueprint/kernel/session.md`「向上回报」，`agents.md` 第八条）。崩在
//! 那一轮里的，载入时补 `aborted`；报了、还没送到就崩了的，载入时最后报的那一份再交一次，父会话照命令编号认出重的；
//! 被重启打断的接着干、做完再报，没接着干的照 `aborted` 报；被打断的等下一轮。

use super::reports::{CHILD, dispatched};
use super::upward::{GRANDCHILD, GRANDCHILD_2, PARENT, child, dispatched_two, last_turn, up};
use super::*;
use crate::event::ChildReason;

#[test]
fn a_crash_in_the_task_reports_aborted_on_load() {
    let mut s = child();
    s.model([Line::says("做到一半").held()]);
    s.say("做这件事");
    assert!(s.reported_up().is_empty());
    s.crash();
    assert_eq!(
        s.reported_up(),
        [up(first(), ChildReason::Aborted, "", false, false)],
        "那一轮没写下回复：正文空着"
    );
}

#[test]
fn the_last_report_is_handed_again_on_load() {
    let mut s = child();
    s.model([Line::says("做完了。")]);
    s.say("做这件事");
    s.crash();
    let again = up(first(), ChildReason::Done, "做完了。", false, false);
    assert_eq!(
        s.reported_up(),
        [again.clone(), again],
        "同一轮的同一份：父会话照命令编号认出重的"
    );
}

#[test]
fn a_task_cut_by_a_restart_resumes_and_reports_when_done() {
    let mut s = child();
    s.model([Line::says("做到一半").held(), Line::says("做完了。")]);
    s.say("做这件事");
    s.restart();
    assert_eq!(
        s.reported_up(),
        [up(
            last_turn(&s),
            ChildReason::Done,
            "做完了。",
            false,
            false
        )],
        "被重启打断的那一轮不报，接着干的那一轮做完再报"
    );
}

#[test]
fn a_task_not_resumed_after_a_restart_reports_aborted() {
    let never = || {
        let mut never = policy();
        never.resumes = 0;
        never
    };
    let mut s = Stage::child(never, environment("~/src/miyu"), at(0), PARENT);
    s.model([Line::says("做到一半").held()]);
    s.say("做这件事");
    s.restart();
    assert_eq!(
        s.reported_up(),
        [up(first(), ChildReason::Aborted, "做到一半", false, false)],
        "重启截下的半截就是它最后说的"
    );
}

#[test]
fn an_interrupted_task_reports_when_the_next_turn_ends() {
    let mut s = child();
    s.model([Line::says("做到一半").held(), Line::says("接着做完了。")]);
    s.say("做这件事");
    s.interrupt(Queued::Return);
    assert!(s.reported_up().is_empty(), "打断了停在半路：等人说话");
    s.person_says("接着做");
    assert_eq!(
        s.reported_up(),
        [up(
            last_turn(&s),
            ChildReason::Done,
            "接着做完了。",
            false,
            true
        )]
    );
}

#[test]
fn the_parent_knows_a_report_handed_again_after_its_recent_commands() {
    let mut s = dispatched(stage());
    s.model([Line::says("看到了。")]);
    let id = s.child_reports(1, CHILD, ChildReason::Done, "CI 红在 macOS。");
    // 挤掉记着的最近 1024 个编号。
    for k in 0..1100 {
        s.set_permission(None, Some(k % 2 == 0));
    }
    let before = s.log().len();
    s.child_reports_again(&id, 1, CHILD, ChildReason::Done, "CI 红在 macOS。");
    assert_eq!(s.log().len(), before, "重交的不再记");
    assert_eq!(
        s.outcome(&id),
        Some(&Outcome::Accepted {
            events: seqs(&[13])
        }),
        "照上一次回应"
    );
    // 换一个编号的是新的一份。
    s.model([Line::says("又看到了。")]);
    s.child_reports(1, CHILD, ChildReason::Done, "又查了一遍。");
    assert!(s.log().len() > before);
}

#[test]
fn the_resent_report_is_the_one_reported_before_later_turns() {
    let mut s = child();
    s.model([Line::says("做完了。"), Line::says("我看了看。")]);
    s.say("做这件事");
    s.person_says("你在做什么");
    s.crash();
    let first = up(first(), ChildReason::Done, "做完了。", false, false);
    assert_eq!(
        s.reported_up(),
        [first.clone(), first],
        "人后来开的那一轮不欠：再交的还是报过的那一份"
    );
}

#[test]
fn a_report_after_a_continuation_is_resent_as_it_was() {
    let mut s = child();
    s.model([
        Line::says("做了一半。").held(),
        Line::says("照你说的改了。"),
    ]);
    s.say("做这件事");
    s.person_says("换个做法");
    s.release_model();
    s.crash();
    let reported = s.reported_up();
    assert_eq!(reported.len(), 2);
    assert_eq!(
        reported[0], reported[1],
        "一轮结束、接着开的那一轮在同一批：那时不报"
    );
}

#[test]
fn the_report_waits_until_the_whole_batch_is_on_disk() {
    let mut s = child();
    s.model([Line::calls("读一下。", &[("read", r#"{"path":"a"}"#)])]);
    s.tools([Play::done("a").held()]);
    s.say("做这件事");
    // 崩在调用没结果的时候：载入补「已取消」和结束，一批里两条。
    let (mut session, actions) = Session::load(
        SessionId::parse(crate::testkit::CHILD_SESSION).unwrap(),
        s.log().to_vec(),
        at(59),
        policy(),
        environment("~/src/miyu"),
    )
    .unwrap();
    let Some(Action::Append(events)) = actions.last() else {
        panic!("载入补了一批：{actions:?}");
    };
    assert_eq!(events.len(), 2, "{events:?}");
    let first = session.handle(Input::Stored {
        at: at(59),
        upto: events[0].seq,
    });
    assert!(
        !first
            .iter()
            .any(|action| matches!(action, Action::Report(_))),
        "结束那一条还没落盘：{first:?}"
    );
    let rest = session.handle(Input::Stored {
        at: at(59),
        upto: events[1].seq,
    });
    assert!(
        rest.iter()
            .any(|action| matches!(action, Action::Report(upward) if upward.reason == ChildReason::Aborted)),
        "{rest:?}"
    );
}

#[test]
fn nothing_is_handed_up_while_stopping_and_the_load_hands_it() {
    let mut s = dispatched_two();
    s.model([Line::says("回来一个。")]);
    s.child_reports(1, GRANDCHILD, ChildReason::Done, "前一半");
    s.restarting();
    // 要关了：孙代理崩了的回报只记下，这时不交，父会话也在停。
    s.child_reports(2, GRANDCHILD_2, ChildReason::Aborted, "");
    assert!(s.reported_up().is_empty());
    s.crash();
    assert_eq!(
        s.reported_up(),
        [up(
            last_turn(&s),
            ChildReason::Done,
            "回来一个。",
            false,
            false
        )],
        "再载入时照日志算出该报，交一次"
    );
}

/// 第一轮：3 号。
fn first() -> TurnId {
    TurnId::new(seq(3))
}
