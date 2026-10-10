//! 旁白（`docs/blueprint/view.md`「`notice` 的 `what`」）：压缩、清空、后台任务了结、回顾、换模型、换工作区、等的会话
//! 怎么了。压缩那一条照「替代到的那一条」编号（`c<序号>`）：流式时进度先开出来，落了盘的检查点接着改它；一轮结束
//! 时还在压的（被打断了）拿掉，翻页本来就没有它。

use miyu_kernel::event::{
    ChildReason, ChildReported, CompactTrigger, CompactionDone, CompactionPaused,
    CompactionProgress, ContextCompacted, Event, JobReason, JobReported, ModelCalled, ModelChanged,
    PeerIdle, PolicyChanged, SessionRecapped, Usage, WorkspaceChanged,
};
use miyu_kernel::id::{JobId, Seq};
use miyu_kernel::time::Timestamp;

use super::Projector;
use super::users::level;
use crate::entry::{Body, EntryId, JobDone, JobEnd, Mark, ModelSwap, Output, Peer, Was};
use crate::explain::explain;
use crate::notice::{Compaction, CompactionState, Notice};

/// 内核在摘要请求调了工具、接着改走隔离式时，原话末尾写的（`compaction.md` 第三条第 7 条）：不是失败。
const ISOLATING: &str = "trying again without tools";

/// 压缩那一条的编号：`c<替代到的那一条>`。
fn compaction_id(upto: Seq) -> EntryId {
    EntryId::compaction(upto)
}

impl Projector {
    /// 换了策略：记下权限级别；钉着的模型没了、退回默认的，另起一条。
    pub(super) fn policy(&mut self, event: &Event, changed: &PolicyChanged) {
        if let Some(permission) = &changed.permission {
            self.level = Some(level(permission));
        }
        let (Some(replaced), Some(model)) = (&changed.replaced, &changed.model) else {
            return;
        };
        self.notice(
            EntryId::event(event.seq),
            event.at,
            Notice::Model(ModelSwap {
                endpoint: None,
                model: model.clone(),
                from: Some(Was {
                    endpoint: None,
                    model: replaced.clone(),
                }),
                why: "replaced",
                class: None,
            }),
        );
    }

    /// 出错换了模型（瞬时的）：只在视图流里有。
    pub(super) fn model_changed(&mut self, at: Timestamp, changed: &ModelChanged) {
        if changed.why.as_str() != "failover" {
            return;
        }
        let Some(model) = &changed.model else {
            return;
        };
        let class = self
            .turn
            .as_ref()
            .and_then(|t| t.failure.as_ref())
            .map(|e| e.class.as_str().to_string());
        let from = self.model.1.as_ref().map(|m| Was {
            endpoint: self.model.0.as_ref().map(|e| e.as_str().to_string()),
            model: m.as_str().to_string(),
        });
        let id = self.fresh_transient();
        self.notice(
            id,
            at,
            Notice::Model(ModelSwap {
                endpoint: changed.endpoint.as_ref().map(|e| e.as_str().to_string()),
                model: model.as_str().to_string(),
                from,
                why: "failover",
                class,
            }),
        );
    }

    /// 换了工作区。
    pub(super) fn workspace(&mut self, event: &Event, changed: &WorkspaceChanged) {
        self.notice(
            EntryId::event(event.seq),
            event.at,
            Notice::Workspace {
                cwd: changed.cwd.clone(),
                dirs: changed.dirs.clone(),
            },
        );
    }

    /// 一段回顾：讲到的最后一轮是 `upto` 所在的那一轮。
    pub(super) fn recap(&mut self, event: &Event, recap: &SessionRecapped) {
        let covers = self
            .turns
            .iter()
            .rev()
            .find(|turn| turn.started() <= recap.upto)
            .copied();
        self.notice(
            EntryId::event(event.seq),
            event.at,
            Notice::Recap {
                text: recap.text.clone(),
                covers,
            },
        );
    }

    /// 等的会话怎么了。
    pub(super) fn peer(&mut self, event: &Event, idle: &PeerIdle) {
        self.notice(
            EntryId::event(event.seq),
            event.at,
            Notice::Peer(Peer {
                session: idle.session.clone(),
                reason: idle.reason.clone(),
                status: idle.status.clone(),
            }),
        );
    }

