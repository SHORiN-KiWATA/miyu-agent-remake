//! 这个会话派出去的任务（施工 7-4，`docs/blueprint/session/tools.md`「查和停」）：`jobs` 列出、读、停的时候查它。照日志算：
//! 载入时从全部事件建起来，之后 actor 每落一批盘跟着记（和她看过的文件一个做法）。
//!
//! 内核的账本只记着对不对得上、还会不会再报；这里多记着列出来、读输出要的几样：什么时候派的、怎么结束的、输出存在哪个
//! blob 里。一个任务一项，随任务数长。

use std::collections::BTreeMap;

use miyu_kernel::event::{Body, ChildReason, Effect, Event, JobKind};
use miyu_kernel::id::{ContentHash, JobId, Seq, SessionId};
use miyu_kernel::time::Timestamp;
use miyu_tool::Listed;

/// 列出来的时候，最近结束的最多几个（`tools/jobs.md`「list」）：还在跑的全列，结束了的只列最近的这么多个。
const RECENT: usize = 5;

/// 这个会话派出去的任务，照编号。
#[derive(Debug, Clone, Default)]
pub(crate) struct Roster(BTreeMap<JobId, Record>);

/// 派出去的一个任务。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Record {
    /// 后台命令、子代理，不认识的原样。
    pub(crate) what: JobKind,
    /// 派它时给的标题。
    pub(crate) title: String,
    /// 子代理的会话；后台命令没有。
    pub(crate) session: Option<SessionId>,
    /// 派它的那条工具结果记下的时刻。
    pub(crate) started: Timestamp,
    /// 最后那条回报；还在跑的没有。子代理报了 `done` 也算结束：它那一轮完了；之后又给它留了言的，又在跑了（施工 3-8 三补，
    /// 照 7-7 的 `job.messaged`）。
    pub(crate) end: Option<End>,
}

/// 一条回报里 `jobs` 要的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct End {
    /// 到的时刻。
    pub(crate) at: Timestamp,
    /// `reason`，照事件里的写法。
    pub(crate) reason: String,
    /// 后台命令的用时；子代理、载入时补的 `aborted` 没有。
    pub(crate) duration_ms: Option<u64>,
    /// 后台命令整份输出的 blob；没存下来的没有。
    pub(crate) output: Option<ContentHash>,
    /// 那条回报的序号：留言的调用发出以后才到的回报算回了那句留言（施工 3-8 三补，和账本一个算法）。
    pub(crate) seq: Seq,
}

impl Roster {
    /// 从日志里的全部事件建起来：载入时。
    pub(crate) fn from_events(events: &[Event]) -> Roster {
        let mut roster = Roster::default();
        for event in events {
            roster.note(event);
        }
        roster
    }

    /// 记下落了盘的一条：派出去的任务、两种回报。撤销、压缩不动它：撤掉的回合里派的照样在跑，照样列出来。
    pub(crate) fn note(&mut self, event: &Event) {
        match &event.body {
            Body::ToolResult(result) => {
                let issued = result.call_id.message();
                for effect in &result.effects {
                    match effect {
                        Effect::JobStarted(started) => {
                            let record = Record {
                                what: started.what.clone(),
                                title: started.title.clone(),
                                session: started.session.clone(),
                                started: event.at,
                                end: None,
                            };
                            self.0.insert(started.job.clone(), record);
                        }
                        // 给报过的子代理留了言（施工 7-7 的 `job.messaged`）：它欠一份回报，又在跑了，停得了、列成在跑（施工 3-8
                        // 三补：删它、停它时不漏了这一份）。留言的调用发出以后才到的回报算回了这句留言，被停掉、撤掉的不会再起来，
                        // 都和账本一样（`kernel/history.md`）。
                        Effect::JobMessaged(messaged) => {
                            if let Some(record) = self.0.get_mut(&messaged.job)
                                && record.end.as_ref().is_some_and(|end| {
                                    end.seq < issued
                                        && end.reason != ChildReason::Stopped.as_str()
                                        && end.reason != ChildReason::Undone.as_str()
                                })
                            {
                                record.end = None;
                            }
                        }
                        _ => {}
                    }
                }
            }
            Body::JobReported(reported) => self.end(
                &reported.job,
                End {
                    at: event.at,
                    reason: reported.reason.as_str().to_string(),
                    duration_ms: reported.duration_ms,
                    output: reported.output.clone(),
                    seq: event.seq,
                },
            ),
            Body::ChildReported(reported) => self.end(
                &reported.job,
                End {
                    at: event.at,
                    reason: reported.reason.as_str().to_string(),
                    duration_ms: None,
                    output: None,
                    seq: event.seq,
                },
            ),
            _ => {}
        }
    }

    /// 编号是 `job` 的那一个；没派过的没有。
    pub(crate) fn get(&self, job: &JobId) -> Option<&Record> {
        self.0.get(job)
    }

    /// 还在跑的，照编号：停下全部时用。
    pub(crate) fn running(&self) -> Vec<(JobId, Record)> {
        self.0
            .iter()
            .filter(|(_, record)| record.end.is_none())
            .map(|(job, record)| (job.clone(), record.clone()))
            .collect()
    }

    /// 列出来的（`tools/jobs.md`「list」）：还在跑的全部，和最近结束的 [`RECENT`] 个（照回报到的先后，一样的照编号），
    /// 合在一起照编号排。用时照 `now`。
    pub(crate) fn list(&self, now: Timestamp) -> Vec<Listed> {
        let mut ended: Vec<(&JobId, &Record)> = self
            .0
            .iter()
            .filter(|(_, record)| record.end.is_some())
            .collect();
        ended.sort_by_key(|(job, record)| (record.end.as_ref().map(|end| end.at), *job));
        let recent: Vec<&JobId> = ended
            .iter()
            .rev()
            .take(RECENT)
            .map(|(job, _)| *job)
            .collect();
        self.0
            .iter()
            .filter(|(job, record)| record.end.is_none() || recent.contains(job))
            .map(|(job, record)| Listed {
                job: job.clone(),
                what: record.what.clone(),
                title: record.title.clone(),
                ended: record.end.as_ref().map(|end| end.reason.clone()),
                took_ms: record.took_ms(now),
            })
            .collect()
    }

    fn end(&mut self, job: &JobId, end: End) {
        if let Some(record) = self.0.get_mut(job) {
            record.end = Some(end);
        }
    }
}

impl Record {
    /// 用时，毫秒：后台命令报了用时的照它；别的从派出去算到结束，还在跑的算到 `now`。
    fn took_ms(&self, now: Timestamp) -> u64 {
        if let Some(ms) = self.end.as_ref().and_then(|end| end.duration_ms) {
            return ms;
        }
        let until = self.end.as_ref().map_or(now, |end| end.at);
        u64::try_from(until.unix_millis() - self.started.unix_millis()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests;
