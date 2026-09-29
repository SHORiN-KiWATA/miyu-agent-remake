//! 文本文件照字放进消息的三句（施工 3-9 三补，`docs/blueprint/drivers/openai-chat.md` 第 9 条）：随核心附带的字，存进
//! 策略快照的 `core.drivers.text_file`；以前造的快照里没有，读成没有，文本文件照别的文件写占位。

use miyu_drivers::TextFileSources;
use serde::{Deserialize, Serialize};

/// 文本文件的三句，每一格是 `resources/core/drivers/` 下同名（下划线换成 `-`）文件的原文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextFileTexts {
    /// 开头，带文件名（`file-open.txt`）。
    pub file_open: String,
    /// 截过的：给了多少、一共多少字节（`file-cut.txt`）。
    pub file_cut: String,
    /// 收尾（`file-close.txt`）。
    pub file_close: String,
}

impl TextFileTexts {
    /// 交给驱动的原文。
    pub(crate) fn sources(&self) -> TextFileSources<'_> {
        TextFileSources {
            file_open: &self.file_open,
            file_cut: &self.file_cut,
            file_close: &self.file_close,
        }
    }
}
