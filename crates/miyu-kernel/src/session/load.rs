//! 从日志载入（`docs/designs/02-内核.md` 第六节「载入、崩溃、重启」第 1、2、4 条）：一条条过账本，
//! 重建有效历史、现在的权限、最近的命令编号。日志停在一个没结束的回合里，就是崩了：那一轮收尾，
//! 等人开口。最后一轮是被有计划的重启打断的：自动开一轮接着干；那以后撤销过的不接（第六节
//! 「撤销与恢复」）。
//!
//! 载入过两遍（施工 6-9，`docs/blueprint/kernel/history.md`「载入」）：先整份过账本，账本认出哪几次压缩还算数（撤掉
//! 了的不算）；有效历史再从还算数的最近一次压缩替代到的下一条起重建。

use std::collections::BTreeMap;
use std::fmt;

use super::Session;
use super::action::Action;
use super::configure::Reference;
use super::grants::Grants;
use super::jobs::{self, Arrived, Waker};
use super::meta::Meta;
use super::policy::Policy;
use super::recent::Recent;
use super::report::Duty;
use super::sight::Sight;
use super::title::Naming;
use super::turn::Stage;
use crate::event::{Body, EndReason, Event, Permission, PolicyChanged, ToolStatus};
use crate::facts::Environment;
use crate::history::History;
use crate::id::{CommandId, Seq, SessionId};
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
            LoadError::Empty => write!(f, "the log has no events"),
            LoadError::Broken(error) => write!(f, "the log is broken: {error}"),
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
    /// 最后开的那个回合有没有触发：没有的是手动压缩单开的那一轮（施工 6-8），被重启打断了也不接着干。
    triggered: bool,
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
    /// 一次性的会话（`session.created` 的 `oneshot`，施工 7-2）。
    oneshot: bool,
    /// 这时有回合开着：回报到的时候闲不闲（施工 7-2）。
    open: bool,
    /// 记在一边的回报、子代理的留言：最后一次开回合以后、闲着时到的、会叫醒她的（施工 7-2，`jobs.rs`；施工 7-7）。
    deferred: Vec<Arrived>,
    /// 现在的标题、置顶（施工 3-8 三补）：撤掉的回合里改的也算，改名不是对话的一部分。
    meta: Meta,
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
    /// 这一轮有没有触发：没有的（手动压缩那一轮）不接着干（施工 6-8）。
    triggered: bool,
}

impl Session {
    /// 从日志载入会话 `session`：日志一条条交给账本查过（坏日志在这里就拦下），重建有效历史、现在的
    /// 权限、会话的引用（施工 8-10，撤掉的回合里换的也算）、最近接受的命令编号。读进来的都已经落了盘。有效历史从还算数的最近一次压缩替代到的下一条起，留着一切地
    /// 收、再落到检查点上（施工 6-9）；那个检查点重读过文件的，交回的动作里第一个是 `Recall`。
    ///
    /// 日志停在一个没结束的回合里，就是崩了：还没有结果的调用各补一条「已取消：Miyu 重启了，没跑完」，
    /// 再结束这一轮，原因 `aborted`，等人开口。最后一轮是被有计划的重启打断的（`restarted`），自动
    /// 开一轮接着干；连着被打断的轮数超过了策略里的上限，就不接。补的、开的事件在返回的动作里，
    /// 时刻是 `at`，`by` 是内核。有 `job.started`、还没报过结束的后台命令，先各补一条 `job.reported`（`aborted`，施工
    /// 7-3）。没听到的回报不因为载入开轮：记在一边的照日志算回来，恢复了撤销再说；有没有头订阅着
    /// 当没有，头订阅了再交（施工 7-2）。子会话最后报过的那一份再交一次，排在 `Recall` 后面；崩了的那一轮该报的，补的
    /// 事件落了盘再报 `aborted`；重启打断了、没接着干的那一轮当场照 `aborted` 报（施工 7-6，`report.rs`）。
    ///
    /// # Errors
    ///
    /// 日志是空的，或者有一条过不了账本，返回 [`LoadError`]。
    pub fn load(
        session: SessionId,
        events: Vec<Event>,
        at: Timestamp,
        policy: Policy,
        environment: Environment,
    ) -> Result<(Session, Vec<Action>), LoadError> {
        let mut ledger = Ledger::for_session(session.clone());
        let mut replay = Replay::default();
        let mut duty = Duty::default();
        let mut naming = Naming::default();
        let mut reference = Reference::default();
        let mut sight = Sight::default();
        let mut grants = Grants::default();
        for event in &events {
            // 向上回报照活着时的算（施工 7-6）：活着时落了盘就报，接着开的一轮和结束在同一批里，那时不该报。
            if !matches!(event.body, Body::TurnStarted(_)) && duty.due(&ledger) {
                duty.take(&policy.reports);
            }
            let queued = ledger.queued();
            ledger.append(event).map_err(LoadError::Broken)?;
            duty.note(event);
            naming.note(event);
            reference.note(event);
            sight.note(event);
            grants.note(event);
            replay.note(event, queued, jobs::waking(&ledger, event));
        }
        if duty.due(&ledger) {
            duty.take(&policy.reports);
        }
        let from = ledger.compacted().map_or(Seq::FIRST, Seq::next);
        let mut history = History::whole();
        // 那一段以前的只记派出去的任务：回报的标题、派它的那一轮撤掉了没有照它（施工 7-2）。
        for event in events {
            match event.seq >= from {
                true => history.append(event),
                false => history.note(&event),
            }
        }
        history.settle();
        let Some(permission) = replay.permission.clone() else {
            return Err(LoadError::Empty);
        };
        let mut recent = Recent::default();
        for (id, seqs) in std::mem::take(&mut replay.commands) {
            recent.insert(id, seqs);
        }
        let mut session = Session {
            id: session,
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
            restoring: None,
            reading: None,
            limits: None,
            oneshot: replay.oneshot,
            watched: false,
            deferred: std::mem::take(&mut replay.deferred),
            restarting: false,
            duty,
            meta: std::mem::take(&mut replay.meta),
            recapping: None,
            naming,
            titling: None,
            reference,
            sight,
            grants,
            told_todos: Vec::new(),
        };
        // 头订阅时从回应里拿当前的待办（施工 D-3）：当已经告诉过它这一份，变了再推。
        session.told_todos = session.todos();
        let mut actions: Vec<Action> = session.recall().into_iter().collect();
        // 最后报的那一份再交一次（施工 7-6）：送到一半崩了的不漏，父会话照命令编号认出重的，不重。
        actions.extend(session.duty.last().cloned().map(Action::Report));
        // 崩了的核心带走了在跑的后台命令：先补它们的结束，再收拾没走完的那一轮（施工 7-3）。
        let mut events = session.abort_commands(at);
        let recovered = session.recover(at, replay);
        if recovered.is_empty() {
            session.duty.not_resumed();
        }
        events.extend(recovered);
        if events.is_empty() {
            actions.extend(session.report_up());
        } else {
            actions.push(Action::Append(events));
        }
        Ok((session, actions))
    }

