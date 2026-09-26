//! 从日志载入（`docs/designs/02-内核.md` 第六节「载入、崩溃、重启」第 1、2、4 条）：一条条过账本，
//! 重建有效历史、现在的权限、最近的命令编号。日志停在一个没结束的回合里，就是崩了：那一轮收尾，
//! 等人开口。最后一轮是被有计划的重启打断的：自动开一轮接着干；那以后撤销过的不接（第六节
//! 「撤销与恢复」）。

use std::collections::BTreeMap;
use std::fmt;

use super::Session;
use super::action::Action;
use super::policy::Policy;
use super::recent::Recent;
use super::turn::{Stage, Turn};
use crate::event::{Body, EndReason, Event, Permission, PolicyChanged, ToolStatus};
use crate::facts::Environment;
use crate::history::History;
use crate::id::{CommandId, Seq};
use crate::ledger::{Ledger, LedgerError};
use crate::origin::By;
use crate::time::Timestamp;

/// 载入不了。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// 日志里一条事件都没有。
    Empty,
    /// 有一条过不了账本：日志坏了。里面写明是第几条、违反了哪一条。
    Broken(LedgerError),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Empty => write!(f, "日志里一条事件都没有"),
            LoadError::Broken(error) => write!(f, "日志坏了：{error}"),
        }
    }
}

impl std::error::Error for LoadError {}

/// 载入时边读边记的几样：日志里没有现成的，要一路算。
#[derive(Default)]
struct Replay {
    /// 现在的权限：创建时定的，被后来切过的盖掉。
    permission: Option<Permission>,
    /// 最后一条的序号。
    last: Option<Seq>,
    /// 最后开的那个回合的 `cause`。
    opened: Option<CommandId>,
    /// 最后结束的那个回合。
    ended: Option<Ended>,
    /// 连着几轮是被有计划的重启打断的：数的是被打断、接着干、又被打断的那一串，别的回合开了就从头数。
    restarts: u32,
    /// 每个命令编号，和 `cause` 是它的那几条，照编号第一次出现的先后。
    commands: Vec<(CommandId, Vec<Seq>)>,
    /// 编号在 `commands` 里排第几。
    index: BTreeMap<CommandId, usize>,
    /// 最后开的那个回合里，每条消息的 `cause`：接着干时，找触发它的那一条的 `cause`。
    causes: BTreeMap<Seq, Option<CommandId>>,
}

/// 结束了的一个回合。
struct Ended {
    /// 那条 `turn.ended` 的序号。
    seq: Seq,
    /// 结束的原因。
    reason: EndReason,
    /// 那条 `turn.ended` 的 `cause`。
    cause: Option<CommandId>,
    /// 结束时还排着队的消息，照先后。
    queued: Vec<Seq>,
}

impl Session {
    /// 从日志载入一个会话：日志一条条交给账本查过（坏日志在这里就拦下），重建有效历史、现在的
    /// 权限、最近接受的命令编号。读进来的都已经落了盘。
    ///
    /// 日志停在一个没结束的回合里，就是崩了：还没有结果的调用各补一条「已取消：Miyu 重启了，没跑完」，
    /// 再结束这一轮，原因 `aborted`，等人开口。最后一轮是被有计划的重启打断的（`restarted`），自动
    /// 开一轮接着干；连着被打断的轮数超过了策略里的上限，就不接。补的、开的事件在返回的动作里，
    /// 时刻是 `at`，`by` 是内核。
    ///
    /// # Errors
    ///
    /// 日志是空的，或者有一条过不了账本，返回 [`LoadError`]。
    pub fn load(
        events: Vec<Event>,
        at: Timestamp,
        policy: Policy,
        environment: Environment,
    ) -> Result<(Session, Vec<Action>), LoadError> {
        let mut ledger = Ledger::default();
        let mut history = History::default();
        let mut replay = Replay::default();
        for event in events {
            let queued = ledger.queued();
            ledger.append(&event).map_err(LoadError::Broken)?;
            replay.note(&event, queued);
            history.append(event);
        }
        let Some(permission) = replay.permission.clone() else {
            return Err(LoadError::Empty);
        };
        let mut recent = Recent::default();
        for (id, seqs) in std::mem::take(&mut replay.commands) {
            recent.insert(id, seqs);
        }
        let mut session = Session {
            ledger,
            history,
            unstored: Vec::new(),
            stored: replay.last,
            waiting: Vec::new(),
            recent,
            policy,
            environment,
            permission: permission.clone(),
            effective: permission,
            turn: None,
            last_request: None,
            closing: Vec::new(),
        };
        let events = session.recover(at, replay);
        let actions = match events.is_empty() {
            true => Vec::new(),
            false => vec![Action::Append(events)],
        };
        Ok((session, actions))
    }

