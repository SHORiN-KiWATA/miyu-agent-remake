//! 界面上的字、布局的数值、模型的窗口：住在 `resources/` 的 JSON 里，编译时带进来。
//!
//! 仓库的规矩是「无硬编码」（`AGENTS.md`「代码的规矩」）。界面上的字照仓库
//! `resources/core/human/` 的做法，一种语言一份（蓝图「界面语言」：`languages.json` 那张表里有几种就有几份）。

use std::collections::HashMap;

use serde::Deserialize;

use crate::commands::Commands;
use crate::core::Level;
use crate::jobs::Script;
use crate::language::{Language, LanguageTable};
use crate::markdown::{Languages, Math};
use crate::mascot::Look;
use crate::pulse::Words;
use crate::theme::Palette;
use localize::localized;

mod attach;
mod figures;
mod icons;
mod layout;
mod localize;
mod mention;
mod motion;
mod notes;
mod notify;
mod open;
mod panels;
mod timeline;
mod words;

pub use attach::{AttachLook, AttachTexts};
pub use figures::{FigureLook, Room};
pub use icons::Icons;
pub use layout::{Bar, Dots, Layout, Shimmer, TodoMarks};
pub use mention::{MentionLook, MentionTexts};
pub use motion::CompactionMotion;
pub use notes::CompactionTexts;
pub use notify::{NotifyLook, NotifyTexts};
pub use open::OpenTexts;
pub use panels::{HistoryTexts, MenuTexts};
pub use timeline::{Summary, Timeline, ToolKind};
pub use words::{JobTexts, ModelTexts, RecapTexts, RenameTexts, UndoFileTexts};

