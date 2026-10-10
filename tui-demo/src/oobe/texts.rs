//! 引导上的字（`text/<语言>.json` 的 `oobe`，蓝图 `tui.md`「第一次打开的引导」）：和网页的引导用一样的说法
//! （2026-10-08 和网页的会话对过）。`{name}` 这样的占位由代码填。

use std::collections::HashMap;

use serde::Deserialize;

/// 欢迎页。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Welcome {
    /// 标题，一个字一个字打出来。
    pub title: String,
    /// 下面一行暗色的说明。
    pub sub: String,
    /// 「回车开始」。
    pub start: String,
}

/// 最下面一行暗色的按键提示。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keys {
    /// 选一行的几屏。
    pub pick: String,
    /// 填密钥、自定义。
    pub form: String,
    /// 光标在自定义的接口那一格。
    pub driver: String,
    /// 光标在「测试连接」那一行。
    pub test: String,
    /// 正在编辑一格。
    pub editing: String,
    /// 正在搜模型。
    pub searching: String,
    /// 选模型。
    pub models: String,
    /// 建人格，光标在名字那一格。
    pub persona: String,
    /// 光标在人格提示词、示范对话、人设提醒短语那几格：回车开浮窗。
    pub edit: String,
    /// 大编辑浮窗。
    pub writing: String,
    /// 自定义的浮窗，光标在功能、工具上。
    pub custom: String,
    /// 自定义的浮窗，光标在名称上。
    pub custom_name: String,
    /// 自定义的浮窗，光标在「创建 →」上。
    pub create: String,
    /// 看一个预设的浮窗。
    pub view: String,
    /// 光标停着、没在编辑的格子右端写的。
    pub enter_edit: String,
    /// 选预设（自定义以外的几块）。
    pub presets: String,
    /// 「更多供应商」浮窗。
    pub more: String,
}

/// 选一行的一步：标题、说明。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Heading {
    /// 标题。
    pub title: String,
    /// 说明。
    pub sub: String,
}

/// 语言那一步。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Language {
    /// 标题、说明。
    #[serde(flatten)]
    pub head: Heading,
    /// 第一行：跟随系统，`{name}` 是照系统认出来的那种。
    pub auto: String,
}

/// 图标那一步。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Icons {
    /// 标题、说明。
    #[serde(flatten)]
    pub head: Heading,
    /// 看得到图标。
    pub yes: String,
    /// 是方块或者乱码。
    pub no: String,
}

/// 接模型那一步。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    /// 选一家：标题、说明。
    #[serde(flatten)]
    pub head: Heading,
    /// 读回来以前。
    pub loading: String,
    /// 找到了 key 的标签。
    pub found: String,
    /// 配好了的标签。
    pub configured: String,
    /// 本机那一段的段名。
    pub local: String,
    /// 其他那一段的段名。
    pub other: String,
    /// 「更多供应商…」：开浮窗搜全目录（第 16a 条）。
    pub more: String,
    /// 「更多供应商」浮窗的标题。
    pub more_title: String,
    /// 浮窗里搜索那一格空着时的说明。
    pub more_search: String,
    /// 浮窗里一家都对不上。
    pub no_match: String,
    /// 搜模型那一格没在搜时右端写的。
    pub search_cue: String,
    /// 「自定义…」。
    pub custom: String,
    /// 「自定义…」后面暗色写的。
    pub custom_note: String,
    /// 已经有聊天模型时的标题。
    pub ready_title: String,
    /// 「已配好：{name}」。
    pub ready: String,
    /// 「用它」。
    pub keep: String,
    /// 「换一个」。
    pub change: String,
    /// 「密钥」。
    pub key: String,
    /// 「来自环境变量 {env}」。
    pub key_env: String,
    /// 本机的：「不用密钥」。
    pub key_none: String,
    /// 空着按回车。
    pub key_needed: String,
    /// 「会真发一句话，花一点额度」。
    pub cost: String,
    /// 自定义：标题、说明。
    pub custom_head: Heading,
    /// 「地址」。
    pub url: String,
    /// 地址空着时暗色写的例子。
    pub url_example: String,
    /// 地址不是 http、https 开头。
    pub url_bad: String,
    /// 「接口」。
    pub driver: String,
    /// 三种接口的名字，照 `openai-chat`、`anthropic`、`openai-responses` 的先后。
    pub drivers: Vec<String>,
    /// 「正在连接…」。
    pub testing: String,
    /// 「连上了，找到 {count} 个模型」。
    pub connected: String,
    /// 选模型那一屏的说明。
    pub pick_sub: String,
    /// 「未获取到模型列表，填上模型名再测一次」。
    pub no_list: String,
    /// 「模型名」。
    pub model_name: String,
    /// 模型名空着时暗色写的例子。
    pub model_example: String,
    /// 「不通（{stage}）：{message}」。
    pub failed: String,
    /// 哪一步：`list`、`request`、`config`。
    pub stages: HashMap<String, String>,
    /// 「搜索模型」。
    pub search: String,
    /// 最后一行「测试连接」。
    pub test: String,
    /// 模型多、开了窗：上面还有几个，`{n}`。
    pub more_above: String,
    /// 下面还有几个，`{n}`。
    pub more_below: String,
    /// 「正在保存…」。
    pub saving: String,
}

/// 建人格那一步。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Persona {
    /// 标题、说明。
    #[serde(flatten)]
    pub head: Heading,
    /// 四格的名字：名字、人格提示词、示范对话、人设提醒短语。
    pub fields: Vec<String>,
    /// 四格空着时暗色写的。
    pub hints: Vec<String>,
    /// 示范对话有几轮，`{n}`。
    pub examples_some: String,
    /// 「下一步」。
    pub next: String,
    /// 名字空着。
    pub name_needed: String,
    /// 已经有默认人格的。
    pub existing: String,
    /// 「正在保存…」。
    pub saving: String,
    /// 头像那一格的名字。
    pub avatar: String,
    /// 头像空着时暗色写的。
    pub avatar_hint: String,
    /// 已有的人格有头像、格子空着时写的。
    pub avatar_set: String,
}

/// 选预设那一步。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    /// 标题、说明。
    #[serde(flatten)]
    pub head: Heading,
    /// 「自定义」。
    pub custom: String,
    /// 「自定义」下面暗色写的。
    pub custom_note: String,
    /// 自定义展开以后「名字」那一格。
    pub name: String,
    /// 名字空着时暗色写的。
    pub name_hint: String,
    /// 名字空着。
    pub name_needed: String,
    /// 读回来以前。
    pub loading: String,
    /// 自定义最后一行「创建」。
    pub create: String,
}

/// 好了那一屏。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Done {
    /// 「好了」。
    pub title: String,
    /// 「模型 {model} · 人格 {persona} · 预设 {preset}」。
    pub summary: String,
    /// 「回车开始聊天」。
    pub start: String,
}

/// 整份。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texts {
    /// 欢迎页。
    pub welcome: Welcome,
    /// 顶上进度那一行五步的名字。
    pub steps: Vec<String>,
    /// 按键提示。
    pub keys: Keys,
    /// 语言。
    pub language: Language,
    /// 图标。
    pub icons: Icons,
    /// 接模型。
    pub model: Model,
    /// 建人格。
    pub persona: Persona,
    /// 选预设。
    pub preset: Preset,
    /// 好了。
    pub done: Done,
    /// 没连上核心。
    pub offline: String,
    /// 标记写不成，`{message}`。
    pub unmarked: String,
}
