//! 回报到了（施工 7-2，`docs/blueprint/kernel/session.md`「回报」，`agents.md` 第三条）：闲着时到的开一轮、`trigger` 是它；
//! 正忙时到的下一次请求就有它；最后一步里到的，回合结束时接着开；只记下的几种不开轮、下一轮开始时在请求里；打断时排着的
//! 不撤回、不接着开；没人看着的一次性会话只记下，有头订阅着照常开；对不上的回报拒绝、不理。回报一律不带回合编号。
//!
//! 撤销、恢复、载入、读回日志的时候到的在 `reports_undo.rs`。

use super::*;
use crate::event::{ChildReason, JobReason, JobReported};
use crate::origin::Tool;

/// 子代理的会话。
pub(super) const CHILD: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";

/// 第一轮（3 号）派出去一个子代理 `j1`、一个后台命令 `j2`，说完了，到 12 号。
pub(super) fn dispatched(mut s: Stage) -> Stage {
    s.model([
        Line::calls("派出去。", &[("shell", "{}"), ("shell", "{}")]),
        Line::says("派出去了。"),
    ]);
    s.tools([
        Play::starts_agent(1, "查 CI", CHILD),
        Play::starts_command(2, "跑测试"),
    ]);
    s.say("查一下 CI，顺便跑测试");
    assert_eq!(s.log().len(), 12, "{:#?}", story(&s));
    s
}

/// 起 `j2` 的那次调用。
pub(super) fn starter() -> By {
    By::Tool(Tool {
        call_id: call(6, 2),
    })
}

/// 第 `n` 条事件。
pub(super) fn event(s: &Stage, n: u64) -> &Event {
    &s.log()[usize::try_from(n - 1).unwrap()]
}

/// 第 `n` 条是一轮的开头，交回它的触发。
pub(super) fn trigger_of(s: &Stage, n: u64) -> Option<Seq> {
    match &event(s, n).body {
        Body::TurnStarted(started) => started.trigger,
        body => panic!("第 {n} 条应该是 turn.started：{body:?}"),
    }
}

/// 最后一次请求，替身的组装一条一行。
pub(super) fn last_request(s: &Stage) -> String {
    listed_request(&s.requests().last().unwrap().1)
}

#[test]
fn a_report_while_idle_opens_a_turn_it_triggers() {
    let mut s = dispatched(stage());
    s.model([Line::says("看到了。")]);
    let id = s.child_reports(1, CHILD, ChildReason::Done, "CI 红在 macOS。");
    assert_eq!(
        story(&s)[12..],
        [
            "13 child.reported child",
            "14 turn.started kernel t14",
            "15 message.assistant model t14",
            "16 model.called:ok kernel t14",
            "17 turn.ended:completed kernel t14",
        ]
    );
    assert_eq!(trigger_of(&s, 14), Some(seq(13)));
    assert_eq!(event(&s, 14).cause, Some(id.clone()), "cause 照回报的");
    assert_eq!(event(&s, 13).cause, Some(id.clone()));
    assert_eq!(
        s.outcome(&id),
        Some(&Outcome::Accepted {
            events: seqs(&[13])
        }),
        "回应只附回报那一条"
    );
    assert!(last_request(&s).ends_with("13 child.reported\n14 turn.started\n"));
}

#[test]
fn a_command_ending_while_idle_opens_a_turn_by_whom_it_says() {
    let mut s = dispatched(stage());
    s.model([Line::says("测试过了。")]);
    s.job_ends(2, JobReason::Exited, starter(), Some(id(1)));
    let reported = event(&s, 13);
    assert_eq!(reported.by, starter(), "by 照交来的：起它的那次调用");
    assert_eq!(reported.cause, Some(id(1)));
    assert_eq!(reported.turn, None, "回报不带回合编号");
    assert_eq!(trigger_of(&s, 14), Some(seq(13)));
    assert_eq!(event(&s, 14).cause, Some(id(1)));
}

#[test]
fn a_report_while_busy_is_heard_at_the_next_step_and_opens_nothing() {
    let mut s = dispatched(stage());
    s.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("好了。"),
    ]);
    s.tools([Play::done("A").held()]);
    s.say("再看看 a");
    let running = s.ran().last().unwrap().0;
    s.job_ends(2, JobReason::Exited, starter(), Some(id(1)));
    s.release_tool(running);
    assert_eq!(
        story(&s)[12..],
        [
            "13 message.user alice",
            "14 turn.started kernel t14",
            "15 message.assistant model t14",
            "16 model.called:ok kernel t14",
            "17 job.reported tool call_6_2",
            "18 tool.result:ok tool call_15_1 t14",
            "19 message.assistant model t14",
            "20 model.called:ok kernel t14",
            "21 turn.ended:completed kernel t14",
        ],
        "回合中途到的不带回合编号，下一次请求听到了，结束时不再开"
    );
    let (seen, request) = s.requests().last().unwrap();
    assert_eq!(*seen, seq(18));
    assert!(listed_request(request).contains("17 job.reported\n"));
}

