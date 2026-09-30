//! 账本里任务的几条（施工 7-1，`docs/blueprint/kernel/history.md`「账本查的规矩」，`agents.md`「对外的样子」）：
//! 派出去的后台命令和子代理，编号整份日志里不重复；两种回报对得上派出去的任务；子会话的 `session.created` 带着父会话
//! 和第几层。
//!
//! 编号不回收要看整份日志：账本记着每一个派出去过的任务，撤掉的回合里派的也在，撤销、恢复、压缩都不动它们。一个任务
//! 一项，账本随任务数长（`kernel/history.md`「账本」）。

use std::collections::{BTreeMap, BTreeSet};

use super::Ledger;
use crate::event::{
    Body, ChildReason, ChildReported, Effect, Event, JobKind, JobMessaged, JobReported, JobStarted,
    SessionCreated,
};
use crate::id::{CommandId, JobId, Seq, SessionId};
use crate::origin::{By, Session};

/// 派出去过的任务，照编号。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Jobs(BTreeMap<JobId, Job>);

/// 从账本读任务的几样（施工 7-3 起）：执行器领号、载入补报、撤销停任务都照它们。
impl Ledger {
    /// 日志里用过的任务编号最后一段最大的数（施工 7-5；照最后一段数，施工 7-1 补）：撤掉的回合里派的也算，一个都没派过的
    /// 是 0。编号不回收，执行器新派的任务从它的下一个数起（`kernel/ids.md`「任务编号」）。
    pub fn last_job_number(&self) -> u64 {
        self.jobs.last()
    }

    /// 还没报过结束的后台命令，照编号（施工 7-3：载入时给它们补 `aborted`）。
    pub fn running_commands(&self) -> Vec<JobId> {
        self.jobs.running_commands()
    }

    /// 还在跑的任务，照编号（施工 7-8）：还没报过结束的后台命令，欠着一份回报、没被停掉的子代理。撤掉的回合里派的也在。
    pub fn running_jobs(&self) -> Vec<JobId> {
        self.jobs.running()
    }

    /// 派出去、一次都还没回报过的子代理的子会话，照任务编号（施工 7-6）。
    pub fn waiting_children(&self) -> impl Iterator<Item = &SessionId> {
        self.jobs.waiting()
    }

    /// 在会话 `session` 里跑的、这个会话派的子代理的编号（施工 7-7）：被停掉的、撤掉的回合里派的也认；别的会话没有。
    pub fn subagent_in(&self, session: &SessionId) -> Option<JobId> {
        self.jobs.agent_in(session)
    }

    /// 派出去过的子代理，照编号（施工 7-7）：编号、子会话、被停掉了没有。撤掉的回合里派的也在。
    pub fn subagents(&self) -> impl Iterator<Item = (JobId, &SessionId, bool)> {
        self.jobs.agents()
    }

    /// 子代理 `job` 最近一次回报就是命令 `id` 交来的：交回那一条的序号（施工 7-6）。
    pub fn reported_as(&self, job: &JobId, id: &CommandId) -> Option<Seq> {
        self.jobs.reported_as(job, id)
    }
}

/// 账本记着的一个任务：是什么，还会不会再报。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Job {
    /// 后台命令；`ended`：报过结束了，哪一种 `reason` 都算。
    Command { ended: bool },
    /// 子代理：它的子会话；`stopped`：以 `stopped`、`undone` 报过了，被停掉的不会再起来；`last`：最近一次回报的命令编号
    /// 和序号，一次都没报过的没有（施工 7-6：还在等它的回报；子会话再交同一份回报，照它认出是重的）；`messaged`：最近一次
    /// 回报以后给它留过言，它欠一份回报（施工 7-7）。
    Agent {
        session: SessionId,
        stopped: bool,
        last: Option<(Option<CommandId>, Seq)>,
        messaged: bool,
    },
    /// 不认识的种类：编号占着，两种回报都对不上它。
    Other,
}

impl Jobs {
    /// 用过的编号最后一段最大的数：撤掉的回合里派的、不认识的种类都算，一个都没派过的是 0（施工 7-5）。执行器照它接着往下
    /// 领号。一个一个看，不取排在最后的那个（施工 7-1 补）：子会话的编号带着前缀，以前的日志里又有不带的，`j5.8` 照整个
    /// 编号排在 `j7` 前面，数的却是 8。
    pub(super) fn last(&self) -> u64 {
        self.0.keys().map(JobId::last).max().unwrap_or(0)
    }

