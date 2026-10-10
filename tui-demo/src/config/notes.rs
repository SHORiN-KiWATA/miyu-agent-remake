//! 正文里几种旁白上的字（`text/zh.json`；蓝图 `tui.md`「正文」第 9 条）。

use serde::Deserialize;

/// 压缩那几行（照 `miyu ask` 的写法，`cli/ask.md`「压缩那一行」）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactionTexts {
    /// 提前在后台开始压时弹的提示（「正文」第 9 条）。
    pub triggered: String,
    /// 压缩中那一行被流光扫的那几个字；后面的数字照 `written` 写。
    pub progress: String,
    /// 压缩中那一行后面的数字，`{written}` 已经写了多少字（三位一撇）。
    pub written: String,
    /// 进度条后面的百分比，`{percent}`；前面是不断行空格（kitty 会把符号后面跟着普通空格的画成两格宽）。
    pub percent: String,
    /// 压好了，`{before}`、`{after}` 压前压后的 token 数。
    pub done: String,
    /// 压好了那一行前面的记号，连同它后面的空格：绿色（主题的 `good`）。
    pub done_mark: String,
    /// 清空了（`/clear`）那一行，前面照样是 `done_mark`。
    pub cleared: String,
    /// 摘要请求出错，`{reason}` 原因。
    pub failed: String,
    /// 摘要请求里调了工具（取不出摘要的一种）。
    pub called_a_tool: String,
    /// 调了工具、改走隔离式那一行（施工 6-6 下）：不是失败，灰。
    pub isolating: String,
    /// 暂停了：连续失败，`{n}` 几次。
    pub paused_failures: String,
    /// 暂停了：内容太大，`{entry}` 哪一条。
    pub paused_too_large: String,
    /// 暂停了：别的、缺了次数或序号的。
    pub paused: String,
}
