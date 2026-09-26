//! 会话：内核的状态机（`docs/designs/02-内核.md` 第四节「输入、动作、命令怎么写」）。
//!
//! 送进一条输入，出来一串动作。会话不做 I/O：要追加的事件、要回应的命令、要推送的事件，都写成
//! 动作交给执行器；事件同步到磁盘以后，执行器送一条「落盘了」进来，会话这才推送、回应
//! （`07-存储.md` S4）。
//!
//! 每收到一次命令，恰好回应一次（不变量 7）；同一个编号只生效一次（不变量 9）。

mod action;
mod input;
mod recent;

pub use action::{Action, Outcome, Reason};
pub use input::{Command, Input, Received};

use crate::event::{Body, Event, MessageUser, SessionCreated};
use crate::id::{CommandId, Seq};
use crate::ledger::Ledger;
use crate::origin::By;
use crate::time::Timestamp;
use recent::Recent;

/// 一个会话的状态机。
#[derive(Debug, Clone)]
pub struct Session {
    /// 日志的账本：给新事件编序号，追加之前照规矩查一遍。
    ledger: Ledger,
    /// 追加了、还没落盘的事件，照先后。
    unstored: Vec<Event>,
    /// 落了盘的最后一条；还没有落过盘就是没有。
    stored: Option<Seq>,
    /// 等事件落了盘才回应的命令：编号，和它产生的事件的序号。照收到的先后。
    waiting: Vec<(CommandId, Vec<Seq>)>,
    /// 最近接受的命令编号。
    recent: Recent,
}

impl Session {
    /// 造一个会话：追加第 1 条事件 `session.created`，它的 `cause` 是造会话的那个命令
    /// （`04-核心协议.md` 的 `session.create`）。这一条落了盘，再回应这个命令。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：第 1 条是 `session.created`，账本收它。
    pub fn create(
        id: CommandId,
        by: By,
        at: Timestamp,
        created: SessionCreated,
    ) -> (Session, Vec<Action>) {
        let mut session = Session {
            ledger: Ledger::default(),
            unstored: Vec::new(),
            stored: None,
            waiting: Vec::new(),
            recent: Recent::default(),
        };
        let event = session.record(&id, &by, at, Body::SessionCreated(created));
        let actions = session.accept(id, vec![event]);
        (session, actions)
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
        }
    }

    /// 收到一个命令。接受过的编号照上一次回应；新的照命令判。
    fn receive(&mut self, received: Received) -> Vec<Action> {
        if let Some(events) = self.recent.get(&received.id) {
            let events = events.to_vec();
            return self.reply_when_stored(received.id, events);
        }
        match received.command {
            Command::Send { blocks } => {
                if blocks.is_empty() {
                    return vec![rejected(received.id, Reason::EmptyMessage)];
                }
                let body = Body::MessageUser(MessageUser { blocks });
                let event = self.record(&received.id, &received.by, received.at, body);
                self.accept(received.id, vec![event])
            }
        }
    }

    /// 造一条这个命令产生的事件：`at`、`by`、`cause` 取自命令。交给账本查过，记在账上。
    fn record(&mut self, id: &CommandId, by: &By, at: Timestamp, body: Body) -> Event {
        let event = Event {
            seq: self.ledger.next_seq(),
            at,
            turn: None,
            by: by.clone(),
            cause: Some(id.clone()),
            body,
        };
        if let Err(error) = self.ledger.append(&event) {
            panic!("内核自己造的事件过不了账本，这是内核的 bug：{error}");
        }
        event
    }

    /// 接受一个命令：追加它产生的事件，记下编号，等落了盘再回应。
    fn accept(&mut self, id: CommandId, events: Vec<Event>) -> Vec<Action> {
        let seqs: Vec<Seq> = events.iter().map(|event| event.seq).collect();
        self.recent.insert(id.clone(), seqs.clone());
        self.waiting.push((id, seqs));
        self.unstored.extend(events.iter().cloned());
        vec![Action::Append(events)]
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
    /// 第六节第 2 条：先见结果，后见回应）。`upto` 超出追加过的，多出来的不算；不比上一次
    /// 往后的，什么都不做。
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
