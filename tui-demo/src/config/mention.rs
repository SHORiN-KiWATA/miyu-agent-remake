//! `@` 文件列表的数值（`resources/mention.json`）和字（`text/zh.json` 的 `mention`），蓝图 `tui.md`「`@` 文件列表」。

use serde::Deserialize;

/// 问核心的节拍。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MentionLook {
    /// 核心的清单还在建：隔多久再问一次（毫秒）。
    pub poll_ms: u64,
}

/// 列表上写的字。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MentionTexts {
    /// 上边框的开头：`{query}` 打的字。
    pub title: String,
    /// 条数：`{count}`。
    pub count: String,
    /// 按目录找。
    pub layer: String,
    /// 清单还在建。
    pub indexing: String,
    /// 清单收满了。
    pub partial: String,
    /// 一条都对不上。
    pub empty: String,
    /// 下边框的按键说明。
    pub hints: String,
}