    /// 摘要写到哪了（瞬时的）：在压的那一条没有就开出来，有了就改进度；自动的还是手动的照进度带的（施工 6-11 再补）。
    pub(super) fn progress(&mut self, at: Timestamp, progress: &CompactionProgress) {
        let id = compaction_id(progress.seen);
        if self.at.contains_key(&id) {
            self.touch(&id, |entry| {
                if let Body::Notice(Notice::Compaction(c)) = &mut entry.body {
                    c.trigger = Some(progress.trigger.clone());
                    c.written = Some(progress.written);
                    c.expected = Some(progress.expected);
                }
            });
        } else {
            self.notice(
                id.clone(),
                at,
                Notice::Compaction(Compaction {
                    trigger: Some(progress.trigger.clone()),
                    written: Some(progress.written),
                    expected: Some(progress.expected),
                    seen: Some(progress.seen.get()),
                    ..running()
                }),
            );
        }
        self.compacting = Some(id);
    }

    /// 压好了（瞬时的）：前后的用量、提前压好的。检查点落了盘时已经开出那一条。
    pub(super) fn compaction_done(&mut self, done: &CompactionDone) {
        let id = compaction_id(done.seen);
        self.touch(&id, |entry| {
            if let Body::Notice(Notice::Compaction(c)) = &mut entry.body {
                c.before = Some(done.before);
                c.after = Some(done.after);
                c.prepared = done.prepared;
                if c.usage.is_none() {
                    c.usage.clone_from(&done.usage);
                }
                if c.took_ms.is_none() {
                    c.took_ms = done.duration_ms;
                }
            }
        });
    }

    /// 检查点落了盘：压好了；清空的另起一条「清空了」。
    pub(super) fn compacted(&mut self, event: &Event, compacted: &ContextCompacted) {
        if compacted.trigger == Some(CompactTrigger::Clear) {
            self.notice(EntryId::event(event.seq), event.at, Notice::Cleared {});
            return;
        }
        let id = compaction_id(compacted.upto);
        if !self.at.contains_key(&id) {
            self.notice(id.clone(), event.at, Notice::Compaction(running()));
        }
        let prepared = std::mem::take(&mut self.prepared)
            .remove(&compacted.upto)
            .filter(|_| compacted.prepared);
        self.touch(&id, |entry| {
            if let Body::Notice(Notice::Compaction(c)) = &mut entry.body {
                c.state = CompactionState::Done;
                c.trigger.clone_from(&compacted.trigger);
                // 换上的是提前压好的（施工 6-11 三补）：日志里记着，翻页也认得出；用量、用时是后台那一次的。
                c.prepared |= compacted.prepared;
                if let Some((usage, took)) = &prepared {
                    if c.usage.is_none() {
                        c.usage.clone_from(usage);
                    }
                    if c.took_ms.is_none() {
                        c.took_ms = *took;
                    }
                }
                c.instructions.clone_from(&compacted.instructions);
                c.written = None;
                c.expected = None;
                c.seen = None;
            }
        });
        if self.compacting.as_ref() == Some(&id) {
            self.compacting = None;
        }
    }

    /// 摘要请求记下了：用量、用时记在那一条上；出错的是失败了（改走隔离式的不算）。
    pub(super) fn compaction_called(&mut self, event: &Event, called: &ModelCalled) {
        let id = compaction_id(called.seen);
        let isolating = called
            .error
            .as_ref()
            .is_some_and(|e| e.class.as_str() == "bad_summary" && e.message.ends_with(ISOLATING));
        let failed = called.error.is_some() && !isolating;
        // 翻页时摘要请求的记录先于检查点到：那一条在这里开出来。开始的时刻照摘要请求发出去的时刻，流式、翻页一样。
        if !self.at.contains_key(&id) {
            self.notice(id.clone(), event.at, Notice::Compaction(running()));
        }
        let duration = i64::try_from(called.duration_ms.unwrap_or(0)).unwrap_or(0);
        let start = Timestamp::from_unix_millis(event.at.unix_millis() - duration);
        let explain_text = called
            .error
            .as_ref()
            .filter(|_| failed)
            .map(|e| explain(e, self.texts.local.as_ref()));
        self.touch(&id, |entry| {
            if let Some(start) = start {
                entry.at = start;
            }
            if let Body::Notice(Notice::Compaction(c)) = &mut entry.body {
                c.usage.clone_from(&called.usage);
                c.took_ms = called.duration_ms;
                if failed {
                    c.state = CompactionState::Failed;
                    c.error.clone_from(&called.error);
                    c.explain = explain_text;
                    c.written = None;
                    c.expected = None;
                    c.seen = None;
                }
            }
        });
        if failed && self.compacting.as_ref() == Some(&id) {
            self.compacting = None;
        }
    }

