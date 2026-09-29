//! 账本里任务的几条（施工 7-1，`docs/blueprint/kernel/history.md`「账本查的规矩」，`agents.md`「对外的样子」）：
//! 派出去的后台命令和子代理，编号整份日志里不重复；两种回报对得上派出去的任务；子会话的 `session.created` 带着父会话
//! 和第几层。
//!
//! 编号不回收要看整份日志：账本记着每一个派出去过的任务，撤掉的回合里派的也在，撤销、恢复、压缩都不动它们。一个任务
//! 一项，账本随任务数长（`kernel/history.md`「账本」）。

use std::collections::{BTreeMap, BTreeSet};

use crate::event::{
    Body, ChildReason, ChildReported, Effect, JobKind, JobReported, JobStarted, SessionCreated,
};
use crate::id::{JobId, SessionId};
use crate::origin::{By, Session};

/// 派出去过的任务，照编号。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Jobs(BTreeMap<JobId, Job>);

/// 账本记着的一个任务：是什么，还会不会再报。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Job {
    /// 后台命令；`ended`：报过结束了，哪一种 `reason` 都算。
    Command { ended: bool },
    /// 子代理：它的子会话；`stopped`：以 `stopped`、`undone` 报过了，被停掉的不会再起来。
    Agent { session: SessionId, stopped: bool },
    /// 不认识的种类：编号占着，两种回报都对不上它。
    Other,
}

impl Jobs {
    /// 用过的最大编号：撤掉的回合里派的、不认识的种类都算，一个都没派过的是 0（施工 7-5）。执行器照它接着往下领号。
    pub(super) fn last(&self) -> u64 {
        self.0.last_key_value().map_or(0, |(job, _)| job.get())
    }

    /// 一条工具结果的效果里派出去的任务：编号没用过，同一条里也不重复；`agent` 带会话，`command` 不带，不认识的
    /// 种类不查。照效果的先后，第一个违反的报出来。
    pub(super) fn check_started(&self, effects: &[Effect]) -> Result<(), String> {
        let mut here = BTreeSet::new();
        for started in started(effects) {
            let job = started.job;
            if self.0.contains_key(&job) || !here.insert(job) {
                return Err(format!(
                    "job {job} is already taken: job ids are never reused, even after an undo"
                ));
            }
            match (&started.what, &started.session) {
                (JobKind::Agent, None) => {
                    return Err(format!("job {job} is an agent and needs session"));
                }
                (JobKind::Command, Some(_)) => {
                    return Err(format!("job {job} is a command and has no session"));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// 后台命令结束了：对得上一个派出去的后台命令，它还没报过结束。
    pub(super) fn check_reported(&self, reported: &JobReported) -> Result<(), String> {
        let job = reported.job;
        match self.0.get(&job) {
            Some(Job::Command { ended: false }) => Ok(()),
            Some(Job::Command { ended: true }) => Err(format!("job {job} has already ended")),
            _ => Err(format!(
                "job {job} is not a background command: no such job, or it is not a command"
            )),
        }
    }

    /// 子会话的回报：对得上一个派出去的子代理，会话是它记的那个，`by` 是那个子会话，它没被停掉过。
    pub(super) fn check_child(&self, reported: &ChildReported, by: &By) -> Result<(), String> {
        let job = reported.job;
        let Some(Job::Agent { session, stopped }) = self.0.get(&job) else {
            return Err(format!(
                "job {job} is not a subagent: no such job, or it is not an agent"
            ));
        };
        if reported.session != *session {
            return Err(format!(
                "job {job} runs in session {session}, not {}",
                reported.session
            ));
        }
        if !matches!(by, By::Session(Session { id }) if id == session) {
            return Err(format!(
                "child.reported for job {job} should be by session {session}"
            ));
        }
        if *stopped {
            return Err(format!(
                "job {job} was stopped or undone and cannot report again"
            ));
        }
        Ok(())
    }

    /// 还没报过结束的后台命令，照编号（施工 7-3）。
    pub(super) fn running_commands(&self) -> Vec<JobId> {
        self.0
            .iter()
            .filter(|(_, job)| matches!(job, Job::Command { ended: false }))
            .map(|(id, _)| *id)
            .collect()
    }

    /// 记下查过的这一条带来的变化：派出去的记下，后台命令报了就结束，子代理以 `stopped`、`undone` 报了就不会再报。
    pub(super) fn record(&mut self, body: &Body) {
        match body {
            Body::ToolResult(result) => {
                for started in started(&result.effects) {
                    let job = match (&started.what, &started.session) {
                        (JobKind::Command, _) => Job::Command { ended: false },
                        (JobKind::Agent, Some(session)) => Job::Agent {
                            session: session.clone(),
                            stopped: false,
                        },
                        _ => Job::Other,
                    };
                    self.0.insert(started.job, job);
                }
            }
            Body::JobReported(reported) => {
                if let Some(Job::Command { ended }) = self.0.get_mut(&reported.job) {
                    *ended = true;
                }
            }
            Body::ChildReported(reported)
                if matches!(reported.reason, ChildReason::Stopped | ChildReason::Undone) =>
            {
                if let Some(Job::Agent { stopped, .. }) = self.0.get_mut(&reported.job) {
                    *stopped = true;
                }
            }
            _ => {}
        }
    }
}

/// 效果里派出去的任务，照先后。
fn started(effects: &[Effect]) -> impl Iterator<Item = &JobStarted> {
    effects.iter().filter_map(|effect| match effect {
        Effect::JobStarted(started) => Some(started),
        _ => None,
    })
}

/// 子会话的第一条带着父会话和第几层，主会话两格都没有；第几层从 1 起：主会话是第 0 层，不写。
pub(super) fn check_created(created: &SessionCreated) -> Result<(), String> {
    match (&created.parent, created.depth) {
        (_, Some(0)) => Err("depth should be at least 1".to_string()),
        (Some(_), Some(_)) | (None, None) => Ok(()),
        _ => Err(
            "parent and depth go together: a child session has both, the main session neither"
                .to_string(),
        ),
    }
}
