//! 会话：内核的状态机（`docs/designs/02-内核.md` 第四节「输入、动作、命令怎么写」、
//! 第六节「回合怎么开、请求怎么发」）。
//!
//! 送进一条输入，出来一串动作。会话不做 I/O：要追加的事件、要回应的命令、要推送的事件、
//! 要跑的挂接点、要发的请求，都写成动作交给执行器；事件同步到磁盘以后，执行器送一条
//! 「落盘了」进来，会话这才推送、回应，回合这才往下走（`07-存储.md` S4）。
//!
//! 每收到一次命令，恰好回应一次（不变量 7）；同一个编号只生效一次（不变量 9）。

mod action;
mod approval;
mod aside;
mod breaker;
mod call;
mod clear;
mod compaction;
mod configure;
mod input;
mod interrupt;
mod jobs;
mod limits;
mod load;
mod manual;
mod messages;
mod meta;
mod overflow;
mod peers;
mod permission;
mod policy;
mod question;
mod queue;
mod rebuild;
mod recap;
mod recent;
mod redo;
mod replies;
mod report;
mod restart;
mod restore;
mod retry;
mod revert;
mod send;
mod shorten;
mod spans;
mod step;
mod summary;
mod title;
mod tools;
mod turn;

pub use action::{Action, Outcome, Reason};
pub use input::{
    Answer, Command, Injection, Input, Limits, Queued, Received, Replaced, Reread, Verdict,
};
pub use limits::ContextLimits;
pub use load::LoadError;
pub use messages::Subagent;
pub use policy::{Compaction, Notes, Pause, Peers, Policy, Rebuild, Reports, Shorten, Titles};
pub use report::Upward;
pub use restore::{Expect, Step, StepAction};

use crate::event::{Body, Event, Permission, SessionCreated, ToolResult, ToolStatus};
use crate::facts::Environment;
use crate::history::History;
use crate::id::{CommandId, Seq, SessionId, TurnId};
use crate::ledger::{Ledger, LedgerError};
use crate::origin::By;
use crate::request::Fingerprint;
use crate::time::Timestamp;
use jobs::Arrived;
use recent::Recent;
use revert::{ReadingBack, Restoring};
use turn::Turn;

/// 一个会话的状态机。
#[derive(Debug)]
pub struct Session {
    /// 这个会话的编号：执行器造会话、载入时交进来，日志所在的目录就叫它。事实 `session` 写它（施工 1-13 再补）。
    id: SessionId,
    /// 日志的账本：给新事件编序号，追加之前照规矩查一遍。
    ledger: Ledger,
    /// 有效历史，组装请求用。事件追加时就交给它。
    history: History,
    /// 追加了、还没落盘的事件，照先后。
    unstored: Vec<Event>,
    /// 落了盘的最后一条；还没有落过盘就是没有。
    stored: Option<Seq>,
    /// 等事件落了盘才回应的命令：编号、要等的事件的序号、回应的结局（施工 3-8 四补：回顾回的不是「接受了」）。照收到的
    /// 先后。
    waiting: Vec<(CommandId, Vec<Seq>, Outcome)>,
    /// 最近接受的命令编号。
    recent: Recent,
    /// 冻结在会话上的策略。
    policy: Policy,
    /// 会话所在的环境：时区、工作目录。
    environment: Environment,
    /// 现在的权限：人最近一次切成的。
    permission: Permission,
    /// 实际生效的权限：收紧的当场换，放宽的等下一次请求（`02-内核.md` 第六节「权限级别怎么切」）。
    effective: Permission,
    /// 正在进行的回合；空闲时没有。
    turn: Option<Turn>,
    /// 这个会话上一次请求的指纹，比出下一次的第一处不同。只在内存里。
    last_request: Option<Fingerprint>,
    /// 结束了、`turn.ended` 还没落盘的回合，和那一条的序号：落了盘才跑回合结束的挂接点。
    closing: Vec<(TurnId, Seq)>,
    /// 撤销、恢复以后正在改回文件（施工 4-7 上）：交出去了，结局还没回来。
    restoring: Option<Restoring>,
    /// 撤掉压缩的撤销正在读回日志（施工 6-9）：交出去了，读回的还没来。
    reading: Option<ReadingBack>,
    /// 模型的限额，执行器交来的；没交过的不主动压缩（施工 6-2 上）。只在内存里。
    limits: Option<Limits>,
    /// 一次性的会话：`miyu ask` 开的（`session.created` 的 `oneshot`）。没人看着的时候回报不叫醒她（施工 7-2）。
    oneshot: bool,
    /// 有头订阅着（施工 7-2，[`Input::Watched`]）：只在内存里，造会话、载入以后当没人看着。
    watched: bool,
    /// 闲着时到的、会叫醒她、这时却开不了一轮的回报，照先后（施工 7-2，`jobs.rs`）：随便哪一轮开了就清掉；恢复了撤销、
    /// 这时开得了，由最后那条接着开。
    deferred: Vec<Arrived>,
    /// 收到了「要重启了」（施工 7-3）：要关了，之后到的回报只记下、不开轮。只在内存里。
    restarting: bool,
    /// 子会话欠着父会话的回报（施工 7-6，`report.rs`）：每追加一条记一次。
    duty: report::Duty,
    /// 现在的标题、置顶（施工 3-8 三补，`meta.rs`）：日志里的 `session.meta_changed` 一路算的。
    meta: meta::Meta,
    /// 在路上的那一次回顾（施工 3-8 四补，`recap.rs`）：只在内存里，载入以后没有。
    recapping: Option<recap::Recapping>,
    /// 起标题的账（施工 3-8 五补，`title.rs`）：每追加一条记一次。
    naming: title::Naming,
    /// 在路上的那一次起标题（施工 3-8 五补）：只在内存里，载入以后没有。
    titling: Option<aside::Aside>,
    /// 会话的引用和最近一次换模型写在第几条（施工 8-10，`configure.rs`）：每追加一条记一次。
    reference: configure::Reference,
}

