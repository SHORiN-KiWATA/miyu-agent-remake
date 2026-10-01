//! 替看不了图的模型看图的事件 `image.described`（施工 8-17，`docs/blueprint/kernel/events-bodies.md`、`models.md`「怎么走」
//! 第十三条）：主对话的模型看不了图，`models.vision` 替它看过一张图，内核记下转述。以后每次请求照它把图换成这段字。
//!
//! `by` 是内核，不带回合编号：转述挂在图上，不属于哪一轮，撤哪一轮都不拿走。不渲染：转述经统一的请求的 `described`
//! 进请求（`kernel/request.md`「替它看的图」）。

use serde::{Deserialize, Serialize};

use crate::id::{ContentHash, ModelName, ProviderId};

/// `image.described`：一张图的转述。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageDescribed {
    /// 哪一张图：图片块的 `blob`。
    pub blob: ContentHash,
    /// 替它看的供应商：一次性入口真发给的那一家。
    pub endpoint: ProviderId,
    /// 替它看的模型。
    pub model: ModelName,
    /// 转述的原文，去掉了前后空白。内核不记空的，读的时候不查。
    pub text: String,
}

#[cfg(test)]
mod tests;
