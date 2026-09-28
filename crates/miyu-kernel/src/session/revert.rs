//! 撤销与恢复（`docs/designs/02-内核.md` 第六节「撤销与恢复」）：从选中的那一轮起往后全撤，
//! 她从此看不到；你发下一句之前，能一次一次地恢复。撤掉的拿走、放回，都在有效历史里做
//! （`history/undo.rs`）；能不能撤、能不能恢复，照账本。
//!
//! 撤销、恢复以后照效果改回文件（`10-自带软件.md` 第七节「改回文件的细则」，施工 4-7 上）：算出几步（`restore.rs`）
//! 交给执行器，等结局回来记一条 `files.restored`，两条都落了盘才回应。没有要改回的当场照旧。

use super::action::{Action, Reason};
use super::restore::{self, Step};
use super::{Session, rejected};
use crate::event::{Body, Event, FilesRestored, Restored, TurnReverted, TurnUnreverted};
use crate::id::{CommandId, Seq, TurnId};
use crate::origin::By;
use crate::time::Timestamp;

/// 撤销、恢复以后正在改回文件：是哪个命令、谁发的、它记的那一条的序号。
#[derive(Debug)]
pub(super) struct Restoring {
    id: CommandId,
    by: By,
    first: Seq,
}

impl Session {
    /// 从 `turn` 起撤销：记一条 `turn.reverted`，照先后列出它和它以后还在有效历史里的每一轮，
    /// `by` 是撤销的人，`cause` 是这个命令；落了盘，回应附上它的序号，头照它找到撤掉的话。
    /// 撤掉的那几轮改过文件的，先改回去（[`Session::settle_files`]）。
    ///
    /// 有回合在进行的，拒绝，原因码 `turn_running`：头先打断再撤。已经压缩进摘要的（序号落在
    /// 最近一次压缩替代掉的范围里），`compacted`；别的不在有效历史里的，`unknown_turn`。`turn` 不写的，撤还在有效
    /// 历史里的最后一轮，照账本当场找（施工 4-7 下）；一轮都没有的，`nothing_to_revert`。
    pub(super) fn revert(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        turn: Option<TurnId>,
    ) -> Vec<Action> {
        if self.turn.is_some() {
            return vec![rejected(id, Reason::TurnRunning)];
        }
        let Some(turn) = turn.or_else(|| self.ledger.last_turn()) else {
            return vec![rejected(id, Reason::NothingToRevert)];
        };
        let Some(turns) = self.ledger.turns_from(turn) else {
            let reason = match self.ledger.compacted() {
                Some(upto) if turn.started() <= upto => Reason::Compacted,
                _ => Reason::UnknownTurn,
            };
            return vec![rejected(id, reason)];
        };
        let body = Body::TurnReverted(TurnReverted { turns });
        let event = self.record(at, by.clone(), Some(id.clone()), body);
        let steps = restore::undo(self.history.last_undone(), self.history.events());
        self.settle_files(id, by, event, steps)
    }

    /// 恢复最近一次撤销：记一条 `turn.unreverted`，列的就是那一次撤掉的那几轮，`by` 是恢复的人，
    /// `cause` 是这个命令。那几轮改过的文件，跟着改回撤销前的样子。没有能恢复的（没撤过，或者撤了以后开过回合、
    /// 压缩过），拒绝，原因码 `nothing_to_unrevert`。
    pub(super) fn unrevert(&mut self, id: CommandId, by: By, at: Timestamp) -> Vec<Action> {
        let Some(turns) = self.ledger.last_reverted().map(<[TurnId]>::to_vec) else {
            return vec![rejected(id, Reason::NothingToUnrevert)];
        };
        let steps = restore::redo(self.history.last_undone(), self.history.events());
        let body = Body::TurnUnreverted(TurnUnreverted { turns });
        let event = self.record(at, by.clone(), Some(id.clone()), body);
        self.settle_files(id, by, event, steps)
    }

    /// 撤销、恢复记下了：没有要改回的文件，照旧等它落了盘就回应；有的，交出去改，结局回来再回应。
    fn settle_files(
        &mut self,
        id: CommandId,
        by: By,
        event: Event,
        steps: Vec<Step>,
    ) -> Vec<Action> {
        if steps.is_empty() {
            self.accept(id, vec![event.seq]);
            return vec![Action::Append(vec![event])];
        }
        self.restoring = Some(Restoring {
            id,
            by,
            first: event.seq,
        });
        vec![Action::Append(vec![event]), Action::Restore { steps }]
    }

    /// 改回文件做完了：记一条 `files.restored`，`by`、`cause` 和撤销、恢复的那一条一样；两条都落了盘才回应。没在改的
    /// （过时的结局）不理。
    pub(super) fn restored(&mut self, at: Timestamp, files: Vec<Restored>) -> Vec<Action> {
        let Some(Restoring { id, by, first }) = self.restoring.take() else {
            return Vec::new();
        };
        let body = Body::FilesRestored(FilesRestored { files });
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![first, event.seq]);
        vec![Action::Append(vec![event])]
    }
}