#[test]
fn a_report_in_the_last_step_opens_the_next_turn() {
    let mut s = dispatched(stage());
    s.model([Line::says("好").held(), Line::says("看到回报了。")]);
    s.say("hi");
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    s.release_model();
    assert_eq!(
        story(&s)[12..],
        [
            "13 message.user alice",
            "14 turn.started kernel t14",
            "15 child.reported child",
            "16 message.assistant model t14",
            "17 model.called:ok kernel t14",
            "18 turn.ended:completed kernel t14",
            "19 turn.started kernel t19",
            "20 message.assistant model t19",
            "21 model.called:ok kernel t19",
            "22 turn.ended:completed kernel t19",
        ]
    );
    assert_eq!(trigger_of(&s, 19), Some(seq(15)));
    assert_eq!(event(&s, 19).cause, event(&s, 15).cause, "cause 照回报的");
}

#[test]
fn a_message_and_a_report_in_the_last_step_the_later_one_opens() {
    let mut s = dispatched(stage());
    s.model([Line::says("好").held(), Line::says("都看到了。")]);
    s.say("hi");
    s.job_ends(2, JobReason::Exited, starter(), None);
    s.say("还有这句");
    s.release_model();
    assert_eq!(trigger_of(&s, 20), Some(seq(16)), "排着的消息在回报后面");
    let mut s = dispatched(stage());
    s.model([Line::says("好").held(), Line::says("都看到了。")]);
    s.say("hi");
    s.say("还有这句");
    s.job_ends(2, JobReason::Exited, starter(), None);
    s.release_model();
    assert_eq!(trigger_of(&s, 20), Some(seq(16)), "回报在排着的消息后面");
}

/// 派出去四个后台命令（`j2` 到 `j5`）、两个子代理（`j1`、`j6`），说完了。
fn dispatched_many() -> Stage {
    let mut s = stage();
    s.model([
        Line::calls("派出去。", &[("read", "{}"); 6]),
        Line::says("派出去了。"),
    ]);
    s.tools([
        Play::starts_agent(1, "a", CHILD),
        Play::starts_command(2, "b"),
        Play::starts_command(3, "c"),
        Play::starts_command(4, "d"),
        Play::starts_command(5, "e"),
        Play::starts_agent(6, "f", "01a0d78c-ca52-7d19-8b64-000000000006"),
    ]);
    s.say("派六个");
    s
}

#[test]
fn the_quiet_reports_only_record_and_the_next_turn_hears_them() {
    let mut s = dispatched_many();
    let before = s.log().len();
    let stopped_by_her = JobReported {
        by_model: true,
        ..reported(2, JobReason::Stopped)
    };
    s.job_ends_with(starter(), None, stopped_by_her);
    s.job_ends(3, JobReason::Undone, alice(), Some(id(7)));
    s.job_ends(4, JobReason::Restarted, By::Kernel, None);
    s.job_ends(5, JobReason::Aborted, By::Kernel, None);
    s.child_reports(1, CHILD, ChildReason::Undone, "");
    s.child_reports(
        6,
        "01a0d78c-ca52-7d19-8b64-000000000006",
        ChildReason::Aborted,
        "",
    );
    let kinds: Vec<String> = s.log()[before..]
        .iter()
        .map(|event| event.body.kind().to_string())
        .collect();
    assert_eq!(
        kinds,
        [
            "job.reported",
            "job.reported",
            "job.reported",
            "job.reported",
            "child.reported",
            "child.reported"
        ],
        "只记下，不开轮"
    );
    s.model([Line::says("知道了。")]);
    s.say("怎么样了");
    let request = last_request(&s);
    for (n, kind) in (before + 1..).zip(kinds) {
        assert!(
            request.contains(&format!("{n} {kind}\n")),
            "下一轮开始时在请求里：{request}"
        );
    }
}

#[test]
fn a_person_stopping_a_subagent_still_wakes_her() {
    let mut s = dispatched(stage());
    s.model([Line::says("它被停了。")]);
    s.child_reports(1, CHILD, ChildReason::Stopped, "");
    assert_eq!(trigger_of(&s, 14), Some(seq(13)), "被人停掉的照样开一轮");
    let mut s = dispatched(stage());
    s.model([Line::says("它被停了。")]);
    s.job_ends(2, JobReason::Stopped, alice(), Some(id(9)));
    assert_eq!(trigger_of(&s, 14), Some(seq(13)), "人停的后台命令也开");
}

