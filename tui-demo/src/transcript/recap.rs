//! 回顾（蓝图 `tui.md`「回顾」）：画在正文末尾，跟着它讲到的最后一轮走。

use super::{Kind, Transcript};
use crate::config::Texts;

impl Transcript {
    /// 正文末尾画一段回顾（第 2 条），记下正文里最后一轮：撤销了那一轮它跟着藏起来，恢复了再露出来（第 5 条）。
    pub(super) fn recap(&mut self, text: &str, texts: &Texts) {
        let covers = self
            .entries
            .iter()
            .rev()
            .filter(|e| !e.hidden)
            .find_map(|e| e.turn);
        self.note(Kind::Recap, texts.recap.label.replace("{text}", text));
        if let Some(entry) = self.entries.last_mut() {
            entry.covers = covers;
        }
    }
}
