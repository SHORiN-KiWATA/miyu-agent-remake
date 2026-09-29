//! 账本里撤销、恢复的几条（`docs/blueprint/kernel/history.md`「账本查的规矩」「账本记下的变化」）：撤的是哪几轮，
//! 能不能恢复；撤掉的压缩跟着这一次撤销记着，恢复时一起回来（施工 6-9）。

use super::{Compaction, Ledger};
use crate::id::TurnId;

/// 还能恢复的一次撤销：撤了哪几轮，跟着撤掉了哪几次压缩。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Undone {
    pub(super) turns: Vec<TurnId>,
    compactions: Vec<Compaction>,
}

impl Ledger {
    /// 撤销：没有回合在进行；撤的是还在有效历史里的某一轮，和它以后还在的每一轮，照先后，一轮
    /// 不漏（`02-内核.md` 第六节「撤销与恢复」）。中间的一轮不能单独撤：后面几轮都是看着它做的。
    pub(super) fn check_revert(&self, turns: &[TurnId]) -> Result<(), String> {
        if let Some(open) = self.open {
            return Err(format!(
                "turn {open} is still running; nothing can be undone"
            ));
        }
        let Some(&first) = turns.first() else {
            return Err("the list of undone turns is empty".to_string());
        };
        if let Some(turn) = turns
            .iter()
            .find(|turn| self.turns.binary_search(turn).is_err())
        {
            return Err(format!(
                "turn {turn} is not in the current history: no such turn, or already undone"
            ));
        }
        let expected = self.turns_from(first).unwrap_or_default();
        match turns == expected.as_slice() {
            true => Ok(()),
            false => Err(format!(
                "undo every turn from {first} on, in order: {}",
                listed(&expected)
            )),
        }
    }

    /// 恢复：正好是最近一次撤销的那几轮；那以后没开过回合，也没压缩过。
    pub(super) fn check_unrevert(&self, turns: &[TurnId]) -> Result<(), String> {
        match self.last_reverted() {
            None => Err(
                "nothing to redo: no undo yet, or a turn or a compaction came after it".to_string(),
            ),
            Some(last) if last != turns => Err(format!(
                "redo the turns of the latest undo: {}",
                listed(last)
            )),
            Some(_) => Ok(()),
        }
    }

    /// 查过了：撤的正好是从第一轮起还没撤掉的后面一截；这几轮里的压缩跟着撤掉，记在这一次撤销上。
    pub(super) fn record_revert(&mut self, reverted: &[TurnId]) {
        let Some(&first) = reverted.first() else {
            return;
        };
        let turns = self.turns.partition_point(|turn| *turn < first);
        self.turns.truncate(turns);
        let kept = self
            .compactions
            .partition_point(|compaction| compaction.turn < first);
        self.undone.push(Undone {
            turns: reverted.to_vec(),
            compactions: self.compactions.split_off(kept),
        });
    }

    /// 最近一次撤销去掉，那几轮和跟着撤掉的压缩回来。
    pub(super) fn record_unrevert(&mut self) {
        if let Some(undone) = self.undone.pop() {
            self.turns.extend(undone.turns);
            self.compactions.extend(undone.compactions);
        }
    }
}

/// 几个回合编号，写成「11, 12」。
fn listed(turns: &[TurnId]) -> String {
    let turns: Vec<String> = turns.iter().map(ToString::to_string).collect();
    turns.join(", ")
}