impl Session {
    /// 造一个会话：追加第 1 条事件 `session.created`，它的 `cause` 是造会话的那个命令
    /// （`04-核心协议.md` 的 `session.create`）。这一条落了盘，再回应这个命令。
    ///
    /// 会话的编号 `session`、冻结在会话上的策略和会话所在的环境，由执行器一起交进来；开始时的权限取自 `created`。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：第 1 条是 `session.created`，账本收它。
    pub fn create(
        session: SessionId,
        id: CommandId,
        by: By,
        at: Timestamp,
        created: SessionCreated,
        policy: Policy,
        environment: Environment,
    ) -> (Session, Vec<Action>) {
        let mut session = Session {
            ledger: Ledger::for_session(session.clone()),
            id: session,
            history: History::default(),
            unstored: Vec::new(),
            stored: None,
            waiting: Vec::new(),
            recent: Recent::default(),
            policy,
            environment,
            permission: created.permission.clone(),
            effective: created.permission.clone(),
            turn: None,
            last_request: None,
            closing: Vec::new(),
            restoring: None,
            reading: None,
            limits: None,
            oneshot: created.oneshot,
            watched: false,
            deferred: Vec::new(),
            restarting: false,
            duty: report::Duty::default(),
            meta: meta::Meta::default(),
            recapping: None,
            naming: title::Naming::default(),
            titling: None,
            reference: configure::Reference::default(),
        };
        let event = session.record(at, by, Some(id.clone()), Body::SessionCreated(created));
        session.accept(id, vec![event.seq]);
        (session, vec![Action::Append(vec![event])])
    }

    /// 空闲：没有在跑的回合，也没有结束了、`turn.ended` 还没落盘的，也没在读回日志、改回文件。核心看它决定能不能空闲
    /// 退出（`12-进程形态与分发.md` 第二节，施工 3-9 上）。
    pub fn idle(&self) -> bool {
        self.turn.is_none()
            && self.closing.is_empty()
            && self.restoring.is_none()
            && self.reading.is_none()
    }

    /// 会话现在在哪个目录里干活：头最近一次报来的，回合开始时这一轮照它（施工 8-4）。纯查询：会话 actor 在回合开始时照它
    /// 读一次项目配置（`docs/blueprint/config.md`「怎么走」第八条第 3 条）。
    pub fn cwd(&self) -> &str {
        &self.environment.cwd
    }

    /// 落了盘的最后一条（施工 3-8 六补）：`Stored` 送进来那一刻就推送了，所以也是推过的最后一条；还没落过盘的没有，载入的
    /// 是日志里最后一条。纯查询：会话 actor 订阅时照它定补发补到哪一条（`docs/blueprint/session/actor.md` 第 6 条）。
    pub fn landed(&self) -> Option<Seq> {
        self.stored
    }