    /// 载入以后：崩了的那一轮收尾；被有计划的重启打断的，接着干。返回追加的事件。
    fn recover(&mut self, at: Timestamp, replay: Replay) -> Vec<Event> {
        if let Some(id) = self.ledger.open_turn() {
            let cause = replay.opened;
            self.turn = Some(self.new_turn(id.started(), cause.clone(), Stage::Settling));
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
                    && ended.triggered
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
    fn resumes(&self, trigger: Option<Seq>) -> bool {
        self.ended.as_ref().is_some_and(|ended| {
            ended.reason == EndReason::Restarted
                && Some(ended.queued.last().copied().unwrap_or(ended.seq)) == trigger
        })
    }

    /// 读进来一条：记下它带来的变化。`queued` 是这一条之前还排着队的消息；`waking` 是它会叫醒她时是谁的（会叫醒她
    /// 的回报、子代理的留言、别的 harness 发来的话）。
    fn note(&mut self, event: &Event, queued: Vec<Seq>, waking: Option<Waker>) {
        self.last = Some(event.seq);
        if !self.open
            && let Some(waker) = waking
        {
            self.deferred.push(Arrived {
                seq: event.seq,
                cause: event.cause.clone(),
                waker,
            });
        }
        match &event.body {
            Body::SessionCreated(created) => {
                self.permission = Some(created.permission.clone());
                self.oneshot = created.oneshot;
            }
            Body::PolicyChanged(PolicyChanged {
                permission: Some(permission),
                ..
            }) => self.permission = Some(permission.clone()),
            Body::TurnStarted(started) => {
                if !self.resumes(started.trigger) {
                    self.restarts = 0;
                }
                self.opened = event.cause.clone();
                self.triggered = started.trigger.is_some();
                self.causes.clear();
                self.open = true;
                self.deferred.clear();
            }
            Body::MessageUser(_) => {
                self.causes.insert(event.seq, event.cause.clone());
            }
            Body::TurnEnded(ended) => {
                self.open = false;
                self.restarts = match ended.reason {
                    EndReason::Restarted => self.restarts + 1,
                    _ => 0,
                };
                self.ended = Some(Ended {
                    seq: event.seq,
                    reason: ended.reason.clone(),
                    cause: event.cause.clone(),
                    queued,
                    triggered: self.triggered,
                });
            }
            // 撤销过的不接着干：人已经动过它了。
            Body::TurnReverted(_) => self.ended = None,
            Body::MetaChanged(changed) => self.meta.note(changed),
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
