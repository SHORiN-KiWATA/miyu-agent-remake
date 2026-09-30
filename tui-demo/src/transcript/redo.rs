//! 重做、编辑上一句要的（蓝图 `tui.md`「斜杠命令」`/redo`、「输入框」第 13 条）：开最后一轮的那一句是哪条；说的话、
//! 重做没发出去时，撤掉先画进正文、还没落盘的那句（「输入框」第 12 条）。

use super::{Entry, Kind, Transcript};

impl Transcript {
    /// 开最后一轮的那一句：没被撤掉的条目里最后一轮，是你说的话开的才有。回报叫醒的、手动压缩、清空的那一轮，
    /// 一轮都没有：`None`（核心只重做人说的话开的最后一轮）。
    pub fn last_said(&self) -> Option<&Entry> {
        let shown = || self.entries.iter().filter(|e| !e.hidden);
        let last = shown().filter_map(|e| e.turn).max()?;
        // 别处来的话开的那一轮不算（「别处来的话」第 3 条）。
        shown().find(|e| e.kind == Kind::User && e.turn == Some(last) && e.from.is_none())
    }

    /// 重做发出去时先在界面上藏掉最后一轮：重发的那句落在它原来的位置，不先在底下闪一下（核心推来的 `turn.reverted`
    /// 照样再藏一次）。交回藏掉的是第几轮；最后一轮不是你说的话开的是 `None`。
    pub fn hide_last_turn(&mut self) -> Option<u64> {
        let turn = self.last_said()?.turn?;
        self.hide(&[turn], true);
        Some(turn)
    }

    /// 重做没发出去：先藏掉的那一轮显示回来。
    pub fn show_turn(&mut self, turn: u64) {
        self.hide(&[turn], false);
    }

    /// 没发出去：撤掉最后一条还没落盘（没有序号、没归到哪一轮）的你说的话。
    pub fn drop_unsent(&mut self) {
        let unsent = self.entries.iter().rposition(|e| {
            e.kind == Kind::User && e.seq.is_none() && e.turn.is_none() && e.from.is_none()
        });
        if let Some(at) = unsent {
            self.entries.remove(at);
            self.removed(at);
        }
    }
}
