//! mermaid 图的收发（蓝图 `tui.md`「图片、公式和 mermaid 图」第 4 条）：排正文时记进单子的源码，主循环每一帧以后交给
//! 核心画；回来了记进账、扔掉排好的行重排（占位换成图）。

use super::App;
use crate::core::{Command, Update};

impl App {
    /// 把单子上的发出去（`tick`）。
    pub(super) fn send_diagram_asks(&mut self) {
        for source in self.diagrams.borrow_mut().take() {
            self.core.send(Command::RenderMermaid(source));
        }
    }

    /// 单子上有没发的：主循环马上醒来发（`deadline.rs`）。
    pub(super) fn diagram_asks_pending(&self) -> bool {
        self.diagrams.borrow().pending()
    }

    /// 图回来了：记进账，排好的行扔掉重排，交回 `true`。核心断开了：没回来的忘掉，重连以后再问（不拿走这一条）。
    pub(super) fn diagram_update(&mut self, update: &mut Option<Update>) -> bool {
        match update.take() {
            Some(Update::Mermaid { source, svg }) => {
                self.diagrams.borrow_mut().got(source, svg);
                *self.row_cache.borrow_mut() = Default::default();
                true
            }
            other => {
                if matches!(other, Some(Update::Disconnected)) {
                    self.diagrams.borrow_mut().lost();
                }
                *update = other;
                false
            }
        }
    }
}
