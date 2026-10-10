//! 会话状态里投影算得出的那一半（施工 9-8 补上）：闲着、在跑、在等人；正在做什么；上下文用了多少；重试和都在冷却；任务表
//! 照派出、了结、叫醒记，做完的只留最近的几个。

use std::sync::Arc;

use miyu_kernel::event::{
    Choice, Decision, ErrorClass, JobReason, Question, Response, TransientBody, Usage,
};
use miyu_kernel::id::ModuleId;
use miyu_kernel::origin::By;
use miyu_kernel::session::Verdict;
use miyu_kernel::testkit::{Line, Play, Stage, Streamed};
use miyu_kernel::tool::Access;
use miyu_view::{Doing, JobState, Projector, State, Status, Wait};

use crate::support::*;

/// 落了盘的和瞬时的照先后喂，每喂一条记一份状态。
fn statuses(stage: &Stage) -> Vec<Status> {
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    let mut seen = Vec::new();
    for item in stage.stream() {
        match item {
            Streamed::Event(event) => drop(projector.event(event)),
            Streamed::Transient(transient) => drop(projector.transient(transient)),
        }
        seen.push(projector.status());
    }
    seen
}

/// 最后的状态：只喂落了盘的（翻页、订阅时那一页的样子）。
fn paged(stage: &Stage) -> Status {
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for event in stage.log() {
        drop(projector.event(event));
    }
    projector.status()
}

#[test]
fn a_finished_turn_is_idle_with_the_context_it_used() {
    let mut stage = stage();
    stage.model([Line::says("Hi.").reports(1_234)]);
    stage.say("hello");
    let status = paged(&stage);
    assert_eq!(status.state, State::Idle);
    assert_eq!(status.since, None);
    assert_eq!(status.doing, None);
    assert_eq!(status.used, Some(1_234));
    let live = statuses(&stage);
    assert!(
        live.iter()
            .any(|s| s.state == State::Running && s.since.is_some()),
        "跑的时候是 running、带开始的时刻"
    );
    assert!(
        live.iter()
            .any(|s| matches!(s.doing, Some(Doing::Writing { .. }))),
        "写回答的时候是 writing"
    );
    assert_eq!(live.last(), Some(&status), "视图流最后和翻页一样");
}

#[test]
fn a_held_tool_keeps_it_running_on_that_step() {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("read", r#"{"file_path":"a"}"#)]),
        Line::says("Done."),
    ]);
    stage.tools([Play::done("A").held()]);
    stage.say("read");
    let status = statuses(&stage).pop().expect("有状态");
    assert_eq!(status.state, State::Running);
    assert!(
        matches!(status.doing, Some(Doing::Tool { .. })),
        "{status:?}"
    );
    let call = stage.held_tools()[0];
    stage.release_tool(call);
    assert_eq!(paged(&stage).state, State::Idle);
}

#[test]
fn an_approval_and_a_question_are_waited_for_until_settled() {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("write", r#"{"file_path":"a","content":"x"}"#)]),
        Line::calls("", &[("ask_user", r#"{"questions":[]}"#)]),
        Line::says("Thanks."),
    ]);
    stage.guards([Verdict::Ask {
        module: ModuleId::parse("permissions").expect("合写法"),
        access: Access::Write,
        rule: None,
        detail: None,
    }]);
    let question = Question {
        header: None,
        question: "Which one?".to_string(),
        options: vec![Choice {
            label: "A".to_string(),
            description: None,
            preview: None,
        }],
        multiple: false,
    };
    stage.tools([Play::done("written"), Play::Asks(vec![question])]);
    stage.say("write it");
    let waiting = paged(&stage);
    assert_eq!(waiting.state, State::Waiting, "{waiting:?}");
    assert_eq!(waiting.waiting.len(), 1);
    assert_eq!(waiting.waiting[0].what, Wait::Approve);
    let call = waiting.waiting[0].call;
    stage.decide(call, Decision::Once, None);
    let asking = paged(&stage);
    assert_eq!(asking.state, State::Waiting, "{asking:?}");
    assert_eq!(asking.waiting[0].what, Wait::Ask);
    assert_ne!(asking.waiting[0].call, call);
    let entry = asking.waiting[0].entry.clone();
    let question_call = asking.waiting[0].call;
    assert!(
        entry.as_str().starts_with('b'),
        "对到那一步的条目：{entry:?}"
    );
    stage.reply(
        question_call,
        vec![Response {
            picked: vec!["A".to_string()],
            text: None,
            notes: None,
        }],
    );
    let done = paged(&stage);
    assert_eq!(done.state, State::Idle);
    assert!(done.waiting.is_empty());
}

#[test]
fn retrying_and_cooling_show_and_clear() {
    let mut stage = stage();
    stage.model([
        Line::fails(ErrorClass::Cooling, "all candidates cooling"),
        Line::says("Back."),
    ]);
    stage.say("hi");
    let live = statuses(&stage);
    let retried = live
        .iter()
        .find(|s| matches!(s.doing, Some(Doing::Retrying { .. })))
        .expect("重试时是 retrying");
    assert!(retried.cooling_until.is_some(), "都在冷却：{retried:?}");
    let Some(Doing::Retrying { at, class, .. }) = &retried.doing else {
        unreachable!("上面挑的就是 retrying");
    };
    assert_eq!(*class, ErrorClass::Cooling);
    let wait = stage
        .transients()
        .iter()
        .find_map(|t| match &t.body {
            TransientBody::Status(status) => Some((t.at, status.retry.wait_ms)),
            _ => None,
        })
        .expect("有重试的瞬时事件");
    assert_eq!(
        at.unix_millis(),
        wait.0.unix_millis() + i64::try_from(wait.1).expect("不大"),
        "再试的时刻是那一刻加上要等的"
    );
    let last = live.last().expect("有状态");
    assert_eq!(last.cooling_until, None, "说成了就不再冷却");
    assert_eq!(last.doing, None);
}

#[test]
fn jobs_are_listed_from_start_to_end() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[(
                "shell",
                r#"{"command":"cargo build","description":"Build","run_in_background":true}"#,
            )],
        ),
        Line::says("Started."),
        Line::says("Built."),
    ]);
    stage.tools([Play::starts_command(1, "Build")]);
    stage.say("build");
    let running = paged(&stage);
    assert_eq!(running.jobs.len(), 1);
    let row = &running.jobs[0];
    assert_eq!(row.state, JobState::Running);
    assert_eq!(row.title, "Build");
    assert_eq!(row.command.as_deref(), Some("cargo build"));
    assert_eq!(row.ended, None);
    stage.job_ends(1, JobReason::Exited, By::Kernel, None);
    let done = paged(&stage);
    let row = &done.jobs[0];
    assert_eq!(row.state, JobState::Done);
    assert_eq!(row.exit_code, Some(0));
    assert!(row.ended.is_some());
}

