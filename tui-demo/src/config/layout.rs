//! 布局的数值（`resources/layout.json`）：宽度、留白、列表几行、提示停多久、压缩进度条怎么动这些（蓝图 `tui.md`
//! 「资源文件」）。

use std::collections::HashMap;

use serde::Deserialize;

use super::CompactionMotion;
use crate::core::Level;

/// 布局的数值。
#[derive(Debug, Clone, Deserialize)]
pub struct Layout {
    /// 输入框和正文占窗口宽度的百分之几，跟着窗口一起变宽。
    pub width_percent: u16,
    /// 配置页的尺寸（蓝图「配置页」第 3、14 条）。
    pub settings: crate::ui::settings::Look,
    /// 输入法跟着打字状态切（蓝图「配置页」第 30 条）。
    pub ime: crate::ime::Look,
    /// 终端窄到这个宽度以下，输入框不留边、占满整行。
    pub narrow_below: u16,
    /// 空会话的首页上，输入框（连边框）最宽几列（`tui.md`「空会话的首页」第 2 条）。
    pub home_max_width: u16,
    /// 窗口至少这么宽才有侧边栏（`tui.md`「后台命令、子代理和侧边栏」第 6 条）。
    pub sidebar_from: u16,
    /// 侧边栏多宽（不算和主列之间那根竖线）。
    pub sidebar_width: u16,
    /// 首页画不画吉祥物（蓝图「后台命令、子代理和侧边栏」第 7 条）。
    pub mascot_home: bool,
    /// 侧边栏上下文那一段的进度条：占了的、没占的。压缩那一行的进度条也照它。
    pub bar: Bar,
    /// 压缩那一行的进度条怎么动（蓝图「正文」第 9 条）。
    pub compaction: CompactionMotion,
    /// 侧边栏画不画吉祥物。
    pub mascot_sidebar: bool,
    /// 子代理状态行最多几行（不算主会话）。
    pub agent_rows: usize,
    /// 后台命令的输出（`job.output`）：面板里露几行、结束了读几行、隔多久读一次、制表符换成几个空格。
    pub output: OutputLook,
    /// 待办每一项前面的记号。
    pub todo_marks: TodoMarks,
    /// 待办默认最多露几行项目（不算标题）；侧边栏照它剩下的高度。
    pub todo_rows: usize,
    /// 输入框和屏幕左右边之间至少留几列。
    pub side_gap: u16,
    /// 正文上面空几行，不贴着屏幕顶。
    pub top_gap: u16,
    /// 文字和框的左边之间留几列。
    pub pad_left: u16,
    /// 文字和框的右边之间留几列。至少要一列，给行尾的光标。
    pub pad_right: u16,
    /// 窗口少于这么多列换紧凑版面：框贴着两边，框里只留提示符和光标的地方（蓝图「输入框」第 10 条）。
    pub compact_below: u16,
    /// 紧凑版面框里左边几列（提示符的宽度）。
    pub compact_pad_left: u16,
    /// 紧凑版面框里右边几列（光标的一列）。
    pub compact_pad_right: u16,
    /// 一次粘贴超过这么多行，输入框里收成一块（蓝图「输入框」第 11 条）。
    pub paste_fold_lines: usize,
    /// 一次粘贴超过这么多字，收成一块。
    pub paste_fold_chars: usize,
    /// 输入框最多长到几行，再多就在框里滚。
    pub max_rows: u16,
    /// 两次点击隔多久以内算双击，毫秒。
    pub double_click_ms: u64,
    /// 按了第一下 `Esc`，多久以内再按一下才打断（抽屉开着时是取消），毫秒。
    pub esc_window_ms: u64,
    /// 抽屉带文字画时可以高过半屏，但屏幕顶上至少留这么多行（蓝图「确认和提问的抽屉」第 2 条）。
    pub drawer_keep_rows: u16,
    /// 提问抽屉里选项预览的面板怎么摆（蓝图「确认和提问的抽屉」第 3 条）。
    pub drawer_preview: crate::ui::PreviewLook,
    /// 输入框左上方的提示停多久，毫秒。
    pub notice_ms: u64,
    /// 最多每多少毫秒画一帧：这中间来的推送攒着，到点一起画（蓝图「每一帧」）。
    pub frame_ms: u64,
    /// 按了键、粘贴的不等 `frame_ms`，只再等这么多毫秒把一串一起到的收齐（输入法上屏一次好几个字）就画。
    pub key_burst_ms: u64,
    /// 她的回答排好的 Markdown 行留最近用过的几篇（蓝图「正文」第 8 条）。
    pub markdown_cache: usize,
    /// 记帧时（`MIYU_TUI_FRAME_LOG`）整帧超过这么多毫秒的，写上各块各花了多久（蓝图「环境变量」）。
    pub slow_frame_ms: u64,
    /// 整份重排时一帧最多花多少毫秒，没排完的下几帧接着排（蓝图「正文」第 8 条）。
    pub relayout_budget_ms: u64,
    /// 连不上核心时隔多久再试，`[最短, 最长]` 毫秒，每次翻倍（蓝图「连核心」第 7 条）。
    pub reconnect_ms: [u64; 2],
    /// 运行状态行词后面的三个点：一直在，和词一起被流光扫（`tui.md`「运行状态行和排队的消息」第 2 条）。
    pub dots: Dots,
    /// 运行状态行的流光。
    pub shimmer: Shimmer,
    /// 用哪套主题（`resources/themes/` 里的名字）。
    pub theme: String,
    /// 用哪套图标（`resources/icons/` 里的名字，蓝图「图标」）；没有这一套的用出厂的第一套。
    pub icons: String,
    /// `Shift+Tab` 轮换权限级别的顺序。
    pub level_cycle: Vec<Level>,
    /// 框外左下角权限级别前面的图标，一档一个，连同它后面的空格：只读是暂停符号。
    pub level_icons: HashMap<Level, String>,
    /// 输入框第一行文字前面的提示符，连同它后面的空格；颜色跟着模式。它住在 `pad_left` 那几列里。
    pub prompt: String,
    /// 排队的消息前面的记号，连同它后面的空格。
    pub queued_mark: String,
    /// 斜杠命令列表最多露出几行。取单数，选中的那一行才停得在正中间。
    pub menu_rows: usize,
    /// 输入历史列表最多露几条（`tui.md`「输入历史列表」）。
    pub history_rows: usize,
    /// 会话列表最多露几条（`tui.md`「会话列表」第 1 条）。
    pub session_rows: usize,
    /// 会话列表里工作目录最多几列，长了只写最后两层（「会话列表」第 1 条）。
    pub session_cwd_width: usize,
    /// 会话列表里标题那一列最宽几列，长了截掉加 `…`（「会话列表」第 1 条）。
    pub session_title_width: usize,
    /// 会话列表里勾上的记号，连同它后面的空格（「会话列表」第 2 条）。
    pub tick_mark: String,
    /// 输入历史列表里 Tab 展开的那一条最多几行（`tui.md`「输入历史列表」第 7 条）。
    pub history_preview_rows: usize,
    /// 输入历史文件里留最近几条（「输入历史列表」第 8 条）。
    pub history_keep: usize,
    /// 待办做完以后全打勾留多久再收，毫秒（「后台命令、子代理和侧边栏」第 4 条）。
    pub todo_linger_ms: u64,
    /// 正文里用户说的话前面那根竖线，连同它后面的空格。
    pub user_bar: String,
    /// 回顾前面的记号，连同它后面的空格（`tui.md`「回顾」第 2 条）。
    pub recap_mark: String,
    /// 会话标题最多几个字，照核心的上限（`tui.md`「改名」第 2 条）。
    pub title_max: usize,
    /// 撤销那一行前面的符号，连同它后面的空格。
    pub undo_icon: String,
    /// 回到底部的按钮（2026-10-02）。
    pub bottom_mark: String,
    /// 链接卡片的封面图几行高（蓝图「链接卡片」第 3 条）。
    pub link_cover_rows: u16,
    /// 链接卡片的网站图标几列宽。
    pub link_icon_cols: u16,
    /// 会话列表里正在用的那个标题前面的记号（2026-10-02）。
    pub current_mark: String,
    /// 撤销点开以后，改回了的文件前面的记号，连同它后面的空格（2026-10-02）。
    pub undo_restored_mark: String,
    /// 撤销点开以后，没动的文件前面的记号，连同它后面的空格。
    pub undo_left_mark: String,
    /// 一轮做完的收尾行前面的符号，连同它后面的空格。
    pub done_icon: String,
    /// 收尾行里图标后面多空的，按级别写；没写的级别不多空（现在只有工作区的 `▣` 多空一格，`tui.md`「正文」第 4 条）。
    pub done_gap: HashMap<Level, String>,
}