    /// 删得了没有（施工 3-8 三补，`protocol.md` 的 `session.delete`）：不空闲的删不了，正在读回日志、改回文件的是
    /// [`Reason::Restoring`]，别的（有回合在进行、结束了 `turn.ended` 还没落盘）是 [`Reason::TurnRunning`]。纯查询：会话
    /// actor 照它答应删、停下，挪目录是会话表的事。
    ///
    /// # Errors
    ///
    /// 删不了，交回原因。
    pub fn deletable(&self) -> Result<(), Reason> {
        if self.restoring.is_some() || self.reading.is_some() {
            return Err(Reason::Restoring);
        }
        match self.idle() {
            true => Ok(()),
            false => Err(Reason::TurnRunning),
        }
    }

    /// 送进一条输入，出来一串动作。
    ///
    /// # Panics
    ///
    /// 内核自己造出的事件过不了账本时 panic：那是内核的 bug，不是命令的拒绝。
    pub fn handle(&mut self, input: Input) -> Vec<Action> {
        match input {
            Input::Command(received) => self.receive(received),
            Input::Stored { at, upto } => self.stored(at, upto),
            Input::Environment(environment) => {
                self.environment = environment;
                Vec::new()
            }
            Input::Limits(limits) => {
                self.limits = Some(limits);
                Vec::new()
            }
            Input::Reread { seen, files, .. } => self.reread_done(seen, files),
            Input::Recalled { texts } => {
                self.history.recall(texts);
                Vec::new()
            }
            Input::TurnStartHooksDone {
                at,
                turn,
                injected,
                replaced,
            } => self.turn_start_hooked(at, turn, injected, replaced),
            Input::RequestSent {
                at,
                seen,
                model,
                request,
            } => self.request_sent(at, seen, model, request),
            Input::ModelDelta { at, seen, delta } => self.model_delta(at, seen, delta),
            Input::ModelEnded {
                at,
                seen,
                usage,
                error,
                wait_ms,
                excess,
                failover,
            } => self.model_ended(
                at,
                seen,
                usage,
                error,
                retry::Said { wait_ms, failover },
                excess,
            ),
            Input::Woke { at, seen } => self.woke(at, seen),
            Input::ToolDone {
                at,
                call_id,
                error,
                blocks,
                duration_ms,
                human,
                effects,
                stopped,
            } => self.tool_done(
                at,
                ToolResult {
                    call_id,
                    status: if stopped {
                        ToolStatus::Cancelled
                    } else if error {
                        ToolStatus::Error
                    } else {
                        ToolStatus::Ok
                    },
                    blocks,
                    duration_ms,
                    human,
                    effects,
                },
            ),
            Input::ToolProgress { at, call_id, text } => self.tool_progress(at, call_id, text),
            Input::ToolGuarded {
                at,
                call_id,
                verdict,
            } => self.tool_guarded(at, call_id, verdict),
            Input::ToolAsks {
                at,
                call_id,
                questions,
            } => self.tool_asks(at, call_id, questions),
            Input::Restored { at, files } => self.restored(at, files),
            Input::ReadBack { at, from, events } => self.read_back(at, from, events),
            Input::Restarting { at } => self.restart(at),
            Input::JobEnded {
                at,
                by,
                cause,
                reported,
            } => self.job_ended(at, by, cause, reported),
            Input::AsideSent {
                at,
                purpose,
                upto,
                model,
                request,
            } => self.aside_sent(at, &purpose, upto, model, request),
            Input::AsideDelta {
                at,
                purpose,
                upto,
                delta,
            } => self.aside_delta(at, &purpose, upto, delta),
            Input::AsideEnded {
                at,
                purpose,
                upto,
                usage,
                error,
            } => self.aside_ended(at, &purpose, upto, usage, error),
            Input::Watched { watched } => {
                self.watched = watched;
                Vec::new()
            }
            Input::WatchEnded {
                at,
                session,
                reason,
            } => self.watch_ended(at, session, reason),
        }
    }