    /// 一条工具结果的效果：留了言的（施工 7-7）先查，对得上这个会话派的一个子代理；再查派出去的任务，编号没用过，同一条
    /// 里也不重复，`agent` 带会话，`command` 不带，不认识的种类不查。照效果的先后，第一个违反的报出来。
    pub(super) fn check_started(&self, effects: &[Effect]) -> Result<(), String> {
        for messaged in messaged(effects) {
            let job = &messaged.job;
            if !matches!(self.0.get(job), Some(Job::Agent { .. })) {
                return Err(format!(
                    "job {job} was messaged but is not a subagent: no such job, or it is not an agent"
                ));
            }
        }
        let mut here = BTreeSet::new();
        for started in started(effects) {
            let job = &started.job;
            if self.0.contains_key(job) || !here.insert(job) {
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
        let job = &reported.job;
        match self.0.get(job) {
            Some(Job::Command { ended: false }) => Ok(()),
            Some(Job::Command { ended: true }) => Err(format!("job {job} has already ended")),
            _ => Err(format!(
                "job {job} is not a background command: no such job, or it is not a command"
            )),
        }
    }

    /// 子会话的回报：对得上一个派出去的子代理，会话是它记的那个，`by` 是那个子会话，它没被停掉过。
    pub(super) fn check_child(&self, reported: &ChildReported, by: &By) -> Result<(), String> {
        let job = &reported.job;
        let Some(Job::Agent {
            session, stopped, ..
        }) = self.0.get(job)
        else {
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
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// 还在跑的任务，照编号（施工 7-8）：还没报过结束的后台命令，和欠着一份回报、没被停掉的子代理（一次都没报过的，报过以后
    /// 又被留了言的）。撤销停哪几个、撤销的回应列哪几个，都照它（`agents.md` 第七条第 1 条）。
    pub(super) fn running(&self) -> Vec<JobId> {
        self.0
            .iter()
            .filter(|(_, job)| match job {
                Job::Command { ended } => !ended,
                Job::Agent {
                    stopped,
                    last,
                    messaged,
                    ..
                } => !stopped && (last.is_none() || *messaged),
                Job::Other => false,
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// 欠着一份回报的子代理的子会话，照编号：派出去一次都还没回报过的（施工 7-6），和最近一次回报以后又给它留过言的（施工
    /// 7-7）。派了孙代理的子会话等它们都报完再向上报；载入以后执行器把它们叫起来，崩了的补报（`agents.md` 第八条）。被停掉的
    /// 报过了，不在里面。
    pub(super) fn waiting(&self) -> impl Iterator<Item = &SessionId> {
        self.0.values().filter_map(|job| match job {
            Job::Agent {
                session,
                last,
                messaged,
                ..
            } if last.is_none() || *messaged => Some(session),
            _ => None,
        })
    }

    /// 在会话 `session` 里跑的子代理的编号（施工 7-7）：子会话发来的留言照发命令的会话认出是哪一个。被停掉的也认，撤掉的
    /// 回合里派的也认；不是这个会话派的子代理的没有。
    pub(super) fn agent_in(&self, session: &SessionId) -> Option<JobId> {
        self.0.iter().find_map(|(job, known)| match known {
            Job::Agent { session: own, .. } if own == session => Some(job.clone()),
            _ => None,
        })
    }

    /// 派出去过的子代理，照编号（施工 7-7）：编号、子会话、被停掉了没有（以 `stopped`、`undone` 报过）。撤掉的回合里派的也在。
    pub(super) fn agents(&self) -> impl Iterator<Item = (JobId, &SessionId, bool)> {
        self.0.iter().filter_map(|(job, known)| match known {
            Job::Agent {
                session, stopped, ..
            } => Some((job.clone(), session, *stopped)),
            _ => None,
        })
    }

    /// 子代理 `job` 最近一次回报就是命令 `id` 交来的：交回那一条的序号（施工 7-6）。子会话载入时再交一次它最后报的那一份，
    /// 父会话照它认出是重的，不再记。
    pub(super) fn reported_as(&self, job: &JobId, id: &CommandId) -> Option<Seq> {
        match self.0.get(job) {
            Some(Job::Agent {
                last: Some((Some(cause), seq)),
                ..
            }) if cause == id => Some(*seq),
            _ => None,
        }
    }

    /// 记下查过的这一条带来的变化：派出去的记下，留了言的子代理欠一份回报（施工 7-7），后台命令报了就结束，子代理报了记下
    /// 是哪一条、不再欠，以 `stopped`、`undone` 报了就不会再报。
    pub(super) fn record(&mut self, event: &Event) {
        match &event.body {
            Body::ToolResult(result) => {
                for started in started(&result.effects) {
                    let job = match (&started.what, &started.session) {
                        (JobKind::Command, _) => Job::Command { ended: false },
                        (JobKind::Agent, Some(session)) => Job::Agent {
                            session: session.clone(),
                            stopped: false,
                            last: None,
                            messaged: false,
                        },
                        _ => Job::Other,
                    };
                    self.0.insert(started.job.clone(), job);
                }
                // 留言送到、这次调用的结果还没记下，它就做完报上来了（这次调用发出以后到的回报）：算回了这句留言，不再等。
                // 不这样，父会话会一直等一份不会再来的回报；这样错的一边只是早报一次（施工 7-7）。
                let issued = result.call_id.message();
                for message in messaged(&result.effects) {
                    if let Some(Job::Agent { messaged, last, .. }) = self.0.get_mut(&message.job) {
                        *messaged = last.as_ref().is_none_or(|(_, seq)| *seq < issued);
                    }
                }
            }
            Body::JobReported(reported) => {
                if let Some(Job::Command { ended }) = self.0.get_mut(&reported.job) {
                    *ended = true;
                }
            }
            Body::ChildReported(reported) => {
                if let Some(Job::Agent {
                    stopped,
                    last,
                    messaged,
                    ..
                }) = self.0.get_mut(&reported.job)
                {
                    *last = Some((event.cause.clone(), event.seq));
                    *messaged = false;
                    *stopped |=
                        matches!(reported.reason, ChildReason::Stopped | ChildReason::Undone);
                }
            }
            _ => {}
        }
    }
}

/// 效果里给子代理留的言，照先后（施工 7-7）。
fn messaged(effects: &[Effect]) -> impl Iterator<Item = &JobMessaged> {
    effects.iter().filter_map(|effect| match effect {
        Effect::JobMessaged(messaged) => Some(messaged),
        _ => None,
    })
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