#[test]
fn reports_queued_when_interrupted_are_not_withdrawn_and_open_nothing() {
    for queued in [Queued::Return, Queued::Send] {
        let mut s = dispatched(stage());
        s.model([Line::says("").held()]);
        s.say("hi");
        s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
        let stop = s.interrupt(queued);
        let tail: Vec<&str> = s.log()[14..]
            .iter()
            .map(|event| event.body.kind())
            .collect();
        assert_eq!(
            tail,
            ["child.reported", "model.called", "turn.ended"],
            "{queued:?}：不撤回，也不由它接着开"
        );
        assert!(matches!(s.outcome(&stop), Some(Outcome::Accepted { .. })));
        s.model([Line::says("看到了。")]);
        s.say("刚才的回报呢");
        assert!(last_request(&s).contains("15 child.reported\n"));
    }
    // 接着发的，排着的消息接着开下一轮，回报在那一轮里一起听到。
    let mut s = dispatched(stage());
    s.model([Line::says("").held(), Line::says("都看到了。")]);
    s.say("hi");
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    s.say("还有这句");
    s.interrupt(Queued::Send);
    assert_eq!(trigger_of(&s, 19), Some(seq(16)), "由排着的消息接着开");
    assert!(last_request(&s).contains("15 child.reported\n"));
}

#[test]
fn a_oneshot_session_nobody_watches_only_records() {
    let mut s = dispatched(Stage::oneshot(policy, environment("~/src/miyu"), at(0)));
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    assert_eq!(s.turns().len(), 1, "没人看着：只记下");
    s.watched(true);
    assert_eq!(s.turns().len(), 1, "头订阅了也不因为以前的开轮");
    s.model([Line::says("测试过了。")]);
    s.job_ends(2, JobReason::Exited, starter(), None);
    assert_eq!(trigger_of(&s, 15), Some(seq(14)), "有头订阅着照常开");
    assert!(
        last_request(&s).contains("13 child.reported\n"),
        "只记下的一起看到"
    );
    // 头退了：最后一步里到的，回合结束时也不接着开。
    let mut s = dispatched(Stage::oneshot(policy, environment("~/src/miyu"), at(0)));
    s.watched(true);
    s.model([Line::says("好").held()]);
    s.say("hi");
    s.watched(false);
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    s.release_model();
    assert_eq!(s.turns().len(), 2, "没人看着了，不接着开");
}

#[test]
fn reports_that_do_not_fit_are_refused_or_ignored() {
    let mut s = dispatched(stage());
    let length = s.log().len();
    let refused = |s: &Stage, id: &CommandId| {
        assert_eq!(
            s.outcome(id),
            Some(&Outcome::Rejected {
                reason: Reason::UnknownJob
            })
        );
    };
    let command = s.child_reports(2, CHILD, ChildReason::Done, "");
    refused(&s, &command);
    let unknown = s.child_reports(9, CHILD, ChildReason::Done, "");
    refused(&s, &unknown);
    let other = s.child_reports(
        1,
        "01a0d78c-ca52-7d19-8b64-000000000009",
        ChildReason::Done,
        "",
    );
    refused(&s, &other);
    assert_eq!(Reason::UnknownJob.code(), "unknown_job");
    s.job_ends(1, JobReason::Exited, starter(), None);
    s.job_ends(7, JobReason::Exited, starter(), None);
    assert_eq!(s.log().len(), length, "对不上的什么都不记");
    // 以 stopped 报过的子代理不再报；后台命令只报一次结束。
    s.model([Line::says("停了。"), Line::says("测试过了。")]);
    s.child_reports(1, CHILD, ChildReason::Stopped, "");
    let again = s.child_reports(1, CHILD, ChildReason::Done, "");
    refused(&s, &again);
    s.job_ends(2, JobReason::Exited, starter(), None);
    let length = s.log().len();
    s.job_ends(2, JobReason::Exited, starter(), None);
    assert_eq!(s.log().len(), length, "结束过的不理");
}

/// 后台命令 `j<job>` 结束的 `body`，除了原因都没有。
fn reported(job: u64, reason: JobReason) -> JobReported {
    JobReported {
        job: crate::id::JobId::new(job).unwrap(),
        reason,
        exit_code: None,
        signal: None,
        by_model: false,
        duration_ms: None,
        output: None,
        chars: None,
    }
}
