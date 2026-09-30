//! 找回来（蓝图 `tui.md`「输入框」第 7、8、13 条、「按键」`Ctrl+C`）：撤销放回的那句、恢复时收回去；照正文里的字
//! 在输入历史里找发出去时的样子（长文、附件还是块）；编辑上一句；`Ctrl+C` 清掉的那一份什么时候不要了。

use super::{Draft, InputBox};

impl InputBox {
    /// 撤销成了：撤掉的那句放回来，整段选中，直接打字就替换掉它（打 `/restore` 不会拼在后面）；
    /// 框里已经有字的不动（`tui.md`「输入框」第 7 条）。
    pub fn put_back(&mut self, said: &str) {
        if !self.editor.is_empty() {
            return;
        }
        // 核心给的是发出去的全文：输入历史里找得到这句的，照发出去时的样子放回来，长文、附件还是块（第 7 条）。
        let sent = self.history.iter().rev().find(|s| s.draft.expand() == said);
        match sent.map(|s| s.draft.clone()) {
            Some(draft) => self.editor.set_draft(draft),
            None => self.editor.set(said),
        }
        self.editor.select_all();
        self.put_back = Some(said.to_string());
    }

    /// 恢复了：撤销时放回来的那句还没动过的，收回去，免得回车再发一遍（`tui.md`「正文」第 6 条）。
    pub fn take_back(&mut self) {
        if self
            .put_back
            .take()
            .is_some_and(|said| self.editor.draft().expand() == said)
        {
            self.editor.take();
        }
    }

    /// 发出去一句话了：`Ctrl+C` 清掉的那一份不要了，`↑` 照旧先翻到刚发的（「按键」`Ctrl+C`）。
    pub fn forget_cleared(&mut self) {
        self.cleared = None;
    }

    /// 输入历史里最近一句写出来是 `text` 的，发出去时的样子（被退回的排队消息、编辑上一句，第 8、13 条）。
    pub fn sent_by_text(&self, text: &str) -> Option<Draft> {
        let sent = self.history.iter().rev().find(|s| s.draft.text == text);
        sent.map(|s| s.draft.clone())
    }

    /// 编辑上一句：放进输入框，光标在最后，框上写「编辑上一句 · Esc 取消」（第 13 条）。
    pub fn start_edit(&mut self, draft: Draft) {
        self.goal_col = None;
        self.follow = true;
        self.editor.set_draft(draft.clone());
        self.editing = Some(draft);
    }

    /// 接着编辑：框里的字照旧，改之前的那句是 `original`（打错了命令、核心拒了，第 13 条）。
    pub fn resume_edit(&mut self, original: Draft) {
        self.editing = Some(original);
    }

    /// 不编辑了（`Esc`）：框里清空。
    pub fn cancel_edit(&mut self) {
        self.editing = None;
        self.editor.take();
    }

    /// 在编辑上一句。
    pub fn editing(&self) -> bool {
        self.editing.is_some()
    }

    /// 回车了：不再编辑，交回改之前的那句；没在编辑是 `None`。
    pub fn take_edit(&mut self) -> Option<Draft> {
        self.editing.take()
    }
}
