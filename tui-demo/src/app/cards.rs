//! 链接卡片的收发（蓝图 `tui.md`「链接卡片」第 2、4 条）：排正文时记进单子的卡片、图，主循环每一帧以后发给核心；回来了记进
//! 账、扔掉排好的行重排（卡片换掉了那一行链接）。

use super::App;
use crate::core::{Command, Update};

impl App {
    /// 把单子上的发出去（`tick`）。
    pub(super) fn send_card_asks(&mut self) {
        let (cards, blobs) = self.cards.borrow_mut().take();
        for url in cards {
            self.core.send(Command::LinkPreview(url));
        }
        for blob in blobs {
            self.core.send(Command::FetchBlob(blob));
        }
    }

    /// 单子上有没发的：主循环马上醒来发（`deadline.rs`）。
    pub(super) fn card_asks_pending(&self) -> bool {
        self.cards.borrow().pending()
    }

    /// 卡片、图回来了：记进账，排好的行扔掉重排。是这两种的交回 `true`。
    pub(super) fn card_update(&mut self, update: &mut Option<Update>) -> bool {
        match update.take() {
            Some(Update::LinkCard { url, card }) => {
                self.cards.borrow_mut().got_card(url, card);
            }
            Some(Update::BlobSaved { blob, path }) => {
                self.cards.borrow_mut().saved(blob, path);
            }
            other => {
                *update = other;
                return false;
            }
        }
        *self.row_cache.borrow_mut() = Default::default();
        true
    }
}
