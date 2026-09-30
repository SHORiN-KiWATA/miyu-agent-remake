//! 看守查回报（施工 7-2，`docs/blueprint/kernel/session.md`「回报」，`agents.md` 第三条）：
//!
//! - 对不上的：子会话的回报拒绝，`unknown_job`；后台命令结束不理；都什么都不记；
//! - 对得上的：记一条，不带回合编号，`by`、`cause` 照交来的（子会话的回报 `cause` 是那个命令）；
//! - 会叫醒她的（不是只记下的那几种，派它的那一轮还在）：闲着、这时开得了，同一批由它开一轮；闲着开不了的记在一边；正忙
//!   的排着，下一次主请求听到，回合结束时还没听到的照排队的消息接着开（`watch/queue.rs`）；
//! - 恢复了撤销、这时开得了，记在一边的里面派它的那一轮还在的，由最后那条接着开，和恢复那一条、改回文件的结局同一批；
//! - 有没有头订阅着什么都不出；载入以后当没人看着（`watch/load.rs`）。

use super::*;
use crate::event::{ChildReason, Effect, JobKind, JobReason};
use crate::id::{JobId, SessionId};
use crate::origin::Session as Child;

/// 看守记着的回报。
#[derive(Default)]
pub(in super::super) struct Reports {
    /// 这个种子造的是一次性的会话。
    pub(in super::super) oneshot: bool,
    /// 有没有头订阅着。
    pub(in super::super) watched: bool,
    /// 派出去的任务，照编号。
    pub(in super::super) jobs: BTreeMap<JobId, Job>,
    /// 这一轮里到的、会叫醒她、还没被主请求听到的。
    pub(super) pending: Vec<Seq>,
    /// 闲着时到的、会叫醒她却没开轮的：序号、任务。
    deferred: Vec<(Seq, JobId)>,
}

/// 看守记着的一个任务。
#[derive(Clone)]
pub(in super::super) struct Job {
    /// 派它的那一轮、那次调用。
    turn: TurnId,
    pub(in super::super) call: CallId,
    /// 子代理的会话；后台命令没有。
    pub(in super::super) session: Option<SessionId>,
    /// 后台命令报过结束；子代理被停掉、撤销停掉过。之后再报对不上。
    pub(in super::super) over: bool,
}

/// 送进去之前判出来的。
pub(super) enum Expect {
    /// 对不上的子会话的回报：拒绝，`unknown_job`。
    Refused,
    /// 对不上的后台命令结束：不理。
    Ignored,
    /// 对得上的：记一条，`by`、`cause` 是这样。
    Recorded(By, Option<CommandId>),
    /// 有没有头订阅着：什么都不出。
    Watched(bool),
}

impl Watch {
    /// 送进一条输入之前：回报、后台命令结束、有没有头订阅着，照规矩判出该怎样。
    pub(super) fn before_report(&self, input: &Input) -> Option<Expect> {
        match input {
            Input::Command(received) if self.fresh(&received.id) => match &received.command {
                Command::Report(reported) => {
                    let fits = self.reports.jobs.get(&reported.job).is_some_and(|job| {
                        !job.over
                            && job.session.as_ref() == Some(&reported.session)
                            && received.by
                                == By::Session(Child {
                                    id: reported.session.clone(),
                                })
                    });
                    Some(match fits {
                        true => Expect::Recorded(received.by.clone(), Some(received.id.clone())),
                        false => Expect::Refused,
                    })
                }
                _ => None,
            },
            Input::JobEnded {
                by,
                cause,
                reported,
                ..
            } => {
                let fits = self
                    .reports
                    .jobs
                    .get(&reported.job)
                    .is_some_and(|job| !job.over && job.session.is_none());
                Some(match fits {
                    true => Expect::Recorded(by.clone(), cause.clone()),
                    false => Expect::Ignored,
                })
            }
            Input::Watched { watched } => Some(Expect::Watched(*watched)),
            _ => None,
        }
    }

