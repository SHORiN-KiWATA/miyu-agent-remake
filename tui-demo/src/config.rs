//! 界面上的字、布局的数值、模型的窗口：住在 `resources/` 的 JSON 里，编译时带进来。
//!
//! 仓库的规矩是「无硬编码」（`AGENTS.md`「代码的规矩」）。界面上的字照仓库
//! `resources/core/human/` 的做法，一种语言一份；演示先只有中文。

use std::collections::HashMap;

use serde::Deserialize;

use crate::commands::Commands;
use crate::core::Level;
use crate::jobs::Script;
use crate::markdown::{Languages, Math};
use crate::mascot::Look;
use crate::pulse::Words;
use crate::theme::Palette;

/// 布局的数值。
#[derive(Debug, Clone, Deserialize)]
pub struct Layout {
    /// 输入框和正文占窗口宽度的百分之几，跟着窗口一起变宽。
    pub width_percent: u16,
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
    /// 侧边栏上下文那一段的进度条：占了的、没占的。
    pub bar: Bar,
    /// 侧边栏画不画吉祥物。
    pub mascot_sidebar: bool,
    /// 子代理状态行最多几行（不算主会话）。
    pub agent_rows: usize,
    /// 后台面板里点开一条命令，展开最后几行输出。
    pub job_preview_rows: usize,
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
    /// 输入框最多长到几行，再多就在框里滚。
    pub max_rows: u16,
    /// 两次点击隔多久以内算双击，毫秒。
    pub double_click_ms: u64,
    /// 在回答时按了第一下 `Esc`，多久以内再按一下才打断，毫秒。
    pub esc_window_ms: u64,
    /// 输入框左上方的提示停多久，毫秒。
    pub notice_ms: u64,
    /// 运行状态行词后面的三个点：一直在，和词一起被流光扫（`tui.md`「运行状态行和排队的消息」第 2 条）。
    pub dots: Dots,
    /// 运行状态行的流光。
    pub shimmer: Shimmer,
    /// 用哪套主题（`resources/themes/` 里的名字）。
    pub theme: String,
    /// 工具的显示名、结果那一句用哪种语言（仓库 `resources/software/basesystem/human/<它>.json`）。
    pub tool_language: String,
    /// `Shift+Tab` 轮换权限级别的顺序。
    pub level_cycle: Vec<Level>,
    /// 框外左下角权限级别前面的图标，一档一个，连同它后面的空格：只读是暂停符号。
    pub level_icons: HashMap<Level, String>,
    /// 输入框第一行文字前面的提示符，连同它后面的空格；颜色跟着模式。它住在 `pad_left` 那几列里。
    pub prompt: String,
    /// 排队的消息前面的记号，连同它后面的空格。
    pub queued_mark: String,
    /// 暂存着东西时的提示符，连同它后面的空格。
    pub stash_prompt: String,
    /// 斜杠命令列表最多露出几行。取单数，选中的那一行才停得在正中间。
    pub menu_rows: usize,
    /// 输入历史列表最多露几条（`tui.md`「输入历史列表」）。
    pub history_rows: usize,
    /// 输入历史列表里 Tab 展开的那一条最多几行（`tui.md`「输入历史列表」第 7 条）。
    pub history_preview_rows: usize,
    /// 正文里用户说的话前面那根竖线，连同它后面的空格。
    pub user_bar: String,
    /// 撤销那一行前面的符号，连同它后面的空格。
    pub undo_icon: String,
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

/// 后台命令、子代理、待办的字（蓝图「后台命令、子代理和侧边栏」）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobTexts {
    /// 框下面那一行的后台按钮，`{count}` 在跑的命令数。
    pub button: String,
    /// 后台面板的标题。
    pub title: String,
    /// 面板标题下面那行，`{count}` 在跑的命令数。
    pub active: String,
    /// 面板最下面的按键提示。
    pub hint: String,
    /// 命令后面的状态：运行中 `{elapsed}`、完成 `{elapsed}`、失败 `{code}`、已停止。
    pub running: String,
    /// 见 `running`。
    pub done: String,
    /// 见 `running`。
    pub failed: String,
    /// 见 `running`。
    pub stopped: String,
    /// 正文里的通知：后台命令完成 `{title}` `{elapsed}`。
    pub done_note: String,
    /// 后台命令失败 `{title}` `{code}`。
    pub failed_note: String,
    /// 后台命令停了 `{title}`。
    pub stopped_note: String,
    /// 后台任务（子代理）完成 `{title}` `{elapsed}`。
    pub agent_note: String,
    /// 通知前面成功的记号（绿）。
    pub ok_mark: String,
    /// 通知前面失败的记号（红）。
    pub fail_mark: String,
    /// 子代理状态行第一行：主会话。
    pub main: String,
    /// 收起来的，`{count}` 个。
    pub more: String,
    /// 点子代理那一行：还不能切进去。
    pub switch_todo: String,
    /// 待办的进度，`{done}` `{total}`。
    pub todo: String,
    /// 待办长了：做完的收成一行，`{count}` 项。
    pub todo_folded: String,
    /// 待办长了：放不下的收成一行，`{count}` 项。
    pub todo_more: String,
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

/// 界面上给人看的字。`{count}` 这样的占位由代码填。
#[derive(Debug, Clone, Deserialize)]
pub struct Texts {
    /// 输入框空着时轮换的提示（蓝图「输入框」第 9 条）。
    pub tips: Vec<String>,
    /// 侧边栏：会话还没起名字时写的（蓝图「后台命令、子代理和侧边栏」第 7 条）。
    pub untitled: String,
    /// 侧边栏的短编号，`{id}` 是完整编号的前 8 位。
    pub side_id: String,
    /// 侧边栏「工作目录」那一段的标题。
    pub side_cwd: String,
    /// 侧边栏「上下文」那一段的标题。
    pub side_context: String,
    /// 上下文用了多少，`{used}` `{window}` `{percent}`。
    pub side_context_value: String,
    /// 侧边栏「用量」那一段的标题。
    pub side_usage: String,
    /// 一共多少，`{tokens}`。
    pub side_total: String,
    /// 输入、输出各多少，`{input}` `{output}`。
    pub side_split: String,
    /// 缓存命中率，`{percent}`。
    pub side_hit: String,
    /// 后台命令、子代理、待办的字。
    pub jobs: JobTexts,
    /// 空会话的首页上，输入框空着时固定写的提示（蓝图「输入框」第 9 条）。
    pub home_placeholder: String,
    /// 权限级别的叫法：`workspace`、`full`、`read_only`（`kernel/events-bodies.md` 的 `permission`）。
    pub levels: HashMap<Level, String>,
    /// 一轮做完的收尾行：`{time}` 做完的时刻，`{endpoint}` 端点，`{model}` 模型名，`{elapsed}` 用时（和「已思考」一个写法）。
    pub done: String,
    /// 输出的速度，`{rate}` 每秒几个 token。
    pub speed: String,
    /// 复制成功，`{count}` 是字数。
    pub copied: String,
    /// 复制失败，`{reason}` 是原因。
    pub copy_failed: String,
    /// 还在连核心。
    pub connecting: String,
    /// 核心没在跑，也没说怎么拉起来。
    pub no_core_bin: String,
    /// 连不上核心，`{reason}` 是原因。
    pub core_failed: String,
    /// 核心断开了。
    pub disconnected: String,
    /// 一条请求被拒，`{reason}` 是核心说的原因。
    pub refused: String,
    /// 这一轮被打断了。
    pub interrupted: String,
    /// 这一轮出错了，`{class}` 分类，`{message}` 原话。
    pub failed: String,
    /// 正在重试。
    pub retry: String,
    /// 上下文用量，紧凑写法照旧版：`{used}` 用了多少，`{window}` 窗口多大，`{percent}` 一位小数的百分比。
    pub context: String,
    /// 空着按 `Ctrl+C` 的提示。
    pub exit_hint: String,
    /// 在回答时按第一下 `Esc` 的提示。
    pub esc_hint: String,
    /// 没在回答、输入框有字时按第一下 `Esc` 的提示。
    pub esc_clear_hint: String,
    /// 回答进行中按 `Ctrl+L` 的提示。
    pub no_clear_running: String,
    /// 暂存着东西时，输入框第一行最右边的标记。
    pub stashed: String,
    /// 收尾行后面的本轮用量，`{tokens}` 输入加输出，`{percent}` 命中率。
    pub done_usage: String,
    /// 请求被拒时，认得的原因码（`data.reason`）写的短话。
    pub refusals: HashMap<String, String>,
    /// 输入历史列表第一行的标签。
    pub history_search: String,
    /// 输入历史列表里一条都对不上时那一行。
    pub history_empty: String,
    /// 输入历史列表里展开的一条太长时，最后一行，`{count}` 还有几行。
    pub history_more: String,
    /// 还没发过话时按 Ctrl+R 的提示。
    pub no_history: String,
    /// 图还在做时那一行占位。
    pub figure_pending: String,
    /// mermaid 图下面那一行，点了开大图。
    pub figure_zoom: String,
    /// 换了主题，`{name}` 是名字。
    pub theme_changed: String,
    /// 撤销那一行，`{turns}` 几轮。
    pub undone: String,
    /// 改回了几个文件，`{count}`。
    pub restored: String,
    /// 几个文件没动，`{count}`。
    pub untouched: String,
    /// 几条命令的改动撤不回，`{count}`。
    pub commands: String,
    /// 没有这个斜杠命令，`{name}`。
    pub unknown_command: String,
    /// 演示用的假命令，`{name}`。
    pub fake_command: String,
    /// 思考进行中那一行的字。
    pub thinking: String,
    /// 想完以后那一行的字。
    pub thought: String,
    /// 她还在写参数时那一行，`{name}` 是工具的显示名。
    pub prepare: String,
    /// 预览放不下时最后一行，`{count}` 是省略了几行。
    pub omitted: String,
    /// 时间线收起那一行的字：永远是英文，和中文正文分开（`13-终端界面.md` 第三节第 4 条）。
    pub summary: Summary,
    /// 累计用量和缓存命中率，紧凑写法照旧版：`{tokens}` 写短的 token 数，`{percent}` 命中率的整数。
    pub total: String,
}

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
    /// 思考、命令的预览最多几行。
    pub preview_rows: usize,
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

/// 模型的数据。核心的协议里还没有模型的窗口多大，先记在这里（等配置系统做出来，由核心推给头）。
#[derive(Debug, Clone, Deserialize)]
pub struct Models {
    /// 模型名到上下文窗口的 token 数。
    pub context_windows: HashMap<String, u64>,
}

/// 全部配置。
#[derive(Debug, Clone)]
pub struct Config {
    /// 布局的数值。
    pub layout: Layout,
    /// 界面上的字。
    pub text: Texts,
    /// 模型的数据。
    pub models: Models,
    /// 斜杠命令。
    pub commands: Commands,
    /// 时间线的样子。
    pub timeline: Timeline,
    /// 代码着色：每种语言的关键字、注释记号。
    pub languages: Languages,
    /// 公式转 Unicode 的对照表。
    pub math: Math,
    /// 正文里的图。
    pub figures: FigureLook,
    /// 运行状态行的词库。
    pub pulse: Words,
    /// 首页的吉祥物。
    pub mascot: Look,
    /// 演示用的假数据源的脚本（后台命令、子代理、待办）。
    pub fake: Script,
    /// 出厂的主题：名字和颜色，照登记的先后。
    pub themes: Vec<(String, Palette)>,
}

/// 正文里的图（`resources/figures.json`，蓝图「图片、公式和 mermaid 图」）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FigureLook {
    /// 一张图最多占几行：只防病态，不为塞进一屏。
    pub max_rows: u16,
    /// mermaid 图里的字体，照先后找；都没有的由 resvg 从系统里找。
    pub fonts: Vec<String>,
    /// 块级公式的字号是一格高的几倍。
    pub math_scale: f32,
    /// 最多记着几张做好的图。
    pub keep: usize,
    /// 点开看的 mermaid 大图，缓存目录里最多留几张。
    pub zoom_keep: usize,
}

