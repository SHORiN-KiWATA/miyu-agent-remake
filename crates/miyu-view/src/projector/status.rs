//! 会话状态（施工 9-8 补上，`docs/blueprint/view.md`「会话状态」）：喂事件时顺手记下在等什么、正在做什么、上下文用了多少、
//! 最近一次的速度、都在冷却到几时、任务表；[`Projector::status`] 照它们和在跑的这一轮交出整份。

use miyu_kernel::accumulate::Kind;
use miyu_kernel::event::{
    Body as EventBody, CallResult, ChildReason, ChildReported, Effect, ErrorClass, Event,
    JobReason, JobReported, ModelCalled, Piece, Transient, TransientBody,
};
use miyu_kernel::id::{CallId, JobId};
use miyu_kernel::time::Timestamp;

use super::Projector;
use crate::entry::EntryId;
use crate::status::{Doing, FINISHED_KEPT, JobRow, JobState, Speed, State, Status, Wait, Waiting};

/// 记着的会话状态。
#[derive(Debug, Default)]
pub(super) struct Tracking {
    waiting: Vec<Waiting>,
    doing: Option<Doing>,
    used: Option<u64>,
    speed: Option<Speed>,
    cooling_until: Option<Timestamp>,
    jobs: Vec<JobRow>,
}

impl Projector {
    /// 这一刻投影算得出的会话状态。
    #[must_use]
    pub fn status(&self) -> Status {
        let tracking = &self.tracking;
        let state = match (tracking.waiting.is_empty(), &self.turn) {
            (false, _) => State::Waiting,
            (true, Some(_)) => State::Running,
            (true, None) => State::Idle,
        };
        Status {
            state,
            waiting: tracking.waiting.clone(),
            since: self.turn.as_ref().map(|turn| turn.started),
            doing: self.turn.as_ref().and(tracking.doing.clone()),
            used: tracking.used,
            speed: tracking.speed,
            cooling_until: tracking.cooling_until,
            jobs: tracking.jobs.clone(),
        }
    }

    /// 一条落了盘的事件喂完以后：记下它改了的状态。
    pub(super) fn track(&mut self, event: &Event) {
        match &event.body {
            EventBody::ApprovalRequested(requested) => {
                self.wait(Wait::Approve, requested.call_id);
            }
            EventBody::QuestionAsked(asked) => self.wait(Wait::Ask, asked.call_id),
            EventBody::ApprovalDecided(decided) => self.settled(decided.call_id),
            EventBody::QuestionAnswered(answered) => self.settled(answered.call_id),
            EventBody::ToolResult(result) => {
                self.settled(result.call_id);
                self.track_effects(event.at, &result.effects);
            }
            EventBody::JobReported(reported) => self.job_ended(event.at, reported),
            EventBody::ChildReported(reported) => self.child_ended(event.at, reported),
            EventBody::TurnStarted(_) => self.tracking.doing = None,
            EventBody::TurnEnded(_) => {
                self.tracking.waiting.clear();
                self.tracking.doing = None;
            }
            EventBody::ModelCalled(called) => self.track_called(called),
            EventBody::ContextCompacted(_) => {
                self.tracking.used = None;
                if matches!(self.tracking.doing, Some(Doing::Compacting { .. })) {
                    self.tracking.doing = None;
                }
            }
            _ => {}
        }
    }

    /// 一条瞬时的事件喂完以后：正在做什么、压完用了多少、都在冷却到几时。
    pub(super) fn track_transient(&mut self, transient: &Transient) {
        match &transient.body {
            TransientBody::ModelDelta(delta) => {
                if let Piece::Start(kind) = &delta.piece {
                    let entry = EntryId::block(delta.seen, delta.index);
                    self.tracking.doing = Some(match kind {
                        Kind::Text => Doing::Writing { entry },
                        Kind::Reasoning => Doing::Thinking { entry },
                        Kind::ToolCall { .. } => Doing::Tool { entry },
                    });
                }
            }
            TransientBody::Status(status) => {
                let retry = &status.retry;
                let at = later(transient.at, retry.wait_ms);
                if retry.class == ErrorClass::Cooling {
                    self.tracking.cooling_until = Some(at);
                }
                self.tracking.doing = Some(Doing::Retrying {
                    attempt: retry.attempt,
                    limit: retry.limit,
                    at,
                    class: retry.class.clone(),
                    message: retry.message.clone(),
                    status: retry.status,
                    failover: retry.failover,
                });
            }
            TransientBody::CompactionProgress(progress) => {
                self.tracking.doing = Some(Doing::Compacting {
                    entry: EntryId::compaction(progress.seen),
                    written: progress.written,
                    expected: progress.expected,
                });
            }
            TransientBody::CompactionDone(done) => {
                self.tracking.used = Some(done.after);
                self.tracking.doing = None;
            }
            _ => {}
        }
    }

    /// 翻页、订阅时切点前的日志：只记任务表（派出、留言叫醒、了结），别的状态照这一页算。
    pub(super) fn learn_rows(&mut self, earlier: &[Event]) {
        for event in earlier {
            match &event.body {
                EventBody::ToolResult(result) => self.track_effects(event.at, &result.effects),
                EventBody::JobReported(reported) => self.job_ended(event.at, reported),
                EventBody::ChildReported(reported) => self.child_ended(event.at, reported),
                _ => {}
            }
        }
    }