    /// 送进去以后：照判出来的查。记下的那一条的别的规矩在 [`Watch::report_check`] 里查。
    pub(super) fn after_report(&mut self, actions: &[Action], expect: Option<Expect>) {
        let seed = self.seed;
        match expect {
            None => {}
            Some(Expect::Refused) => {
                self.seen_paths.insert("对不上的回报被拒");
                assert!(
                    matches!(
                        actions,
                        [Action::Reply {
                            outcome: Outcome::Rejected {
                                reason: Reason::UnknownJob
                            },
                            ..
                        }]
                    ),
                    "种子 {seed}：对不上的回报应该拒绝，unknown_job：{actions:?}"
                );
            }
            Some(Expect::Ignored) => {
                self.seen_paths.insert("对不上的后台命令结束不理");
                assert!(
                    actions.is_empty(),
                    "种子 {seed}：对不上的后台命令结束：{actions:?}"
                );
            }
            Some(Expect::Watched(watched)) => {
                self.seen_paths.insert("交了有没有头订阅着");
                assert!(actions.is_empty(), "种子 {seed}：{actions:?}");
                self.reports.watched = watched;
            }
            Some(Expect::Recorded(by, cause)) => {
                let first = actions.iter().find_map(|action| match action {
                    Action::Append(events) => events.first(),
                    _ => None,
                });
                assert!(
                    first.is_some_and(|event| {
                        matches!(event.body, Body::JobReported(_) | Body::ChildReported(_))
                            && event.by == by
                            && event.cause == cause
                    }),
                    "种子 {seed}：回报记一条，by、cause 照交来的：{actions:?}"
                );
            }
        }
    }

    /// 这一批里的第 `k` 条：派出去的任务记下；回报照叫不叫醒她查它后面的那一条；主请求听到了排着的；恢复撤销、改回文件
    /// 以后，记在一边的该不该接着开。
    pub(super) fn report_check(&mut self, events: &[Event], k: usize) {
        let seed = self.seed;
        let event = &events[k];
        let next = match events.get(k + 1).map(|event| &event.body) {
            Some(Body::TurnStarted(started)) => started.trigger,
            _ => None,
        };
        match &event.body {
            Body::ToolResult(result) => {
                for effect in &result.effects {
                    if let Effect::JobStarted(started) = effect {
                        let job = Job {
                            turn: event.turn.expect("工具结果带着回合"),
                            call: result.call_id,
                            session: started.session.clone(),
                            over: false,
                        };
                        assert_eq!(started.what == JobKind::Agent, job.session.is_some());
                        self.reports.jobs.insert(started.job.clone(), job);
                    }
                }
            }
            Body::JobReported(_) | Body::ChildReported(_) => {
                assert_eq!(event.turn, None, "种子 {seed}：回报不带回合编号");
                let (job, wakes) = self.arrived(&event.body);
                let hidden = self.hidden(&job);
                let opens = next == Some(event.seq);
                if self.turn_open() {
                    assert!(!opens, "种子 {seed}：正忙时到的回报不开轮");
                    if wakes && !hidden {
                        self.seen_paths.insert("回合中途到的回报排着");
                        self.reports.pending.push(event.seq);
                    }
                } else if wakes && !hidden && self.can_wake() {
                    self.seen_paths.insert("闲着时回报开了一轮");
                    assert!(opens, "种子 {seed}：闲着时到的回报 {} 该开一轮", event.seq);
                } else {
                    assert!(!opens, "种子 {seed}：回报 {} 只记下，不开轮", event.seq);
                    self.seen_paths.insert(match (wakes, hidden) {
                        (false, _) => "只记下的回报",
                        (true, true) => "派它的那一轮撤掉了的回报不开轮",
                        (true, false) if self.undo.can_unrevert() => "能恢复撤销时回报只记下",
                        (true, false) => "没人看着只记下",
                    });
                    if wakes {
                        self.reports.deferred.push((event.seq, job));
                    }
                }
            }
            Body::TurnStarted(_) => self.reports.deferred.clear(),
            Body::ModelCalled(called)
                if called.compaction.is_none()
                    && !called.aside()
                    && called
                        .error
                        .as_ref()
                        .is_none_or(|error| error.class != ErrorClass::CompactionPaused) =>
            {
                self.reports
                    .pending
                    .retain(|pending| *pending > called.seen);
            }
            Body::TurnEnded(_) => self.reports.pending.clear(),
            Body::TurnUnreverted(unreverted) if self.changes_in(&unreverted.turns) == 0 => {
                self.woken(next);
            }
            Body::FilesRestored(_) => self.woken(next),
            _ => {}
        }
    }

