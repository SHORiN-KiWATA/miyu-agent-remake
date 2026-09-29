//! 贴在输入框上面的几块面板上的字（`text/zh.json`；蓝图 `tui.md`「输入历史列表」）。

use serde::Deserialize;

/// 输入历史列表上的字。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryTexts {
    /// 标题。
    pub title: String,
    /// 标题后面的条数，`{count}`。
    pub count: String,
    /// 搜了字时接在条数后面，再跟打的字。
    pub query: String,
    /// 一条都对不上时那一行。
    pub empty: String,
    /// 好几行的，第一行后面跟着的，`{count}` 还有几行。
    pub lines: String,
    /// 展开的一条太长时最后一行，`{count}` 还有几行。
    pub more: String,
    /// 最后一行按键提示。
    pub hints: String,
    /// 一分钟以内发的。
    pub now: String,
    /// `{n}` 分钟前发的。
    pub minutes: String,
    /// `{n}` 小时前发的。
    pub hours: String,
    /// 还没发过话时按 Ctrl+R 的提示。
    pub none: String,
}

/// 斜杠命令列表上的字（蓝图「斜杠命令列表」第 3 条）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuTexts {
    /// 标题。
    pub title: String,
    /// 标题后面的条数，`{count}`。
    pub count: String,
}