/// 界面上给人看的字。`{count}` 这样的占位由代码填。
#[derive(Debug, Clone, Deserialize)]
pub struct Texts {
    /// 输入框空着时轮换的提示（蓝图「输入框」第 9 条）。
    pub tips: Vec<String>,
    /// 回答里 Markdown 要写的几个字（蓝图「她的回答：Markdown」第 12、15 条）。
    pub markdown: crate::markdown::Labels,
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
    /// 压缩过几次，`{n}`；没压过不写。
    pub side_compactions: String,
    /// 意外断过几次缓存，`{n}`；没断过不写。
    pub side_breaks: String,
    /// 后台命令、子代理、待办的字。
    pub jobs: JobTexts,
    /// 帮助框里的字（`/help`）。
    pub help: crate::ui::help::Texts,
    /// 配置页的字（`/config`，蓝图「配置页」）。
    pub settings: crate::settings::Texts,
    /// 确认和提问的抽屉上的字。
    pub drawer: crate::drawer::Texts,
    /// 输入框里粘贴块上写的，`{lines}` 行数。
    pub paste_label: String,
    /// 附件、文件块上写的（蓝图「输入框」第 12 条）。
    pub attach: AttachTexts,
    /// `@` 文件列表上写的。
    pub mention: MentionTexts,
    /// `Ctrl+V` 读不到剪贴板时的提示。
    pub clipboard_unreadable: String,
    /// `Ctrl+V` 读到空剪贴板时的提示。
    pub clipboard_empty: String,
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
    /// `/copy` 还没有能复制的回答（蓝图「斜杠命令」`/copy`）。
    pub nothing_to_copy: String,
    /// 还在连核心。
    pub connecting: String,
    /// 核心没在跑，也没说怎么拉起来。
    pub no_core_bin: String,
    /// 连不上核心，`{reason}` 是原因。
    pub core_failed: String,
    /// 核心断开了，正在重新连接（蓝图「连核心」第 7 条）。
    pub reconnecting: String,
    /// 核心断开时在进行的那一轮收尾那一行。
    pub turn_cut: String,
    /// `MIYU_CORE_BIN` 指的程序不存在，`{path}` 是路径（第 8 条）。
    pub missing_core: String,
    /// 连不上核心时按 `Enter`：发不出去，字留在输入框里。
    pub not_connected: String,
    /// 崩了以后终端里说调用栈记在哪，`{path}` 是文件（蓝图「崩了」）。
    pub crash_saved: String,
    /// 系统通知上的字（蓝图「系统通知」第 3 条）。
    pub notify: NotifyTexts,
    /// 一条请求被拒，`{reason}` 是核心说的原因。
    pub refused: String,
    /// 这一轮被打断了。
    pub interrupted: String,
    /// 这一轮出错了，`{reason}` 是原因（`transcript/failure.rs` 拼的）。
    pub failed: String,
    /// 原因里人话和原话连起来：`{head}` 人话，`{message}` 原话。
    pub reason_with: String,
    /// 这几种 HTTP 状态码，原话前面加的人话（蓝图「正文」第 4 条）。
    pub status_hints: HashMap<String, String>,
    /// 正在重试。
    pub retry: String,
    /// 上下文用量，紧凑写法照旧版：`{used}` 用了多少，`{window}` 窗口多大，`{percent}` 一位小数的百分比。
    pub context: String,
    /// 空着按 `Ctrl+C` 的提示。
    pub exit_hint: String,
    /// `Ctrl+C` 清空了输入框的提示：清掉的按 `↑` 找回。
    pub input_cleared: String,
    /// 编辑上一句时输入框上边框写的（「输入框」第 13 条）。
    pub editing: String,
    /// 开不了链接、文件时的提示。
    pub open: OpenTexts,
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
    /// 请求被拒时，认得的原因码（`data.reason`）写的短话：只弹提示框，不写进正文（蓝图「正文」第 6、9 条；
    /// 2026-10-01 项目主人：命令没生效、什么都没变的，弹通知就行）。认不得的才照核心的原话写进正文。
    pub refusal_hints: HashMap<String, String>,
    /// 回顾（`/recap`，蓝图「回顾」）。
    pub recap: RecapTexts,
    /// 换模型、冷却的字（「配置与模型」第 2、7 条）。
    pub models: ModelTexts,
    /// 改名的提示。
    pub rename: RenameTexts,
    /// 输入历史列表上的字（蓝图「输入历史列表」）。
    pub history: HistoryTexts,
    /// 斜杠命令列表上的字（蓝图「斜杠命令列表」）。
    pub menu: MenuTexts,
    /// 压缩那几行（蓝图「正文」第 9 条）。
    pub compaction: CompactionTexts,
    /// 出错的分类写成人话：内核自己查出来的、没有原话的用（蓝图「正文」第 4 条）；认不得的照原样。
    pub error_classes: HashMap<String, String>,
    /// 图还在做时那一行占位。
    pub figure_pending: String,
    /// mermaid 图下面那一行，点了开大图。
    pub figure_zoom: String,
    /// 换了主题，`{name}` 是名字。
    pub theme_changed: String,
    /// 换了图标，`{name}` 是那一套的名字。
    pub icons_changed: String,
    /// 撤销那一行打头的：`已撤销`（不写几轮：撤销只能一轮一轮撤）。
    pub undone: String,
    /// 撤销那一行接着写的：`/restore 恢复`。
    pub undo_restore: String,
    /// 撤掉的几轮里有压缩时，撤销那一行下面那一句（施工 6-9）。
    pub undo_compactions: String,
    /// 撤掉的几轮里有清空：撤销那一行下面说一句（`/clear`，照 `miyu undo`）。
    pub undo_clears: String,
    /// 改回了几个文件，`{count}`。
    pub restored: String,
    /// 撤销点开以后，改回的每个文件后面写的（2026-10-02 项目主人要文件清单）。
    pub undo_files: UndoFileTexts,
    /// 撤销点开以后：停掉了几个后台任务，`{count}`（施工 7-8）。
    pub stopped_jobs: String,
    /// `/language` 换了语言以后提示的一句（蓝图「界面语言」）。
    pub language_switched: String,
    /// `/language` 开的框里的字。
    pub languages: crate::ui::languages::Texts,
    /// `/sessions` 开的框里的字（蓝图「会话列表」）。
    pub sessions: crate::ui::session_list::Texts,
    /// `/model` 开的框里的字（「配置与模型」第 1 条）。
    pub model_panel: crate::ui::model_list::Texts,
    /// `/effort` 的框（`ui/effort_list.rs`）。
    pub effort: crate::ui::effort_list::Texts,
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
    /// 一步对着别的会话（参数里带着会话编号），`{id}` 是短编号。
    pub on_session: String,
    /// 预览放不下时最后一行，`{count}` 是省略了几行。
    pub omitted: String,
    /// 时间线收起那一行的字：界面语言是自动时换成语言表 `auto_summary` 那一种（英文），手动选了哪种照哪种（蓝图「时间线」第 17 条）。
    pub summary: Summary,
    /// 累计用量和缓存命中率，紧凑写法照旧版：`{tokens}` 写短的 token 数，`{percent}` 命中率的整数。
    pub total: String,
}