    /// 等人的一件：那一步的条目还在才记。
    fn wait(&mut self, what: Wait, call: CallId) {
        if let Some(entry) = self.calls.get(&call).cloned()
            && !self.tracking.waiting.iter().any(|w| w.call == call)
        {
            self.tracking.waiting.push(Waiting { what, entry, call });
        }
    }

    /// 那一次调用了结了：不再等人。
    fn settled(&mut self, call: CallId) {
        self.tracking.waiting.retain(|waiting| waiting.call != call);
    }

    /// 主请求说完了：上下文用了多少、速度；说成了的不再冷却、不再等重试。
    fn track_called(&mut self, called: &ModelCalled) {
        if called.purpose.is_some() || called.compaction.is_some() {
            return;
        }
        if let Some(usage) = &called.usage {
            self.tracking.used = Some(usage.uncached + usage.cache_read + usage.cache_write);
            if let Some(duration) = called.duration_ms {
                let ms = duration.saturating_sub(called.first_token_ms.unwrap_or(0));
                self.tracking.speed = Some(Speed {
                    output: usage.output,
                    ms,
                });
            }
        }
        if called.result == CallResult::Ok {
            self.tracking.cooling_until = None;
            if matches!(self.tracking.doing, Some(Doing::Retrying { .. })) {
                self.tracking.doing = None;
            }
        }
    }

    /// 工具结果的效果：派出去的任务记一行，给子代理留了言的叫醒了、回到在跑。
    fn track_effects(&mut self, at: Timestamp, effects: &[Effect]) {
        for effect in effects {
            match effect {
                Effect::JobStarted(started) => {
                    let command = self
                        .jobs
                        .get(&started.job)
                        .and_then(|job| job.command.clone());
                    self.tracking.jobs.retain(|row| row.job != started.job);
                    self.tracking.jobs.push(JobRow {
                        job: started.job.clone(),
                        what: started.what.clone(),
                        title: started.title.clone(),
                        state: JobState::Running,
                        started: at,
                        ended: None,
                        session: started.session.clone(),
                        command,
                        exit_code: None,
                        signal: None,
                        why: None,
                    });
                }
                Effect::JobMessaged(messaged) => {
                    if let Some(row) = self.row(&messaged.job) {
                        row.state = JobState::Running;
                        row.ended = None;
                        row.why = None;
                    }
                }
                _ => {}
            }
        }
        self.trim_rows();
    }

    /// 后台命令结束了。
    fn job_ended(&mut self, at: Timestamp, reported: &JobReported) {
        let (state, why) = match reported.reason {
            JobReason::Exited if reported.exit_code == Some(0) => (JobState::Done, None),
            JobReason::Exited => (JobState::Failed, None),
            JobReason::Stopped => (JobState::Stopped, Some("stopped")),
            JobReason::Undone => (JobState::Stopped, Some("undone")),
            JobReason::Restarted => (JobState::Stopped, Some("restarted")),
            JobReason::Aborted => (JobState::Aborted, None),
            _ => (JobState::Failed, None),
        };
        if let Some(row) = self.row(&reported.job) {
            row.state = state;
            row.ended = Some(at);
            row.exit_code = reported.exit_code;
            row.signal = reported.signal;
            row.why = why;
        }
        self.trim_rows();
    }

    /// 子代理报了。
    fn child_ended(&mut self, at: Timestamp, reported: &ChildReported) {
        let (state, why) = match reported.reason {
            ChildReason::Done => (JobState::Done, None),
            ChildReason::Stopped => (JobState::Stopped, Some("stopped")),
            ChildReason::Undone => (JobState::Stopped, Some("undone")),
            ChildReason::Aborted => (JobState::Aborted, None),
            _ => (JobState::Done, None),
        };
        if let Some(row) = self.row(&reported.job) {
            row.state = state;
            row.ended = Some(at);
            row.why = why;
        }
        self.trim_rows();
    }

    fn row(&mut self, job: &JobId) -> Option<&mut JobRow> {
        self.tracking.jobs.iter_mut().find(|row| &row.job == job)
    }

    /// 做完的只留最近的几个：照结束的先后，早的先拿掉。
    fn trim_rows(&mut self) {
        let finished = self
            .tracking
            .jobs
            .iter()
            .filter(|row| !row.state.running())
            .count();
        let mut extra = finished.saturating_sub(FINISHED_KEPT);
        while extra > 0 {
            let oldest = self
                .tracking
                .jobs
                .iter()
                .enumerate()
                .filter(|(_, row)| !row.state.running())
                .min_by_key(|(_, row)| row.ended.map(Timestamp::unix_millis))
                .map(|(index, _)| index);
            match oldest {
                Some(index) => {
                    self.tracking.jobs.remove(index);
                    extra -= 1;
                }
                None => break,
            }
        }
    }
}

/// `at` 再过 `ms` 毫秒。
fn later(at: Timestamp, ms: u64) -> Timestamp {
    let ms = i64::try_from(ms).unwrap_or(i64::MAX);
    Timestamp::from_unix_millis(at.unix_millis().saturating_add(ms)).unwrap_or(at)
}
