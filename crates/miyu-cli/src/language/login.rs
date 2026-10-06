//! `miyu login`、`miyu logout` 给人看的字（施工 8-5，`docs/blueprint/config.md`「给人看的字」的「命令行」那张表、「样子」；
//! 图纸没写到的几句 2026-10-01 主会话照施工员的推荐定）。key 从不出现在这里的任何一句里。

use super::Language;

impl Language {
    /// 交互着选的那张表的头一行：`login` 列出配置里用到的、设过的，`logout` 只列设过的。
    pub(crate) fn keys_heading(&self, logout: bool) -> &'static str {
        match (self, logout) {
            (Language::Chinese, false) => "配置里用到的密钥：",
            (Language::English, false) => "Keys the config uses:",
            (Language::Chinese, true) => "设过的 key：",
            (Language::English, true) => "Keys that are set:",
        }
    }

    /// 问选哪一个：`login` 还能敲一个新名字。
    pub(crate) fn pick_key(&self, logout: bool) -> &'static str {
        match (self, logout) {
            (Language::Chinese, false) => "选一个编号，或者敲一个新名字：",
            (Language::English, false) => "Pick a number, or type a new name: ",
            (Language::Chinese, true) => "选一个编号：",
            (Language::English, true) => "Pick a number: ",
        }
    }

    /// 直接回车、读到头：没选。
    pub(crate) fn not_picked(&self) -> &'static str {
        match self {
            Language::Chinese => "没选",
            Language::English => "Nothing picked",
        }
    }

    /// `login` 不写名字，一个能选的都没有。
    pub(crate) fn no_keys_used(&self) -> &'static str {
        match self {
            Language::Chinese => "配置里还没有用到密钥的供应商：写 miyu login <名字>",
            Language::English => "No provider in the config uses a key yet: run miyu login <name>",
        }
    }

    /// 名字不合写法（写在命令行上的、敲的）。
    pub(crate) fn bad_key_name(&self, name: &str) -> String {
        match self {
            Language::Chinese => format!(
                "{name} 不能当 key 的名字：小写字母开头，只有小写字母、数字、-、_，最长 64 个字符"
            ),
            Language::English => format!(
                "{name} cannot name a key: start with a lowercase letter and use only lowercase letters, digits, - and _, up to 64 characters"
            ),
        }
    }

    /// 不写名字又不在终端里：`command` 是 `login` 或 `logout`。
    pub(crate) fn pick_needs_terminal(&self, command: &str) -> String {
        match self {
            Language::Chinese => format!("要在终端里选，或者写 miyu {command} <名字>"),
            Language::English => format!("Pick in a terminal, or run miyu {command} <name>"),
        }
    }

    /// 问 key：不显示。
    pub(crate) fn paste_key(&self, name: &str) -> String {
        match self {
            Language::Chinese => format!("粘贴 {name} 的 key（不显示）："),
            Language::English => format!("Paste the key for {name} (it will not show): "),
        }
    }

    /// 读到的 key 去掉前后空白是空的。
    pub(crate) fn no_key_given(&self) -> &'static str {
        match self {
            Language::Chinese => "没收到 key",
            Language::English => "No key was given",
        }
    }

    /// 贴 key 时取消了：按了 `Ctrl+C`，或者空行按了 `Ctrl+D`（施工 8-5 补）。
    pub(crate) fn key_paste_cancelled(&self) -> &'static str {
        match self {
            Language::Chinese => "没存，取消了",
            Language::English => "Not saved, cancelled",
        }
    }

    /// 存好了：`replaced` 的是换掉了原来的。
    pub(crate) fn key_saved(&self, name: &str, replaced: bool) -> String {
        match (self, replaced) {
            (Language::Chinese, false) => format!("· {name} 的 key 存好了"),
            (Language::Chinese, true) => format!("· 换掉了 {name} 的 key"),
            (Language::English, false) => format!("· Saved the key for {name}"),
            (Language::English, true) => format!("· Replaced the key for {name}"),
        }
    }

    /// 设没设。
    pub(crate) fn key_state(&self, set: bool) -> &'static str {
        match (self, set) {
            (Language::Chinese, true) => "已设置",
            (Language::Chinese, false) => "未设置",
            (Language::English, true) => "set",
            (Language::English, false) => "not set",
        }
    }

    /// 谁在用：几项连在一起。
    pub(crate) fn users(&self, used_by: &[&str]) -> String {
        let between = match self {
            Language::Chinese => "、",
            Language::English => ", ",
        };
        used_by.join(between)
    }

    /// 一个都没设过。
    pub(crate) fn no_keys_yet(&self) -> &'static str {
        match self {
            Language::Chinese => "还没有设过 key",
            Language::English => "No keys yet",
        }
    }

    /// 删掉了。
    pub(crate) fn key_deleted(&self, name: &str) -> String {
        match self {
            Language::Chinese => format!("· 删掉了 {name} 的 key"),
            Language::English => format!("· Deleted the key for {name}"),
        }
    }

    /// 要删的没设过。
    pub(crate) fn no_such_key(&self, name: &str) -> String {
        match self {
            Language::Chinese => format!("{name} 没有设过 key"),
            Language::English => format!("{name} has no key"),
        }
    }
}