    /// 载入以后：崩了的那一轮收尾；被有计划的重启打断的，接着干。返回追加的事件。
    fn recover(&mut self, at: Timestamp, replay: Replay) -> Vec<Event> {
        if let Some(id) = self.ledger.open_turn() {
            let cause = replay.opened;
            self.turn = Some(Turn {
                id,
                cause: cause.clone(),
                stage: Stage::Settling,
                cwd: self.environment.cwd.clone(),
                requests: 0,
                interjected: None,
                queued: Vec::new(),
                refresh: false,
            });
            let text = self.policy.tool_texts.restarted();
            let mut events: Vec<Event> = self
                .ledger
                .pending_calls()
                .into_iter()
                .map(|call_id| {
                    self.written_result(
                        at,
                        By::Kernel,
                        cause.clone(),
                        call_id,
                        ToolStatus::Cancelled,
                        text.clone(),
                    )
                })
                .collect();
            events.push(self.end_turn(at, By::Kernel, cause, EndReason::Aborted));
            return events;
        }
        match replay.ended {
            Some(ended)
                if ended.reason == EndReason::Restarted
                    && replay.restarts <= self.policy.resumes =>
            {
                let trigger = ended.queued.last().copied().unwrap_or(ended.seq);
                let cause = match trigger == ended.seq {
                    true => ended.cause,
                    false => replay.causes.get(&trigger).cloned().flatten(),
                };
                self.open_turn(at, trigger, cause)
            }
            _ => Vec::new(),
        }
    }
}

impl Replay {
    /// 由 `trigger` 开的这一轮，是不是被重启打断以后接着干的那一轮：最后结束的那一轮是被有计划的
    /// 重启打断的，这一轮由那时排着队的最后一条触发，没有排着队的，由那条结束触发（[`Session::recover`]）。
    fn resumes(&self, trigger: Seq) -> bool {
        self.ended.as_ref().is_some_and(|ended| {
            ended.reason == EndReason::Restarted
                && ended.queued.last().copied().unwrap_or(ended.seq) == trigger
        })
    }

    /// 读进来一条：记下它带来的变化。`queued` 是这一条之前还排着队的消息。
    fn note(&mut self, event: &Event, queued: Vec<Seq>) {
        self.last = Some(event.seq);
        match &event.body {
            Body::SessionCreated(created) => self.permission = Some(created.permission.clone()),
            Body::PolicyChanged(PolicyChanged {
                permission: Some(permission),
                ..
            }) => self.permission = Some(permission.clone()),
            Body::TurnStarted(started) => {
                if !self.resumes(started.trigger) {
                    self.restarts = 0;
                }
                self.opened = event.cause.clone();
                self.causes.clear();
            }
            Body::MessageUser(_) => {
                self.causes.insert(event.seq, event.cause.clone());
            }
            Body::TurnEnded(ended) => {
                self.restarts = match ended.reason {
                    EndReason::Restarted => self.restarts + 1,
                    _ => 0,
                };
                self.ended = Some(Ended {
                    seq: event.seq,
                    reason: ended.reason.clone(),
                    cause: event.cause.clone(),
                    queued,
                });
            }
            // 撤销过的不接着干：人已经动过它了。
            Body::TurnReverted(_) => self.ended = None,
            _ => {}
        }
        if let Some(id) = &event.cause {
            match self.index.get(id) {
                Some(&k) => self.commands[k].1.push(event.seq),
                None => {
                    self.index.insert(id.clone(), self.commands.len());
                    self.commands.push((id.clone(), vec![event.seq]));
                }
            }
        }
    }
}
