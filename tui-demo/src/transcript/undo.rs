//! 撤销说明那一行（蓝图 `tui.md`「正文」第 5、6 条）：看着撤的照撤销的回应画；补发时没有回应，照补发来的事件画
//! （`core/replay.rs` 的 `Push::UndoLine`、`UndoFiles`、`UndoGone`）。

use super::{Kind, Transcript};
use crate::core::{Report, UndoFile};

impl Transcript {
    /// 画一行撤销说明：全文照这一次撤掉的第一轮里你说的话，没有的照回应里的第一行。
    pub(super) fn undo_line(&mut self, report: Report) {
        let said = self
            .entries
            .iter()
            .find(|e| e.kind == Kind::User && e.turn.is_some_and(|t| self.reverted.contains(&t)))
            .map(|e| e.text.clone())
            .or_else(|| report.said.clone())
            .unwrap_or_default();
        self.note(Kind::Undo, said);
        if let Some(last) = self.entries.last_mut() {
            last.undo = Some(report);
        }
    }

    /// 补发来的改回的文件：接在最近那一行撤销说明上（没有差异，差异只在回应里）。
    pub(super) fn undo_files(&mut self, files: Vec<UndoFile>) {
        if let Some(entry) = self.entries.iter_mut().rev().find(|e| e.kind == Kind::Undo) {
            entry.undo.get_or_insert_with(Report::default).files = files;
        }
    }

    /// 恢复、重做：去掉最近那一行撤销说明，不另写一句。
    pub(super) fn undo_gone(&mut self) {
        if let Some(i) = self.entries.iter().rposition(|e| e.kind == Kind::Undo) {
            self.entries.remove(i);
            self.removed(i);
        }
    }
}
