//! 开不了链接、文件时的提示（`text/zh.json` 的 `open`），蓝图 `tui.md`「她的回答：Markdown」第 10 条。

use serde::Deserialize;

/// 开不了时说什么。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenTexts {
    /// 不开这种协议：`{url}` 地址。
    pub refused: String,
    /// 本机的文件不在了：`{path}` 路径。
    pub missing: String,
    /// 起不来打开它的程序：`{reason}` 原因。
    pub failed: String,
}
