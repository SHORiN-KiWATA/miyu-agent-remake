//! 回报到了（施工 7-2，`docs/blueprint/agents.md` 第三条，`docs/blueprint/kernel/session.md`「回报」）：子会话交来的回报
//! （命令 `Report`，记 `child.reported`），执行器交来的后台命令结束（输入 `JobEnded`，记 `job.reported`）。
//!
//! 回报不带回合编号（2026-09-30 定）：它不属于哪一轮，撤哪一轮都不会跟着拿走（别处来的留着）。回合中途到的，照它在
//! 日志里的位置和请求看到的范围排，下一次请求就在那一步的工具结果后面。
//!
//! 记下以后看叫不叫醒她：闲着的，这时开得了就由它开一轮；正忙的，排进这一轮的回报队，照排队的消息下一次请求听到，回合
//! 结束时还没听到的接着开下一轮（[`Session::finish_turn`]）。只记下的几种不叫醒；闲着时开不了的记在一边，恢复了撤销、
//! 这时开得了，由最后那条接着开（[`Session::wake_deferred`]）。

use super::action::{Action, Reason};
use super::input::Input;
use super::{Session, rejected};
use crate::event::{Body, ChildReason, ChildReported, Event, JobReason, JobReported};
use crate::id::{CommandId, JobId, Seq};
use crate::ledger::LedgerError;
use crate::origin::By;
use crate::time::Timestamp;

/// 一条会叫醒她的回报：序号、它的 `cause`、哪个任务。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Arrived {
    pub(super) seq: Seq,
    pub(super) cause: Option<CommandId>,
    pub(super) job: JobId,
}

impl Session {
    /// 子会话交来的回报：记一条 `child.reported`，`by` 是发命令的子会话，`cause` 是这个命令；落了盘回应，附上它的序号。
    /// 对不上一个还会报的子代理的（账本的几条），拒绝，`unknown_job`，什么都不记。
    pub(super) fn report(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        reported: ChildReported,
    ) -> Vec<Action> {
        let body = Body::ChildReported(reported);
        let Ok(events) = self.arrive(at, by, Some(id.clone()), body) else {
            return vec![rejected(id, Reason::UnknownJob)];
        };
        self.accept(id, vec![events[0].seq]);
        vec![Action::Append(events)]
    }

    /// 执行器交来的后台命令结束：记一条 `job.reported`，`by`、`cause` 照交来的。对不上一个还没结束的后台命令的，不理。
    /// 读回日志的时候到的先放着，读回来记了撤销再记：读回的那一段要连到追加过的最后一条（`revert.rs`）。
    pub(super) fn job_ended(
        &mut self,
        at: Timestamp,
        by: By,
        cause: Option<CommandId>,
        reported: JobReported,
    ) -> Vec<Action> {
        if let Some(reading) = self.reading.as_mut() {
            reading.later.push(Input::JobEnded {
                at,
                by,
                cause,
                reported,
            });
            return Vec::new();
        }
        match self.arrive(at, by, cause, Body::JobReported(reported)) {
            Ok(events) => vec![Action::Append(events)],
            Err(_) => Vec::new(),
        }
    }

    /// 记下一条回报，看叫不叫醒她。交回追加的事件：回报那一条，和由它开的那一轮的开头。过不了账本的什么都不记。
    fn arrive(
        &mut self,
        at: Timestamp,
        by: By,
        cause: Option<CommandId>,
        body: Body,
    ) -> Result<Vec<Event>, LedgerError> {
        let job = job_of(&body);
        let wakes = wakes(&body);
        let event = Event {
            seq: self.ledger.next_seq(),
            at,
            turn: None,
            by,
            cause,
            body,
        };
        self.commit(&event)?;
        let (Some(job), true) = (job, wakes) else {
            return Ok(vec![event]);
        };
        let arrived = Arrived {
            seq: event.seq,
            cause: event.cause.clone(),
            job,
        };
        let mut events = vec![event];
        let hidden = self.hidden(job);
        if let Some(turn) = self.turn.as_mut() {
            if !hidden {
                turn.reports.push(arrived);
            }
        } else if !hidden && self.can_wake() {
            events.extend(self.open_turn(at, arrived.seq, arrived.cause));
        } else {
            self.deferred.push(arrived);
        }
        Ok(events)
    }

    /// 记在一边的回报，这时开得了、派它的那一轮还在的，由最后那条开一轮（「回报」第 7 条）：恢复了撤销以后。有回合在
    /// 进行、这时开不了、没有这样的，什么都不做。交回追加的事件。
    pub(super) fn wake_deferred(&mut self, at: Timestamp) -> Vec<Event> {
        if self.turn.is_some() || !self.can_wake() {
            return Vec::new();
        }
        let Some(last) = self
            .deferred
            .iter()
            .rev()
            .find(|arrived| !self.hidden(arrived.job))
            .cloned()
        else {
            return Vec::new();
        };
        self.open_turn(at, last.seq, last.cause)
    }

    /// 没人看着的一次性会话：`miyu ask` 开的，这时没有头订阅着。回报只记下，等人开口（2026-09-29 项目主人定）。
    pub(super) fn unwatched(&self) -> bool {
        self.oneshot && !self.watched
    }

    /// 闲着时回报这时开得了一轮：能恢复撤销的时候不开，恢复了或者人说了下一句再说（`02-内核.md` 第六节「撤销与恢复」）；
    /// 正在改回文件的时候不开；没人看着的一次性会话不开。读回日志的时候回报到不了这里：后台命令结束先放着，子会话的回报
    /// 照别的命令拒绝。
    fn can_wake(&self) -> bool {
        !self.unwatched() && self.ledger.last_reverted().is_none() && self.restoring.is_none()
    }

    /// 派它的那一轮撤掉了：回报不渲染，也不叫醒她（`agents.md` 第七条第 2 条）。没派过的一样。
    fn hidden(&self, job: JobId) -> bool {
        self.history
            .dispatched(job)
            .is_none_or(|dispatched| dispatched.undone)
    }
}

/// 这条回报叫不叫醒她（`agents.md` 第三条第 3 条）：她自己用 `jobs` 停的、撤销停掉的、重启停掉的、崩了的，只记下；别的
/// 叫醒，被人停掉的子代理也叫醒，她不会白等。不认识的原因叫醒：她至少知道结束了。
pub(super) fn wakes(body: &Body) -> bool {
    match body {
        Body::JobReported(reported) => match reported.reason {
            JobReason::Exited | JobReason::Other(_) => true,
            JobReason::Stopped => !reported.by_model,
            JobReason::Undone | JobReason::Restarted | JobReason::Aborted => false,
        },
        Body::ChildReported(reported) => {
            !matches!(reported.reason, ChildReason::Undone | ChildReason::Aborted)
        }
        _ => false,
    }
}

/// 回报说的是哪个任务；别的事件没有。
pub(super) fn job_of(body: &Body) -> Option<JobId> {
    match body {
        Body::JobReported(reported) => Some(reported.job),
        Body::ChildReported(reported) => Some(reported.job),
        _ => None,
    }
}
