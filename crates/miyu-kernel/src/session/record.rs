//! 造事件、记事件（施工 O-13 上从 `session.rs` 挪出来，那边到了上限）：造一条交给账本查过、记在账上、交给有效历史，等着落盘；
//! 回合进行中造的带上这个回合，旁听的消息、`events.append` 记的不带。

use super::Session;
use crate::event::{Body, Event};
use crate::id::{CommandId, Seq, TurnId};
use crate::ledger::LedgerError;
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 造一条事件：交给账本查过，记在账上，交给有效历史，等着落盘。回合进行中造的，带上
    /// 这个回合的编号；`turn.started` 带它自己的序号（`03-事件模型.md` 第二节）。
    pub(super) fn record(
        &mut self,
        at: Timestamp,
        by: By,
        cause: Option<CommandId>,
        body: Body,
    ) -> Event {
        let seq = self.ledger.next_seq();
        let turn = match body {
            Body::TurnStarted(_) => Some(TurnId::new(seq)),
            _ => self.turn.as_ref().map(|turn| turn.id),
        };
        self.stamped(seq, at, turn, (by, cause), body)
    }

    /// 同 [`Session::record`]，不带回合编号（施工 O-13 上）：旁听的消息、`events.append` 记的，回合中途来的也不算这一轮的。
    pub(super) fn record_outside(
        &mut self,
        at: Timestamp,
        by: By,
        cause: Option<CommandId>,
        body: Body,
    ) -> Event {
        let seq = self.ledger.next_seq();
        self.stamped(seq, at, None, (by, cause), body)
    }

    /// 造好序号是 `seq`、回合编号是 `turn` 的一条，交给账本、记下。
    fn stamped(
        &mut self,
        seq: Seq,
        at: Timestamp,
        turn: Option<TurnId>,
        (by, cause): (By, Option<CommandId>),
        body: Body,
    ) -> Event {
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
    pub(super) fn commit(&mut self, event: &Event) -> Result<(), LedgerError> {
        self.ledger.append(event)?;
        self.duty.note(event);
        self.naming.note(event);
        self.reference.note(event);
        self.sight.note(event);
        self.grants.note(event);
        self.history.append(event.clone());
        self.unstored.push(event.clone());
        Ok(())
    }
}
