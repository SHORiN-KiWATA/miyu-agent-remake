//! 撤销与恢复（`docs/designs/02-内核.md` 第六节「撤销与恢复」）：从选中的那一轮起往后全撤，
//! 她从此看不到；你发下一句之前，能一次一次地恢复。撤掉的拿走、放回，都在有效历史里做
//! （`history/undo.rs`）；能不能撤、能不能恢复，照账本。

use super::action::{Action, Reason};
use super::{Session, rejected};
use crate::event::{Body, TurnReverted, TurnUnreverted};
use crate::id::{CommandId, TurnId};
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 从 `turn` 起撤销：记一条 `turn.reverted`，照先后列出它和它以后还在有效历史里的每一轮，
    /// `by` 是撤销的人，`cause` 是这个命令；落了盘，回应附上它的序号，头照它找到撤掉的话。
    ///
    /// 有回合在进行的，拒绝，原因码 `turn_running`：头先打断再撤。已经压缩进摘要的（序号落在
    /// 最近一次压缩替代掉的范围里），`compacted`；别的不在有效历史里的，`unknown_turn`。
    pub(super) fn revert(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        turn: TurnId,
    ) -> Vec<Action> {
        if self.turn.is_some() {
            return vec![rejected(id, Reason::TurnRunning)];
        }
        let Some(turns) = self.ledger.turns_from(turn) else {
            let reason = match self.ledger.compacted() {
                Some(upto) if turn.started() <= upto => Reason::Compacted,
                _ => Reason::UnknownTurn,
            };
            return vec![rejected(id, reason)];
        };
        let body = Body::TurnReverted(TurnReverted { turns });
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }

    /// 恢复最近一次撤销：记一条 `turn.unreverted`，列的就是那一次撤掉的那几轮，`by` 是恢复的人，
    /// `cause` 是这个命令。没有能恢复的（没撤过，或者撤了以后开过回合、压缩过），拒绝，原因码
    /// `nothing_to_unrevert`。
    pub(super) fn unrevert(&mut self, id: CommandId, by: By, at: Timestamp) -> Vec<Action> {
        let Some(turns) = self.ledger.last_reverted().map(<[TurnId]>::to_vec) else {
            return vec![rejected(id, Reason::NothingToUnrevert)];
        };
        let body = Body::TurnUnreverted(TurnUnreverted { turns });
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }
}
