//! 时间线的样子（`resources/timeline.json`、`text/zh.json` 的 `summary`，蓝图「时间线」）：图标、转圈、预览几行、
//! 收起那一行的说法、哪几样默认铺开。

use std::collections::HashMap;

use serde::Deserialize;

/// 时间线收起那一行的几种说法。两个的是 `[一个的写法, 几个的写法]`，`{count}` 是几个。
#[derive(Debug, Clone, Deserialize)]
pub struct Summary {
    /// 跑过命令：打头的那一格。
    pub ran: [String; 2],
    /// 没跑命令、用过别的工具：打头的那一格。
    pub used: [String; 2],
    /// 只编辑过：打头的那一格。
    pub made: [String; 2],
    /// 编辑。
    pub edits: [String; 2],
    /// 别的工具。
    pub tools: [String; 2],
    /// 思考。
    pub thoughts: [String; 2],
    /// 出错。
    pub errors: [String; 2],
    /// 只想过：`{elapsed}` 是想了多久。
    pub thought_for: String,
}

/// 时间线的样子：图标、转圈、预览几行。头自己定的（`13-终端界面.md` 第三节的表：图标、连接行归终端界面）。
#[derive(Debug, Clone, Deserialize)]
pub struct Timeline {
    /// 每件工具的图标和它算哪一类（`command`、`edit`，没写的算工具）。
    pub tools: HashMap<String, ToolLook>,
    /// 没登记的工具的图标。
    pub tool_icon: String,
    /// 思考的图标。
    pub think_icon: String,
    /// 出错时顶替图标的叉。
    pub error_icon: String,
    /// 转圈的一帧帧。
    pub spinner: Vec<String>,
    /// 转圈一帧多少毫秒。
    pub spinner_ms: u64,
    /// 命令的预览最多几行。
    pub preview_rows: usize,
    /// 思考滚着显示最后几行。
    pub thought_rows: usize,
    /// 步与步之间的连接线，预览行首的竖线也是它。
    pub line: String,
    /// 预览放不下时那一行打头的符号。
    pub omitted: String,
    /// 一段做完要不要收成一行（蓝图「时间线」第 18 条）。
    pub fold: bool,
    /// 哪几样默认铺开全文。
    pub expand: Expand,
}

/// 一件工具在时间线上的样子。
#[derive(Debug, Clone, Deserialize)]
pub struct ToolLook {
    /// 图标。
    pub icon: String,
    /// 算哪一类；没写的算工具。
    #[serde(default)]
    pub kind: Option<ToolKind>,
}

/// 一件工具在时间线上算哪一类：数收起那一行、画预览和差异照它。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    /// 执行命令：标题写短标题，下面预览命令。
    Command,
    /// 编辑、写入：标题写加减的行数，点开是差异。
    Edit,
}

/// 时间线里哪几样默认铺开全文（`timeline.json` 的 `expand`，蓝图「时间线」第 18 条）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expand {
    /// 思考。
    pub thought: bool,
    /// 执行命令。
    pub command: bool,
    /// 编辑、写入的差异。
    pub edit: bool,
}
