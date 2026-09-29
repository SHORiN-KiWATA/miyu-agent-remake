//! 排版要用的几样资料：代码怎么着色、公式怎么转写、要写的几个字。

use serde::Deserialize;

use super::{Languages, Math};

/// Markdown 里要写的几个字（`text/zh.json` 的 `markdown`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Labels {
    /// 图片那一行的开头：`[图片: `。
    pub image: String,
    /// `<details>` 没写 `<summary>` 时的标题。
    pub details: String,
    /// `<details>` 标题前面的记号：收着的、展开着的（后面带一格空）。
    pub details_marks: [String; 2],
    /// 无序列表的记号：第一层、第二层、第三层起。
    pub bullets: [String; 3],
}

/// 排版要用的几样。
pub struct Kit<'a> {
    /// 代码着色照哪种语言认什么（`code.json`）。
    pub languages: &'a Languages,
    /// 行内公式的转写（`math.json`）。
    pub math: &'a Math,
    /// 要写的几个字。
    pub labels: &'a Labels,
}
