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
mod call;
mod input;
mod interrupt;
mod permission;
mod policy;
mod question;
mod queue;
mod recent;
mod step;
mod tools;
mod turn;

pub use action::{Action, Outcome, Reason};
pub use input::{Answer, Command, Injection, Input, Queued, Received, Verdict};
pub use policy::Policy;

use crate::event::{Body, Event, MessageUser, Permission, SessionCreated};
use crate::facts::Environment;
use crate::history::History;
use crate::id::{CommandId, Seq, TurnId};
use crate::ledger::Ledger;
use crate::origin::By;
use crate::request::Fingerprint;
use crate::time::Timestamp;
use recent::Recent;
use turn::Turn;

/// 一个会话的状态机。
#[derive(Debug)]
pub struct Session {
    /// 日志的账本：给新事件编序号，追加之前照规矩查一遍。
    ledger: Ledger,
    /// 有效历史，组装请求用。事件追加时就交给它。
    history: History,
    /// 追加了、还没落盘的事件，照先后。
    unstored: Vec<Event>,
    /// 落了盘的最后一条；还没有落过盘就是没有。
    stored: Option<Seq>,
    /// 等事件落了盘才回应的命令：编号，和它产生的事件的序号。照收到的先后。
    waiting: Vec<(CommandId, Vec<Seq>)>,
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
}

impl Session {
    /// 造一个会话：追加第 1 条事件 `session.created`，它的 `cause` 是造会话的那个命令
    /// （`04-核心协议.md` 的 `session.create`）。这一条落了盘，再回应这个命令。
    ///
    /// 冻结在会话上的策略和会话所在的环境，由执行器一起交进来；开始时的权限取自 `created`。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：第 1 条是 `session.created`，账本收它。
    pub fn create(
        id: CommandId,
        by: By,
        at: Timestamp,
        created: SessionCreated,
        policy: Policy,
        environment: Environment,
    ) -> (Session, Vec<Action>) {
        let mut session = Session {
            ledger: Ledger::default(),
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
        };
        let event = session.record(at, by, Some(id.clone()), Body::SessionCreated(created));
        session.accept(id, vec![event.seq]);
        (session, vec![Action::Append(vec![event])])
    }

    /// 送进一条输入，出来一串动作。
    ///
    /// # Panics
    ///
    /// 内核自己造出的事件过不了账本时 panic：那是内核的 bug，不是命令的拒绝。
    pub fn handle(&mut self, input: Input) -> Vec<Action> {
        match input {
            Input::Command(received) => self.receive(received),
            Input::Stored { upto } => self.stored(upto),
            Input::Environment(environment) => {
                self.environment = environment;
                Vec::new()
            }
            Input::TurnStartHooksDone { at, turn, injected } => {
                self.turn_start_hooked(at, turn, injected)
            }
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
            } => self.model_ended(at, seen, usage, error),
            Input::ToolDone {
                at,
                call_id,
                error,
                blocks,
                duration_ms,
            } => self.tool_done(at, call_id, error, blocks, duration_ms),
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
        }
    }

    /// 收到一个命令。接受过的编号照上一次回应；新的照命令判。
    fn receive(&mut self, received: Received) -> Vec<Action> {
        let Received {
            id,
            by,
            at,
            command,
        } = received;
        if let Some(events) = self.recent.get(&id) {
            let events = events.to_vec();
            return self.reply_when_stored(id, events);
        }
        match command {
            Command::Send { blocks, urgent } => {
                if blocks.is_empty() {
                    return vec![rejected(id, Reason::EmptyMessage)];
                }
                let body = Body::MessageUser(MessageUser { blocks });
                let message = self.record(at, by.clone(), Some(id.clone()), body);
                self.accept(id.clone(), vec![message.seq]);
                let trigger = message.seq;
                let mut events = vec![message];
                let mut stops = Vec::new();
                if self.turn.is_none() {
                    events.extend(self.open_turn(at, trigger, Some(id)));
                } else {
                    self.enqueue(trigger, id.clone());
                    let (voided, stopped) = self.void_waiting(at, &by, &id);
                    events.extend(voided);
                    stops = stopped;
                    if urgent {
                        events.extend(self.interject(at, by, id));
                    }
                }
                let mut actions = vec![Action::Append(events)];
                actions.extend(stops);
                actions
            }
            Command::Interrupt { queued } => self.interrupt(id, by, at, queued),
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
        if let Err(error) = self.ledger.append(&event) {
            panic!("内核自己造的事件过不了账本，这是内核的 bug：{error}");
        }
        self.history.append(event.clone());
        self.unstored.push(event.clone());
        event
    }

    /// 接受一个命令：记下编号和它产生的事件，等落了盘再回应。
    fn accept(&mut self, id: CommandId, events: Vec<Seq>) {
        self.recent.insert(id.clone(), events.clone());
        self.waiting.push((id, events));
    }

    /// 接受过的命令又来了：它的事件都落了盘，当场回应；还没有，排队等落盘。
    fn reply_when_stored(&mut self, id: CommandId, events: Vec<Seq>) -> Vec<Action> {
        if self.is_stored(&events) {
            vec![accepted(id, events)]
        } else {
            self.waiting.push((id, events));
            Vec::new()
        }
    }

    /// 这几条事件都落了盘没有。
    fn is_stored(&self, events: &[Seq]) -> bool {
        match (events.last(), self.stored) {
            (None, _) => true,
            (Some(last), Some(stored)) => *last <= stored,
            (Some(_), None) => false,
        }
    }

    /// 到第 `upto` 条为止落了盘：先推送这些事件，再回应事件全落了盘的命令（`04-核心协议.md`
    /// 第六节第 2 条：先见结果，后见回应），再跑结束了的回合的挂接点，然后回合往下走。`upto` 超出追加过的，多出来的
    /// 不算；不比上一次往后的，什么都不做。
    fn stored(&mut self, upto: Seq) -> Vec<Action> {
        let split = self.unstored.partition_point(|event| event.seq <= upto);
        if split == 0 {
            return Vec::new();
        }
        let pushed: Vec<Event> = self.unstored.drain(..split).collect();
        self.stored = pushed.last().map(|event| event.seq);
        let mut actions = vec![Action::Push(pushed)];
        for (id, events) in std::mem::take(&mut self.waiting) {
            if self.is_stored(&events) {
                actions.push(accepted(id, events));
            } else {
                self.waiting.push((id, events));
            }
        }
        actions.extend(self.closed());
        actions.extend(self.advance());
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
