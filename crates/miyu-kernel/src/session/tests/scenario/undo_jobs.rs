//! 撤销时停掉那几轮派出去的（施工 7-8，`docs/blueprint/agents.md` 第七条第 1、3 条，`kernel/history.md`「撤销」）：撤掉的
//! 那几轮派出去、还在跑的后台命令和子代理，记下撤销的同时交出 `StopJobs`，`by`、`cause` 是撤销的人和命令；已经结束了的、
//! 别的回合派的不停；停好了的回报记 `undone`，不叫醒她；恢复撤销不重起；重做的撤销那一半一样停；改过文件的先停任务、
//! 再改回文件。

use super::super::load::Logged;
use super::super::restore::steps_of;
use super::super::revert::revert as undo;
use super::reports::{CHILD, dispatched, last_request, starter};
use super::*;
use crate::event::{ChildReason, Effect, FileChanged, JobKind, JobReason, JobStarted};
use crate::id::{ContentHash, JobId};

/// 任务编号 `j<n>`。
fn job(n: u64) -> JobId {
    JobId::new(n).unwrap()
}

/// 撤销 `undo` 交出的停任务：停 `jobs`，`by` 是 alice。
fn stop(jobs: &[u64], undo: &CommandId) -> Action {
    Action::StopJobs {
        jobs: jobs.iter().map(|&n| job(n)).collect(),
        by: alice(),
        cause: undo.clone(),
    }
}

#[test]
fn undoing_the_turn_stops_what_it_dispatched_and_the_reports_only_record() {
    let mut s = dispatched(stage());
    let undo = s.revert(TurnId::new(seq(3)));
    assert_eq!(s.stopping(), [stop(&[1, 2], &undo)], "子代理、后台命令都停");
    s.child_reports(1, CHILD, ChildReason::Undone, "");
    s.job_ends(2, JobReason::Undone, alice(), Some(undo.clone()));
    assert_eq!(s.turns().len(), 1, "undone 的只记下，不叫醒她");
    let reported = &s.log()[s.log().len() - 1];
    assert_eq!(
        (&reported.by, reported.cause.as_ref(), reported.turn),
        (&alice(), Some(&undo), None),
        "后台命令那条 by、cause 照撤销的，不带回合编号"
    );
}

#[test]
fn jobs_that_already_ended_are_not_stopped() {
    let mut s = dispatched(stage());
    s.model([Line::says("跑完了。")]);
    s.job_ends(2, JobReason::Exited, starter(), Some(id(1)));
    let undo = s.revert(TurnId::new(seq(3)));
    assert_eq!(s.stopping(), [stop(&[1], &undo)], "跑完了的后台命令不停");

    let mut s = dispatched(stage());
    s.model([Line::says("看到了。"), Line::says("跑完了。")]);
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    s.job_ends(2, JobReason::Exited, starter(), Some(id(1)));
    s.revert(TurnId::new(seq(3)));
    assert!(s.stopping().is_empty(), "都结束了，什么都不停");
}

#[test]
fn an_agent_messaged_after_its_report_is_stopped_again() {
    let mut s = dispatched(stage());
    s.model([
        Line::calls("再问问。", &[("read", "{}")]),
        Line::says("问了。"),
    ]);
    s.tools([Play::messages(1)]);
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    let undo = s.revert(TurnId::new(seq(3)));
    assert_eq!(
        s.stopping(),
        [stop(&[1, 2], &undo)],
        "报过以后又被留了言的，又在干活，照样停：{:#?}",
        story(&s)
    );
}

#[test]
fn undoing_a_later_turn_leaves_the_earlier_jobs_running() {
    let mut s = dispatched(stage());
    s.model([Line::says("好。")]);
    s.say("再来");
    s.revert(TurnId::new(seq(14)));
    assert!(s.stopping().is_empty(), "派它们的那一轮没撤");
}

#[test]
fn restoring_the_undo_does_not_restart_them() {
    let mut s = dispatched(stage());
    let undo = s.revert(TurnId::new(seq(3)));
    s.child_reports(1, CHILD, ChildReason::Undone, "");
    s.job_ends(2, JobReason::Undone, alice(), Some(undo));
    s.unrevert();
    assert_eq!(s.stopping().len(), 1, "恢复不停也不起");
    assert_eq!(s.turns().len(), 1, "恢复了也不开轮：undone 的只记下");
    s.model([Line::says("它们被撤销停掉了。")]);
    s.say("它们呢");
    let request = last_request(&s);
    assert!(
        request.contains("child.reported") && request.contains("job.reported"),
        "派它们的那一轮回来了，undone 的回报照留、照渲染：{request}"
    );
    s.revert(TurnId::new(seq(3)));
    assert_eq!(s.stopping().len(), 1, "停过的不再停");
}

#[test]
fn a_redo_stops_them_too() {
    let mut s = dispatched(stage());
    s.model([Line::says("好。")]);
    let redo = s.redo(None);
    assert_eq!(s.stopping(), [stop(&[1, 2], &redo)]);
}

/// 撤掉的那一轮改过文件、又起了一条后台命令：先停任务，再改回文件（停下的命令不会再动文件）。
#[test]
fn stopping_comes_before_restoring_files() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "改一下，顺便跑测试");
    logged.tools(seen, &[("write", r#"{"file_path":"a.txt"}"#)]);
    let reply = logged
        .log
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::MessageAssistant(_)))
        .map(|event| event.seq)
        .unwrap();
    logged.handle(Input::ToolDone {
        at: at(50),
        call_id: call(reply.get(), 1),
        error: false,
        blocks: Vec::new(),
        duration_ms: Some(3),
        human: None,
        effects: vec![
            Effect::FileChanged(FileChanged {
                path: "/w/a.txt".to_string(),
                before: Some(ContentHash::of(b"A")),
                after: ContentHash::of(b"B"),
            }),
            Effect::JobStarted(JobStarted {
                job: job(1),
                what: JobKind::Command,
                title: "跑测试".to_string(),
                session: None,
            }),
        ],
        stopped: false,
    });
    let actions = logged.handle(stored(logged.last()));
    let (seen, _) = calls(&actions).remove(0);
    logged.say(seen.get(), "改好了");
    let actions = logged.handle(undo(2, 3));
    let kinds: Vec<&str> = actions
        .iter()
        .map(|action| match action {
            Action::Append(_) => "append",
            Action::StopJobs { .. } => "stop_jobs",
            Action::Restore { .. } => "restore",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["append", "stop_jobs", "restore"]);
    assert_eq!(steps_of(&actions).len(), 1);
}
