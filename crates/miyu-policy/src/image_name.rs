//! 带名字的图片的三句（施工 3-9 四补，`docs/blueprint/drivers/openai-chat.md` 第 9 条）：随核心附带的字，存进策略快照的
//! `core.drivers.image_name`；以前造的快照里没有，读成没有，带名字的图片照不带名字的写。

use miyu_drivers::ImageNameSources;
use serde::{Deserialize, Serialize};

/// 带名字的图片的三句，每一格是 `resources/core/drivers/` 下同名（下划线换成 `-`）文件的原文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageNameTexts {
    /// 能看图时图片前面，带文件名（`image-open.txt`）。
    pub image_open: String,
    /// 能看图时图片后面（`image-close.txt`）。
    pub image_close: String,
    /// 不能看图时的占位，带文件名（`image-omitted-named.txt`）。
    pub image_omitted_named: String,
}

impl ImageNameTexts {
    /// 交给驱动的原文。
    pub(crate) fn sources(&self) -> ImageNameSources<'_> {
        ImageNameSources {
            image_open: &self.image_open,
            image_close: &self.image_close,
            image_omitted_named: &self.image_omitted_named,
        }
    }
}
