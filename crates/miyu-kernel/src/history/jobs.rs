//! 有效历史另记的一张表：派出去过的任务（施工 7-2，`docs/blueprint/kernel/history.md`「派出去过的任务」）。
//!
//! 渲染回报要知道任务的标题、是后台命令还是子代理；派它的那一轮撤掉了的，回报不渲染、也不叫醒她
//! （`docs/blueprint/agents.md` 第七条第 2 条）。这些都在派它的那条 `tool.result` 里，可那一条会被压缩换掉、被撤销拿走，
//! 回报却可能在那以后才到，所以另记一张表：压缩不丢，撤销、恢复只改「撤掉了没有」。一个任务一项，随任务数长。

use std::collections::BTreeMap;

use crate::event::{Body, Effect, Event, JobKind};
use crate::id::{JobId, SessionId, TurnId};

/// 派出去过的任务，照编号。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Jobs(BTreeMap<JobId, Dispatched>);

/// 派出去的一个任务：渲染回报、判回报叫不叫醒她用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dispatched {
    /// 派的是什么：后台命令、子代理，不认识的原样。
    pub what: JobKind,
    /// 调用时给的标题（`job.started` 的 `title`）。
    pub title: String,
    /// 子代理的会话（`job.started` 的 `session`）；后台命令没有。子代理发来的留言照它认出是哪一个（施工 7-7）。
    pub session: Option<SessionId>,
    /// 派它的那一轮撤掉了：还能恢复的、恢复不了的都算。恢复了就不算。
    pub undone: bool,
    /// 派它的那一轮：撤销、恢复照它改 `undone`。
    turn: Option<TurnId>,
}

impl Jobs {
    /// 记下这一条带来的变化：工具结果的效果里派出去的记下；撤掉的那几轮里派的标成撤掉了，恢复的去掉这个标。
    pub(super) fn note(&mut self, event: &Event) {
        match &event.body {
            Body::ToolResult(result) => {
                for effect in &result.effects {
                    if let Effect::JobStarted(started) = effect {
                        let dispatched = Dispatched {
                            what: started.what.clone(),
                            title: started.title.clone(),
                            session: started.session.clone(),
                            undone: false,
                            turn: event.turn,
                        };
                        self.0.insert(started.job.clone(), dispatched);
                    }
                }
            }
            Body::TurnReverted(reverted) => self.mark(&reverted.turns, true),
            Body::TurnUnreverted(unreverted) => self.mark(&unreverted.turns, false),
            _ => {}
        }
    }

    /// 编号是 `job` 的那一个；没派过的没有。
    pub(super) fn get(&self, job: &JobId) -> Option<&Dispatched> {
        self.0.get(job)
    }

    /// 在会话 `session` 里跑的子代理：编号和它（施工 7-7）。不是这个会话派的子代理的没有。
    pub(super) fn in_session(&self, session: &SessionId) -> Option<(JobId, &Dispatched)> {
        self.0
            .iter()
            .find(|(_, job)| job.session.as_ref() == Some(session))
            .map(|(id, job)| (id.clone(), job))
    }

    /// 在这几轮里派的，照编号（施工 7-8：撤销停掉它们）。
    pub(super) fn in_turns<'a>(&'a self, turns: &'a [TurnId]) -> impl Iterator<Item = JobId> + 'a {
        self.0
            .iter()
            .filter(|(_, job)| job.turn.is_some_and(|turn| turns.contains(&turn)))
            .map(|(id, _)| id.clone())
    }

    /// 在这几轮里派的，标成撤掉了没有。
    fn mark(&mut self, turns: &[TurnId], undone: bool) {
        for job in self.0.values_mut() {
            if job.turn.is_some_and(|turn| turns.contains(&turn)) {
                job.undone = undone;
            }
        }
    }
}