    /// 回合结束时，由排着的回报接着开的那一条：最后一条，打断的、没人看着的一次性会话不算（`watch/queue.rs` 照它和排着
    /// 的消息里后来的那条比）。
    pub(super) fn report_trigger(&self, reason: &EndReason) -> Option<Seq> {
        let quiet = *reason == EndReason::Interrupted || self.unwatched();
        self.reports.pending.last().copied().filter(|_| !quiet)
    }

    /// 恢复撤销、改回文件以后：这时开得了、记在一边的里面有派它的那一轮还在的，接着开的那一轮由最后那条触发；不然不开。
    fn woken(&mut self, next: Option<Seq>) {
        let expected = self
            .reports
            .deferred
            .iter()
            .rev()
            .find(|(_, job)| !self.hidden(job))
            .map(|(seq, _)| *seq)
            .filter(|_| self.can_wake());
        if expected.is_some() {
            self.seen_paths.insert("恢复撤销以后由回报接着开");
        }
        assert_eq!(
            next, expected,
            "种子 {}：恢复撤销以后，记在一边的回报该不该接着开",
            self.seed
        );
    }

    /// 这条回报说的是哪个任务、会不会叫醒她；记下它带来的变化（后台命令结束了，子代理被停掉了）。
    fn arrived(&mut self, body: &Body) -> (JobId, bool) {
        let (job, wakes, over) = match body {
            Body::JobReported(reported) => {
                let wakes = match reported.reason {
                    JobReason::Stopped => !reported.by_model,
                    JobReason::Undone | JobReason::Restarted | JobReason::Aborted => false,
                    JobReason::Exited | JobReason::Other(_) => true,
                };
                (reported.job.clone(), wakes, true)
            }
            Body::ChildReported(reported) => {
                // 她自己停的子代理不叫醒她（施工 7-4），人停的叫醒。
                let wakes = match reported.reason {
                    ChildReason::Stopped => !reported.by_model,
                    ChildReason::Undone | ChildReason::Aborted => false,
                    ChildReason::Done | ChildReason::Other(_) => true,
                };
                let over = matches!(reported.reason, ChildReason::Stopped | ChildReason::Undone);
                (reported.job.clone(), wakes, over)
            }
            _ => unreachable!("只有两种回报"),
        };
        if let Some(known) = self.reports.jobs.get_mut(&job) {
            known.over |= over;
        }
        (job, wakes)
    }

    /// 派它的那一轮撤掉了。
    fn hidden(&self, job: &JobId) -> bool {
        self.reports
            .jobs
            .get(job)
            .is_none_or(|job| !self.undo.effective.contains(&job.turn))
    }

    /// 没人看着的一次性会话。
    pub(super) fn unwatched(&self) -> bool {
        self.reports.oneshot && !self.reports.watched
    }

    /// 闲着时回报这时开得了一轮。
    fn can_wake(&self) -> bool {
        !self.unwatched()
            && !self.undo.can_unrevert()
            && self.restoring.pending.is_none()
            && self.undo.reading.is_none()
    }
}