/// 运行状态行的流光明暗：主题的 `accent` 打底，一道亮光从左往右扫过。颜色不变，只变明暗。
#[derive(Debug, Clone, Deserialize)]
pub struct Shimmer {
    /// 亮光从头扫到尾要几秒。
    pub sweep_seconds: f64,
    /// 亮光宽几个字。
    pub band: f64,
    /// 没扫到的字亮度乘几。
    pub dim: f64,
    /// 扫到正中的字往白里偏多少，0 到 1。
    pub lift: f64,
}

/// 运行状态行词后面的点（`layout.json` 的 `dots`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dots {
    /// 点是哪个字。
    pub mark: String,
    /// 几个点。
    pub count: usize,
}

/// 进度条的两种格（`layout.json` 的 `bar`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bar {
    /// 几格。
    pub width: usize,
    /// 占了的。
    pub full: String,
    /// 没占的。
    pub empty: String,
}

/// 待办每一项前面的记号（`layout.json` 的 `todo_marks`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TodoMarks {
    /// 没做。
    pub pending: String,
    /// 在做。
    pub active: String,
    /// 做完。
    pub done: String,
}

/// 后台命令的输出怎么读、露几行（蓝图「后台命令、子代理和侧边栏」第 3、5 条）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputLook {
    /// 面板里点开一条露最后几行。
    pub panel_rows: usize,
    /// 结束了读最后几行，正文里那一行点开看（核心最多给 2000）。
    pub note_lines: usize,
    /// 在跑的隔几毫秒读一次。
    pub poll_ms: u64,
    /// 制表符换成几个空格。
    pub tab: usize,
}