/// 全部配置。
#[derive(Debug, Clone)]
pub struct Config {
    /// 布局的数值。
    pub layout: Layout,
    /// 界面上的字。
    pub text: Texts,
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
    /// 系统通知怎么弹、怎么响（`resources/notify.json`）。
    pub notify: NotifyLook,
    /// 附件认哪几种（`resources/attachments.json`）。
    pub attachments: AttachLook,
    /// `@` 文件列表的数值（`resources/mention.json`）。
    pub mention: MentionLook,
    /// 运行状态行的词库。
    pub pulse: Words,
    /// 首页的吉祥物。
    pub mascot: Look,
    /// 演示用的假数据源的脚本（后台命令、子代理、待办）。
    pub fake: Script,
    /// 出厂的主题：名字和颜色，照登记的先后。
    pub themes: Vec<(String, Palette)>,
    /// 当前这一套图标（`/icons` 换的是它）。
    pub icons: Icons,
    /// 出厂的几套图标，照登记的先后。
    pub icon_sets: Vec<Icons>,
    /// 界面语言有哪几种（`/language` 的框照它列）。
    pub language_table: LanguageTable,
    /// 这一份照哪种语言读的。
    pub language: Language,
    /// 界面语言是自动（跟系统）的，不是手动选的：收起那一行照 `language_table.auto_summary` 那一种（蓝图「界面语言」）。
    pub auto: bool,
}

impl Config {
    /// 读编译时带进来的那一份，中文界面。
    ///
    /// # Errors
    ///
    /// JSON 写坏了、缺了字段时返回错误，说清是哪一份。
    #[cfg(test)]
    pub fn builtin() -> Result<Self, String> {
        let table = LanguageTable::builtin()?;
        Self::load(&table.find("zh").ok_or("languages.json 里没有中文")?, true)
    }

    /// 读编译时带进来的那一份，照 `language` 挑界面上的字、命令的说明、运行状态行的词（蓝图「界面语言」）。
    ///
    /// # Errors
    ///
    /// JSON 写坏了、缺了字段时返回错误，说清是哪一份。
    pub fn load(language: &Language, auto: bool) -> Result<Self, String> {
        let table = LanguageTable::builtin()?;
        let (text_name, text_json) = localize::text(language)?;
        let mut text: Texts = parse(&text_name, text_json)?;
        // 自动时收起那一行照语言表定的那一种（英文）；手动选了哪种照哪种（蓝图「时间线」第 17 条）。
        if auto && let Some(summary_language) = table.find(&table.auto_summary) {
            text.summary = localize::summary(&summary_language)?;
        }
        let layout: Layout = parse("layout.json", include_str!("../../resources/layout.json"))?;
        let icon_sets = icons::builtin()?;
        let icons = icons::pick(&icon_sets, &layout.icons).ok_or("resources/icons/ 一套都没有")?;
        Ok(Self {
            layout,
            text,
            commands: localized(
                "commands.json",
                include_str!("../../resources/commands.json"),
                language,
                &table,
            )?,
            timeline: parse(
                "timeline.json",
                include_str!("../../resources/timeline.json"),
            )?,
            languages: parse("code.json", include_str!("../../resources/code.json"))?,
            math: parse("math.json", include_str!("../../resources/math.json"))?,
            figures: parse("figures.json", include_str!("../../resources/figures.json"))?,
            notify: parse("notify.json", include_str!("../../resources/notify.json"))?,
            mention: parse("mention.json", include_str!("../../resources/mention.json"))?,
            attachments: parse(
                "attachments.json",
                include_str!("../../resources/attachments.json"),
            )?,
            pulse: localized(
                "pulse.json",
                include_str!("../../resources/pulse.json"),
                language,
                &table,
            )?,
            mascot: parse("mascot.json", include_str!("../../resources/mascot.json"))?,
            fake: parse("fake.json", include_str!("../../resources/fake.json"))?,
            themes: crate::theme::builtin()?,
            icons,
            icon_sets,
            language_table: table,
            language: language.clone(),
            auto,
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

    #[test]
    fn nothing_to_compact_is_a_short_hint_not_a_line() {
        // 2026-09-30 项目主人：没必要在正文里打一行，给个简短的通知就行。
        let text = Config::builtin().unwrap().text;
        assert_eq!(
            text.refusal_hints
                .get("nothing_to_compact")
                .map(String::as_str),
            Some("上下文过少")
        );
    }

    #[test]
    fn only_the_new_tool_names_are_known() {
        // 2026-10-01 核心改名 agent → subagent、message_agent → send_message，同一天项目主人定不认旧名、不留兼容。
        let config = Config::builtin().unwrap();
        let kinds = &config.timeline.kinds;
        assert!(kinds.get("subagent").is_some() && kinds.get("send_message").is_some());
        assert!(kinds.get("agent").is_none() && kinds.get("message_agent").is_none());
        assert_eq!(
            config.icons.tool("agent"),
            config.icons.tool("没登记的工具")
        );
        assert_ne!(
            config.icons.tool("subagent"),
            config.icons.tool("没登记的工具")
        );
    }
}
