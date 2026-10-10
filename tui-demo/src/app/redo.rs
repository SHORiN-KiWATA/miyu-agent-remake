//! 重做、编辑上一句（蓝图 `tui.md`「斜杠命令」`/redo`、「输入框」第 13 条，核心的 `session.redo`）：先在界面上查得
//! 出来的（在回答、最后一轮不是你说的话开的）当场提示；发出去的同时先藏掉重来的那一轮、把重发的那句画在它的位置，
//! 核心推来的序号配给它；核心拒了，那句撤掉、那一轮显示回来、编辑的字放回输入框。说的话没发出去（附件传不上、核心
//! 拒了）也照这样撤回来（「输入框」第 12 条）。

use super::paste;
use super::{App, Unsent};
use crate::core::Command;
use crate::input::Draft;
use crate::transcript::Chip;

impl App {
    /// `/redo`：这一轮原样重来一遍。
    pub(super) fn redo(&mut self) {
        let Some((text, chips)) = self.redoable() else {
            return;
        };
        let turn = self.send_redo(text, chips);
        self.core.send(Command::Redo {
            text: None,
            files: None,
        });
        self.unsent = Some(Unsent {
            draft: None,
            original: None,
            turn,
        });
    }

    /// `/edit`：开最后一轮的那句放进输入框改。输入历史里找得到的照发出去时的样子，长文、附件还是块。
    pub(super) fn edit_last(&mut self) {
        let Some((text, chips)) = self.redoable() else {
            return;
        };
        let draft = self
            .input
            .sent_by_text(&text)
            .unwrap_or_else(|| Draft::from_pasted(&text, &paste::pasted(chips)));
        self.input.start_edit(draft);
    }

    /// 编辑上一句回车了：照改过的重来。字照改过的发；附件和改之前的不一样才带（换成没有的带空的）。
    pub(super) fn submit_edit(&mut self, original: Draft, mut draft: Draft) {
        // 收不了的附件改成文件块写路径（「输入框」第 12 条「照模型收」）。
        self.demote_for_send(&mut draft);
        let files = draft.attachments();
        let files = (files != original.attachments()).then_some(files);
        let turn = self.send_redo(draft.text.clone(), paste::chips(&draft));
        self.core.send(Command::Redo {
            text: Some(draft.expand()),
            files,
        });
        self.input.remember(draft.clone());
        self.input.forget_cleared();
        self.unsent = Some(Unsent {
            draft: Some(draft),
            original: Some(original),
            turn,
        });
    }

    /// 重做发出去的那一下：重来的那一轮先藏掉，重发的那句画在它的位置，不先在底下闪一下；视口跟到最新，不守撤掉
    /// 之前的底边（长回答撤掉以后视口会停在空处，那句在屏幕外）。交回藏掉的是第几轮。
    fn send_redo(&mut self, text: String, chips: Vec<Chip>) -> Option<u64> {
        let turn = self.transcript.hide_last_turn();
        self.transcript.user(text, chips);
        self.view.settle();
        self.view.follow();
        turn
    }

    /// 没发出去（核心拒了、附件传不上）：先画进正文的那句撤掉，重做先藏掉的那一轮显示回来；字放回输入框，编辑上一句的
    /// 接着编辑（框里已经有字的不动）。
    pub(super) fn take_back_unsent(&mut self) {
        self.transcript.drop_unsent();
        let Some(unsent) = self.unsent.take() else {
            return;
        };
        if let Some(turn) = unsent.turn {
            self.transcript.show_turn(turn);
        }
        let Some(draft) = unsent.draft.filter(|_| self.input.editor.is_empty()) else {
            return;
        };
        if let Some(original) = unsent.original {
            self.input.start_edit(original);
        }
        self.input.editor.set_draft(draft);
    }

    /// 能不能重做：在回答时、最后一轮不是你说的话开的，提示一句交回 `None`（不找核心）；能的交回那句的字和块。
    fn redoable(&mut self) -> Option<(String, Vec<Chip>)> {
        let texts = &self.config.text;
        if self.transcript.running.is_some() {
            let note = texts
                .refusal_hints
                .get("turn_running")
                .cloned()
                .unwrap_or_default();
            self.hint(note, false);
            return None;
        }
        let Some(said) = self.transcript.last_said() else {
            let note = texts
                .refusal_hints
                .get("not_redoable")
                .cloned()
                .unwrap_or_default();
            self.hint(note, false);
            return None;
        };
        Some((said.text.clone(), said.pasted.clone()))
    }
}
