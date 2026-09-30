//! 撤销的回应里停掉的任务（施工 7-8，`docs/blueprint/protocol/undo.md`「jobs」，`agents.md` 第七条第 1 条）：撤掉的那几轮
//! 派出去、撤销那一刻还在跑的，就是内核交给执行器停的那几个（`kernel/history.md`「撤销」）。回应不等它们的回报落盘，所以
//! 照日志算撤销那一刻的：撤销以前的日志过一遍账本，还在跑的（`running_jobs`）里挑派它的调用在撤掉的那几轮里的。

use serde::Serialize;

use miyu_kernel::event::{Body, Effect, Event};
use miyu_kernel::id::{JobId, Seq, TurnId};
use miyu_kernel::ledger::Ledger;

/// 停掉的一个任务：编号、种类、标题，照 `job.started` 写。
#[derive(Debug, Serialize)]
pub(crate) struct Stopped {
    job: String,
    what: String,
    title: String,
}

/// 撤销（第 `revert` 条）停掉的：撤掉的那几轮 `turns` 里派出去、撤销那一刻还在跑的，照编号。撤销以前的日志过不了账本的
/// （坏了），交回空的：撤销本身已经成了。
pub(crate) fn stopped(log: &[Event], revert: Seq, turns: &[TurnId]) -> Vec<Stopped> {
    let mut ledger = Ledger::default();
    let before = log.iter().take_while(|event| event.seq < revert);
    for event in before.clone() {
        if ledger.append(event).is_err() {
            tracing::warn!(target: "miyu::endpoint", seq = event.seq.get(), "undo report jobs not read");
            return Vec::new();
        }
    }
    let running = ledger.running_jobs();
    let mut stopped: Vec<(JobId, Stopped)> = before
        .filter(|event| event.turn.is_some_and(|turn| turns.contains(&turn)))
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.effects.iter()),
            _ => None,
        })
        .flatten()
        .filter_map(|effect| match effect {
            Effect::JobStarted(started) if running.contains(&started.job) => Some((
                started.job.clone(),
                Stopped {
                    job: started.job.to_string(),
                    what: started.what.as_str().to_string(),
                    title: started.title.clone(),
                },
            )),
            _ => None,
        })
        .collect();
    stopped.sort_by(|(one, _), (other, _)| one.cmp(other));
    stopped.into_iter().map(|(_, stopped)| stopped).collect()
}
