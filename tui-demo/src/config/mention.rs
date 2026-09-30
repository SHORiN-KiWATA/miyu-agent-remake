//! `@` 文件列表的数值（`resources/mention.json`）和字（`text/zh.json` 的 `mention`），蓝图 `tui.md`「`@` 文件列表」。

use serde::Deserialize;

/// 清单建多大、列几条、多久重建。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MentionLook {
    /// 清单最多收几个，收满就停。
    pub cap: usize,
    /// 最深走几层。
    pub depth: usize,
    /// 跳过这些名字的目录（隐藏目录本来就跳过）。
    pub skip: Vec<String>,
    /// 最多列几条。
    pub shown: usize,
    /// 隔多久以上再弹，重建一份清单（秒）。
    pub refresh_secs: u64,
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
