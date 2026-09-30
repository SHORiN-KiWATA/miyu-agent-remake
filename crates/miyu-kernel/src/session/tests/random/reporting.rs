//! 随机测试里的回报（施工 7-2）：子会话交来的回报、执行器交来的后台命令结束、有没有头订阅着，另用一串随机数送，夹在原来
//! 的输入之间、不占名额：原来那串输入不跟着错开。七个种子里有一个造一次性的会话，没人看着时回报只记下。

use super::*;
use crate::event::{ChildReason, ChildReported, JobReason, JobReported};
use crate::id::{JobId, SessionId};
use crate::origin::{Session as Child, Tool};

/// 造好、第 1 条落了盘的会话；`oneshot` 的是一次性的。
pub(super) fn opened(policy: Policy, oneshot: bool) -> Session {
    let mut created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    created.oneshot = oneshot;
    let (mut session, _) = Session::create(
        session_id(),
        id(0),
        alice(),
        at(0),
        created,
        policy,
        environment("~/src/miyu"),
    );
    session.handle(stored(1));
    session
}

/// 回报，另用一串随机数。派出去的任务少（三百例里二十来个），有还没结束的时候多送：闲着三回里一回，正忙八回里一回；
/// 没有的三十回里一回（多半对不上）。八回里一回交有没有头订阅着（三回里两回有），三回后台命令结束，四回子会话的回报。
/// 后台命令结束不在改回文件、读回日志的时候送：执行器做完那一件才收收件箱（`kernel/session.md`「回报」第 9 条）。
pub(super) fn some_report(rng: &mut Rng, watch: &Watch, next_id: &mut u64) -> Option<Input> {
    let live = watch.reports.jobs.values().any(|job| !job.over);
    let chance = match (live, watch.turn_open()) {
        (true, false) => 3,
        (true, true) => 8,
        (false, _) => 30,
    };
    if rng.below(chance) != 0 {
        return None;
    }
    let busy = watch.restoring.pending.is_some() || watch.undo.reading.is_some();
    match rng.below(8) {
        0 => Some(Input::Watched {
            watched: rng.below(3) > 0,
        }),
        1..=3 if !busy => Some(job_ended(rng, watch)),
        _ => Some(child_reports(rng, watch, next_id)),
    }
}

/// 后台命令结束：多半是还没结束的一个，偶尔是结束过的、子代理、没派过的（该不理）。原因照份数抽：自己退出 4、她自己停的
/// 1、人停的 1、撤销 1、重启 1、崩了 1；`by` 照原因。
fn job_ended(rng: &mut Rng, watch: &Watch) -> Input {
    let (job, known) = pick(rng, watch, |job| job.session.is_none() && !job.over);
    let started = known.map(|job| job.call);
    let (reason, by_model, by) = match rng.below(9) {
        0..=3 => (
            JobReason::Exited,
            false,
            started.map(tool).unwrap_or(By::Kernel),
        ),
        4 => (JobReason::Stopped, true, tool(some_call(rng, watch))),
        5 => (JobReason::Stopped, false, alice()),
        6 => (JobReason::Undone, false, alice()),
        7 => (JobReason::Restarted, false, By::Kernel),
        _ => (JobReason::Aborted, false, By::Kernel),
    };
    let exited = reason == JobReason::Exited;
    Input::JobEnded {
        at: at(51),
        by,
        cause: (rng.below(2) == 0).then(|| id(1)),
        reported: JobReported {
            job,
            reason,
            exit_code: exited.then(|| rng.below(3) as i32 - 1),
            signal: None,
            by_model,
            duration_ms: Some(rng.below(100_000)),
            output: exited.then(|| ContentHash::of(b"output")),
            chars: exited.then(|| rng.below(1000)),
        },
    }
}

/// 子会话的回报：多半是还没停掉的子代理、由它的会话交，偶尔对不上（该拒）。原因照份数抽：做完了 4、人停掉 1、她自己用
/// `jobs` 停掉 1（施工 7-4）、撤销 1、崩了 1。
fn child_reports(rng: &mut Rng, watch: &Watch, next_id: &mut u64) -> Input {
    let (job, known) = pick(rng, watch, |job| job.session.is_some() && !job.over);
    let fallback = || SessionId::parse("01a0d78c-ca52-7d19-8b64-00000000ffff").unwrap();
    let session = known
        .and_then(|job| job.session.clone())
        .unwrap_or_else(fallback);
    let by = match rng.below(10) {
        0 => alice(),
        _ => By::Session(Child {
            id: session.clone(),
        }),
    };
    let (reason, by_model) = match rng.below(8) {
        0..=3 => (ChildReason::Done, false),
        4 => (ChildReason::Stopped, false),
        5 => (ChildReason::Stopped, true),
        6 => (ChildReason::Undone, false),
        _ => (ChildReason::Aborted, false),
    };
    Input::Command(Received {
        id: id(next_command(next_id)),
        by,
        at: at(52),
        command: Command::Report(ChildReported {
            job,
            session,
            reason,
            text: ["", "done", "line\n"][rng.below(3) as usize].to_string(),
            truncated: rng.below(4) == 0,
            person: rng.below(4) == 0,
            by_model,
        }),
    })
}

/// 挑一个任务：五回里四回挑合 `fits` 的（有的话），不然随便一个派过的，再不然一个没派过的编号。
fn pick<'a>(
    rng: &mut Rng,
    watch: &'a Watch,
    fits: impl Fn(&watch::Job) -> bool,
) -> (JobId, Option<&'a watch::Job>) {
    let jobs = &watch.reports.jobs;
    let fitting: Vec<(&JobId, &watch::Job)> = jobs.iter().filter(|(_, job)| fits(job)).collect();
    let chosen = match (fitting.len(), jobs.len()) {
        (n, _) if n > 0 && rng.below(5) > 0 => Some(fitting[rng.below(n as u64) as usize]),
        (_, n) if n > 0 && rng.below(2) == 0 => jobs.iter().nth(rng.below(n as u64) as usize),
        _ => None,
    };
    match chosen {
        Some((job, known)) => (job.clone(), Some(known)),
        None => (JobId::new(999).unwrap(), None),
    }
}

/// 随便一次调用：她自己用 `jobs` 停掉的，`by` 是那次调用。
fn some_call(rng: &mut Rng, watch: &Watch) -> CallId {
    CallId::new(seq(1 + rng.below(watch.last())), 1).unwrap()
}

/// 那次调用。
fn tool(call_id: CallId) -> By {
    By::Tool(Tool { call_id })
}