    /// 收到一个命令。接受过的编号照上一次回应；新的照命令判。回顾不记编号（施工 3-8 四补）：再来一次就是再要一次，没有新内容
    /// 的照样交回上一句。
    fn receive(&mut self, received: Received) -> Vec<Action> {
        let Received {
            id,
            by,
            at,
            command,
        } = received;
        if !matches!(command, Command::Recap)
            && let Some(events) = self.recent.get(&id)
        {
            let events = events.to_vec();
            return self.reply_when_stored(id, events);
        }
        // 读回日志、改回文件的时候不接命令：会话 actor 做完才接下一个，这是兜底（施工 4-7 上、6-9）。
        if self.restoring.is_some() || self.reading.is_some() {
            return vec![rejected(id, Reason::Restoring)];
        }
        match command {
            Command::Send { blocks, urgent } => self.send(id, by, at, blocks, urgent),
            Command::Interrupt { queued } => self.interrupt(id, by, at, queued),
            Command::SetMeta { title, pinned } => self.set_meta(id, by, at, title, pinned),
            Command::Configure { model } => self.configure(id, by, at, model),
            Command::SetPermission { level, read_only } => {
                self.set_permission(id, by, at, level, read_only)
            }
            Command::Answer {
                call_id,
                answer: Answer::Approval { decision, reason },
            } => self.answer(id, by, at, call_id, decision, reason),
            Command::Answer {
                call_id,
                answer: Answer::Questions(answers),
            } => self.answer_question(id, by, at, call_id, answers),
            Command::Revert { turn } => self.revert(id, by, at, turn),
            Command::Unrevert => self.unrevert(id, by, at),
            Command::Redo { text, attachments } => self.redo(id, by, at, text, attachments),
            Command::Compact { instructions } => self.compact(id, at, instructions),
            Command::Clear => self.clear(id, at),
            Command::Recap => self.recap(id),
            Command::Report(reported) => self.report(id, by, at, reported),
            Command::PeerIdle { status } => self.peer_idle(id, by, at, status),
        }
    }

    /// 造一条事件：交给账本查过，记在账上，交给有效历史，等着落盘。回合进行中造的，带上
    /// 这个回合的编号；`turn.started` 带它自己的序号（`03-事件模型.md` 第二节）。
    fn record(&mut self, at: Timestamp, by: By, cause: Option<CommandId>, body: Body) -> Event {
        let seq = self.ledger.next_seq();
        let turn = match body {
            Body::TurnStarted(_) => Some(TurnId::new(seq)),
            _ => self.turn.as_ref().map(|turn| turn.id),
        };
        let event = Event {
            seq,
            at,
            turn,
            by,
            cause,
            body,
        };
        if let Err(error) = self.commit(&event) {
            panic!("the kernel's own event failed the ledger, a kernel bug: {error}");
        }
        event
    }

    /// 追加一条造好的事件：交给账本查过，记在账上，交给有效历史，等着落盘。过不了账本的什么都不动，交回违反了哪一条。
    fn commit(&mut self, event: &Event) -> Result<(), LedgerError> {
        self.ledger.append(event)?;
        self.duty.note(event);
        self.naming.note(event);
        self.reference.note(event);
        self.history.append(event.clone());
        self.unstored.push(event.clone());
        Ok(())
    }

    /// 到第 `upto` 条为止落了盘：先推送这些事件，再回应事件全落了盘的命令（`04-核心协议.md`
    /// 第六节第 2 条：先见结果，后见回应），再跑结束了的回合的挂接点、向上回报、起标题（施工 3-8 五补），然后回合往下走。`upto` 超出追加过的，多出来的
    /// 不算；不比上一次往后的，什么都不做。`at` 是落完盘的时刻。
    fn stored(&mut self, at: Timestamp, upto: Seq) -> Vec<Action> {
        let split = self.unstored.partition_point(|event| event.seq <= upto);
        if split == 0 {
            return Vec::new();
        }
        let pushed: Vec<Event> = self.unstored.drain(..split).collect();
        self.stored = pushed.last().map(|event| event.seq);
        let mut actions = vec![Action::Push(pushed)];
        for (id, events, outcome) in std::mem::take(&mut self.waiting) {
            if self.is_stored(&events) {
                actions.push(Action::Reply { id, outcome });
            } else {
                self.waiting.push((id, events, outcome));
            }
        }
        let closed = self.closed();
        actions.extend(closed.iter().map(|&turn| Action::RunTurnEndHooks { turn }));
        actions.extend(self.report_up());
        actions.extend(self.title_up(&closed));
        actions.extend(self.advance(at));
        actions
    }
}

/// 回应：接受了，附上它产生的事件的序号。
fn accepted(id: CommandId, events: Vec<Seq>) -> Action {
    Action::Reply {
        id,
        outcome: Outcome::Accepted { events },
    }
}

/// 回应：拒绝了，附上原因。
fn rejected(id: CommandId, reason: Reason) -> Action {
    Action::Reply {
        id,
        outcome: Outcome::Rejected { reason },
    }
}

#[cfg(test)]
mod tests;