impl Config {
    /// 读编译时带进来的那一份。
    ///
    /// # Errors
    ///
    /// JSON 写坏了、缺了字段时返回错误，说清是哪一份。
    pub fn builtin() -> Result<Self, String> {
        Ok(Self {
            layout: parse("layout.json", include_str!("../resources/layout.json"))?,
            text: parse("text/zh.json", include_str!("../resources/text/zh.json"))?,
            models: parse("models.json", include_str!("../resources/models.json"))?,
            commands: parse("commands.json", include_str!("../resources/commands.json"))?,
            timeline: parse("timeline.json", include_str!("../resources/timeline.json"))?,
            languages: parse("code.json", include_str!("../resources/code.json"))?,
            math: parse("math.json", include_str!("../resources/math.json"))?,
            figures: parse("figures.json", include_str!("../resources/figures.json"))?,
            pulse: parse("pulse.json", include_str!("../resources/pulse.json"))?,
            mascot: parse("mascot.json", include_str!("../resources/mascot.json"))?,
            fake: parse("fake.json", include_str!("../resources/fake.json"))?,
            themes: crate::theme::builtin()?,
        })
    }
}

fn parse<T: for<'de> Deserialize<'de>>(name: &str, json: &str) -> Result<T, String> {
    serde_json::from_str(json).map_err(|e| format!("resources/{name} 读不懂：{e}"))
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn builtin_resources_parse() {
        let config = Config::builtin().unwrap();
        assert!(config.layout.pad_right >= 1, "行尾的光标要有地方待");
        assert!(config.layout.max_rows >= 1);
        assert!((1..=100).contains(&config.layout.width_percent));
        // 每一档都有图标；只读是暂停符号（蓝图「权限级别」第 3 条）。
        let icons = &config.layout.level_icons;
        assert!(
            config
                .layout
                .level_cycle
                .iter()
                .all(|l| icons.contains_key(l))
        );
        assert_eq!(
            icons.get(&crate::core::Level::ReadOnly).map(String::as_str),
            Some("⏸ ")
        );
    }
}
