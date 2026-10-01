//! 替看不了图的模型看图的字（施工 8-17，`docs/blueprint/models.md`「怎么走」第十三条第 10 条）：随核心附带，造会话时冻结在
//! 策略快照里。两处：转述那一次请求的两份（`core.vision`，交给组装器），主请求里图的位置的三份标签（`core.drivers
//! .image_description`，交给驱动）。以前造的快照里都没有，读成没有：那些会话不转述，看不了图的照旧写占位。

use miyu_assemble::Vision;
use miyu_drivers::ImageDescriptionSources;
use serde::{Deserialize, Serialize};

use crate::snapshot::Snapshot;

/// 转述一张图的请求要用的两份（`vision/` 下，文件名是同名的 `.txt`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisionTexts {
    /// 指令，以换行结尾。
    pub instruction: String,
    /// 人的话前面那一行：以空行开头、以换行结尾。
    pub question: String,
}

/// 替它看的图的三句标签，每一格是 `resources/core/drivers/` 下同名（下划线换成 `-`）文件的原文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageDescriptionTexts {
    /// 转述前面，不带名字的图（`image-description-open.txt`）。
    pub image_description_open: String,
    /// 转述前面，带名字的图（`image-description-open-named.txt`），字段是 `name`。
    pub image_description_open_named: String,
    /// 转述后面（`image-description-close.txt`）。
    pub image_description_close: String,
}

impl ImageDescriptionTexts {
    /// 交给驱动的原文。
    pub(crate) fn sources(&self) -> ImageDescriptionSources<'_> {
        ImageDescriptionSources {
            image_description_open: &self.image_description_open,
            image_description_open_named: &self.image_description_open_named,
            image_description_close: &self.image_description_close,
        }
    }
}

impl Snapshot {
    /// 交给组装器的转述一张图的字：快照里有的才有。
    pub(crate) fn vision(&self) -> Option<Vision> {
        let vision = self.core.vision.as_ref()?;
        Some(Vision {
            instruction: vision.instruction.clone(),
            question: vision.question.clone(),
        })
    }
}