    /// 暂停了自动压缩：照原样另起一条。
    pub(super) fn paused(&mut self, event: &Event, paused: &CompactionPaused) {
        self.notice(
            EntryId::event(event.seq),
            event.at,
            Notice::Paused {
                reason: paused.reason.clone(),
                failures: paused.failures,
                entry: paused.entry,
            },
        );
    }

    /// 一轮结束时还在压的：被打断了，拿掉（翻页本来就没有它）。
    pub(super) fn drop_compacting(&mut self) {
        if let Some(id) = self.compacting.take() {
            self.remove(&id);
        }
    }

    /// 手动压缩、清空那一轮结束了：不另起收尾，用时、用量接在这一轮最后那一条压缩上。
    pub(super) fn manual_ended(&mut self, took: u64, usage: Option<Usage>) {
        let last = self
            .entries
            .iter()
            .rev()
            .find(|e| matches!(e.body, Body::Notice(Notice::Compaction(_))))
            .map(|e| e.id.clone());
        let Some(id) = last else {
            return;
        };
        self.touch(&id, |entry| {
            if let Body::Notice(Notice::Compaction(c)) = &mut entry.body {
                c.took_ms = Some(took);
                if usage.is_some() {
                    c.usage = usage;
                }
            }
        });
    }

    /// 后台命令结束了。叫撤销停掉的记在撤销那一条上，不另起。
    pub(super) fn job_reported(&mut self, event: &Event, reported: &JobReported) {
        if reported.reason == JobReason::Undone {
            self.stopped_by_undo(&reported.job);
            return;
        }
        let mark = match reported.reason {
            JobReason::Exited if reported.exit_code == Some(0) => Mark::Done,
            JobReason::Exited => Mark::Failed,
            _ => Mark::Stopped,
        };
        let job = self.jobs.get(&reported.job).cloned();
        let output = reported.output.clone().map(|blob| Output {
            blob,
            chars: reported.chars,
        });
        self.notice(
            EntryId::event(event.seq),
            event.at,
            Notice::Job(JobDone {
                job: reported.job.clone(),
                job_kind: job
                    .as_ref()
                    .map_or(miyu_kernel::event::JobKind::Command, |j| j.what.clone()),
                title: job.as_ref().map(|j| j.title.clone()).unwrap_or_default(),
                mark,
                reason: JobEnd::Command(reported.reason.clone()),
                report: None,
                command: job.and_then(|j| j.command),
                output,
                exit_code: reported.exit_code,
                signal: reported.signal,
                took_ms: reported.duration_ms,
            }),
        );
    }

    /// 子代理回报了。叫撤销停掉的记在撤销那一条上，不另起。
    pub(super) fn child_reported(&mut self, event: &Event, reported: &ChildReported) {
        if reported.reason == ChildReason::Undone {
            self.stopped_by_undo(&reported.job);
            return;
        }
        let mark = match reported.reason {
            ChildReason::Done => Mark::Done,
            _ => Mark::Stopped,
        };
        let title = self
            .jobs
            .get(&reported.job)
            .map(|j| j.title.clone())
            .unwrap_or_default();
        self.notice(
            EntryId::event(event.seq),
            event.at,
            Notice::Job(JobDone {
                job: reported.job.clone(),
                job_kind: miyu_kernel::event::JobKind::Agent,
                title,
                mark,
                reason: JobEnd::Agent(reported.reason.clone()),
                report: Some(reported.text.clone()),
                command: None,
                output: None,
                exit_code: None,
                signal: None,
                took_ms: None,
            }),
        );
    }

    /// 撤销停掉的任务记在最近一条撤销说明上。
    fn stopped_by_undo(&mut self, job: &JobId) {
        let Some(id) = self.reverted.clone() else {
            return;
        };
        let job = job.clone();
        self.touch(&id, |entry| {
            if let Body::Notice(Notice::Reverted { jobs, .. }) = &mut entry.body
                && !jobs.contains(&job)
            {
                jobs.push(job);
            }
        });
    }
}

/// 在压的那一条，什么都还没有。
fn running() -> Compaction {
    Compaction {
        trigger: None,
        instructions: None,
        state: CompactionState::Running,
        written: None,
        expected: None,
        seen: None,
        prepared: false,
        before: None,
        after: None,
        took_ms: None,
        usage: None,
        error: None,
        explain: None,
    }
}
