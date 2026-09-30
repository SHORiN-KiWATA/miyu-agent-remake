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

mod attach;
mod figures;
mod icons;
mod layout;
mod mention;
mod motion;
mod notes;
mod notify;
mod open;
mod panels;
mod timeline;

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
    /// 命令被信号杀掉的状态：`{signal}`。
    pub killed: String,
    /// 撤销时停掉的状态。
    pub undone: String,
    /// 核心重启时停掉的状态。
    pub restarted: String,
    /// 展开一条命令、点开结束的那一行：它一个字都没输出。
    pub no_output: String,
    /// 后台面板里结束了的多于一个时收起来的那一行：`{count}` 收了几条。
    pub more_ended: String,
    /// 展开以后最后那一行：收起。
    pub less_ended: String,
    /// 命令被信号杀掉的通知：`{title}` `{signal}`。
    pub killed_note: String,
    /// 子代理被停掉的通知：`{title}`，后面接 `{why}`。
    pub agent_stopped_note: String,
    /// 通知后面接的为什么停：撤销时停的。
    pub why_undone: String,
    /// 通知后面接的为什么停：核心重启时停的。
    pub why_restarted: String,
    /// 子代理在想。
    pub doing_thinking: String,
    /// 子代理在说。
    pub doing_replying: String,
    /// 别处来的话那一行写的来处（蓝图「别处来的话」第 2 条）：同一个人在别处（网页、命令行）说的。
    pub from_person: String,
    /// 来处：主会话（在子会话里看，交代的活、留言）。
    pub from_main: String,
    /// 来处：这个会话派的子代理，`{job}` 任务编号。
    pub from_agent: String,
    /// 来处：别的 harness，`{name}` 它报的名字（洗过）。
    pub from_harness: String,
    /// 收着时那一行：`{from}` 来处，`{text}` 话的预览。
    pub received: String,
    /// 切进子会话时输入框上边框右边写的：`{title}` 子代理的名字。
    pub child_tag: String,
    /// 子代理报完了、还在看它时，状态行里写的「完成」（用时写在右边，不重复）。
    pub finished: String,
    /// 待办的进度，`{done}` `{total}`。
    pub todo: String,
    /// 待办长了：做完的收成一行，`{count}` 项。
    pub todo_folded: String,
    /// 待办长了：放不下的收成一行，`{count}` 项。
    pub todo_more: String,
}

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
    /// 请求被拒时，认得的原因码（`data.reason`）写的短话。
    pub refusals: HashMap<String, String>,
    /// 这几种拒绝不写进正文，只弹提示框（`nothing_to_compact` 的「上下文过少」，蓝图「正文」第 9 条）。
    pub refusal_hints: HashMap<String, String>,
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
    /// 撤销那一行：`已撤销 · /restore 恢复`（不写几轮：撤销只能一轮一轮撤）。
    pub undone: String,
    /// 撤掉的几轮里有压缩时，撤销那一行下面那一句（施工 6-9）。
    pub undo_compactions: String,
    /// 撤掉的几轮里有清空：撤销那一行下面说一句（`/clear`，照 `miyu undo`）。
    pub undo_clears: String,
    /// 改回了几个文件，`{count}`。
    pub restored: String,
    /// 几个文件没动，`{count}`。
    pub untouched: String,
    /// 几条命令的改动撤不回，`{count}`。
    pub commands: String,
    /// 撤销点开以后：停掉了几个后台任务，`{count}`（施工 7-8）。
    pub stopped_jobs: String,
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
}

impl Config {
    /// 读编译时带进来的那一份。
    ///
    /// # Errors
    ///
    /// JSON 写坏了、缺了字段时返回错误，说清是哪一份。
    pub fn builtin() -> Result<Self, String> {
        let layout: Layout = parse("layout.json", include_str!("../../resources/layout.json"))?;
        let icon_sets = icons::builtin()?;
        let icons = icons::pick(&icon_sets, &layout.icons).ok_or("resources/icons/ 一套都没有")?;
        Ok(Self {
            layout,
            text: parse("text/zh.json", include_str!("../../resources/text/zh.json"))?,
            commands: parse(
                "commands.json",
                include_str!("../../resources/commands.json"),
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
            pulse: parse("pulse.json", include_str!("../../resources/pulse.json"))?,
            mascot: parse("mascot.json", include_str!("../../resources/mascot.json"))?,
            fake: parse("fake.json", include_str!("../../resources/fake.json"))?,
            themes: crate::theme::builtin()?,
            icons,
            icon_sets,
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
        assert!(!text.refusals.contains_key("nothing_to_compact"));
    }
}