#[test]
fn a_stopped_job_says_why() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[(
                "shell",
                r#"{"command":"sleep 9","description":"Wait","run_in_background":true}"#,
            )],
        ),
        Line::says("Started."),
        Line::says("Stopped."),
    ]);
    stage.tools([Play::starts_command(1, "Wait")]);
    stage.say("wait");
    stage.job_ends(1, JobReason::Undone, By::Kernel, None);
    let row = &paged(&stage).jobs[0];
    assert_eq!(row.state, JobState::Stopped);
    assert_eq!(row.why, Some("undone"));
}

#[test]
fn a_decided_approval_stops_waiting_while_the_tool_runs() {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("write", r#"{"file_path":"a","content":"x"}"#)]),
        Line::says("Done."),
    ]);
    stage.guards([Verdict::Ask {
        module: ModuleId::parse("permissions").expect("合写法"),
        access: Access::Write,
        rule: None,
        detail: None,
    }]);
    stage.tools([Play::done("written").held()]);
    stage.say("write it");
    let call = paged(&stage).waiting[0].call;
    stage.decide(call, Decision::Once, None);
    let running = paged(&stage);
    assert_eq!(running.state, State::Running, "定了就不等人：{running:?}");
    assert!(running.waiting.is_empty());
}

#[test]
fn the_context_counts_all_three_inputs() {
    let mut stage = stage();
    stage.model([Line {
        usage: Some(Usage {
            uncached: 100,
            cache_read: 200,
            cache_write: 30,
            output: 5,
            reasoning: None,
        }),
        ..Line::says("Hi.")
    }]);
    stage.say("hello");
    assert_eq!(paged(&stage).used, Some(330));
}

#[test]
fn only_the_latest_finished_jobs_are_kept() {
    let mut stage = stage();
    let total = miyu_view::FINISHED_KEPT as u64 + 2;
    let calls: Vec<(String, String)> = (1..=total)
        .map(|n| {
            (
                "shell".to_string(),
                format!(
                    r#"{{"command":"echo {n}","description":"Job {n}","run_in_background":true}}"#
                ),
            )
        })
        .collect();
    let calls: Vec<(&str, &str)> = calls
        .iter()
        .map(|(name, args)| (name.as_str(), args.as_str()))
        .collect();
    stage.model([Line::calls("", &calls), Line::says("Started.")]);
    stage.tools((1..=total).map(|n| Play::starts_command(n, &format!("Job {n}"))));
    stage.say("run them");
    for n in 1..=total {
        stage.model([Line::says("Noted.")]);
        stage.job_ends(n, JobReason::Exited, By::Kernel, None);
    }
    let jobs = paged(&stage).jobs;
    assert_eq!(jobs.len(), miyu_view::FINISHED_KEPT);
    assert_eq!(jobs[0].title, "Job 3", "最早做完的两个拿掉了");
}
