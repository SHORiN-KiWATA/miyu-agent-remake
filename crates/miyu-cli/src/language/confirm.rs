//! `miyu pkg install`、`remove` 做之前印的那一份、问的那一句（施工 F-8 下补，`docs/blueprint/cli/pkg.md`「给人看的字」）。

use super::Language;

impl Language {
    /// 第一行：将安装、将装回、将卸载哪个包（带版本的接版本）。
    pub(crate) fn will(&self, doing: Doing, id: &str) -> String {
        match (self, doing) {
            (Language::Chinese, Doing::Install) => format!("将安装 {id}"),
            (Language::Chinese, Doing::Restore) => format!("将装回 {id}"),
            (Language::Chinese, Doing::Remove) => format!("将卸载 {id}"),
            (Language::English, Doing::Install) => format!("Install {id}"),
            (Language::English, Doing::Restore) => format!("Restore {id}"),
            (Language::English, Doing::Remove) => format!("Remove {id}"),
        }
    }

    /// 升级时接在第一行后面：换下的那一份的版本（没写版本的没有）。
    pub(crate) fn replacing(&self, version: Option<&str>) -> String {
        match (self, version) {
            (Language::Chinese, Some(version)) => format!("（替换 {version}）"),
            (Language::Chinese, None) => "（替换已装的）".to_string(),
            (Language::English, Some(version)) => format!(" (replaces {version})"),
            (Language::English, None) => " (replaces the installed one)".to_string(),
        }
    }

    /// 带的程序：`ui`、`process`、`worker`、`builtin`。
    pub(crate) fn program(&self, kind: &str) -> String {
        let word = match (self, kind) {
            (Language::Chinese, "ui") => "界面程序",
            (Language::Chinese, "process") => "扩展程序",
            (Language::Chinese, "worker") => "小程序",
            (Language::Chinese, "builtin") => "内置功能",
            (Language::English, "ui") => "interface",
            (Language::English, "process") => "extension",
            (Language::English, "worker") => "worker program",
            (Language::English, "builtin") => "built-in feature",
            (_, other) => other,
        };
        word.to_string()
    }

    /// 带的另几样：子命令、后台页、吉祥物、接的平台、系统账号、几个配置项。
    pub(crate) fn carried(&self, what: Carried<'_>) -> String {
        match (self, what) {
            (Language::Chinese, Carried::Command(name)) => format!("命令 miyu {name}"),
            (Language::Chinese, Carried::Page) => "后台页".to_string(),
            (Language::Chinese, Carried::Mascot) => "吉祥物".to_string(),
            (Language::Chinese, Carried::Connection(platform)) => format!("接入 {platform}"),
            (Language::Chinese, Carried::SystemAccount) => "系统账号".to_string(),
            (Language::Chinese, Carried::Settings(n)) => format!("{n} 项设置"),
            (Language::English, Carried::Command(name)) => format!("command miyu {name}"),
            (Language::English, Carried::Page) => "admin page".to_string(),
            (Language::English, Carried::Mascot) => "mascot".to_string(),
            (Language::English, Carried::Connection(platform)) => format!("{platform} connection"),
            (Language::English, Carried::SystemAccount) => "system account".to_string(),
            (Language::English, Carried::Settings(1)) => "1 setting".to_string(),
            (Language::English, Carried::Settings(n)) => format!("{n} settings"),
        }
    }

    /// 一行的开头：包含、需要、一并删除、大小。
    pub(crate) fn plan_label(&self, label: PlanLabel) -> &'static str {
        match (self, label) {
            (Language::Chinese, PlanLabel::Includes) => "包含：",
            (Language::Chinese, PlanLabel::Needs) => "需要：",
            (Language::Chinese, PlanLabel::AlsoDeletes) => "一并删除：",
            (Language::Chinese, PlanLabel::Size) => "大小：",
            (Language::English, PlanLabel::Includes) => "Includes: ",
            (Language::English, PlanLabel::Needs) => "Needs: ",
            (Language::English, PlanLabel::AlsoDeletes) => "Also deletes: ",
            (Language::English, PlanLabel::Size) => "Size: ",
        }
    }

    /// 一行里几样之间隔开的。
    pub(crate) fn list_separator(&self) -> &'static str {
        match self {
            Language::Chinese => "、",
            Language::English => ", ",
        }
    }

    /// 一并删除的配置项：写了的那几个键。
    pub(crate) fn deleted_settings(&self, keys: &str) -> String {
        match self {
            Language::Chinese => format!("设置 {keys}"),
            Language::English => format!("settings {keys}"),
        }
    }

    /// 一并删除的状态目录。
    pub(crate) fn state_dir(&self) -> &'static str {
        match self {
            Language::Chinese => "状态目录",
            Language::English => "state directory",
        }
    }

    /// 问的那一句，后面不换行。
    pub(crate) fn proceed(&self, doing: Doing) -> &'static str {
        match (self, doing) {
            (Language::Chinese, Doing::Remove) => "继续卸载？[Y/n] ",
            (Language::Chinese, _) => "继续安装？[Y/n] ",
            (Language::English, Doing::Remove) => "Proceed with removal? [Y/n] ",
            (Language::English, _) => "Proceed with installation? [Y/n] ",
        }
    }

    /// 答了不。
    pub(crate) fn cancelled(&self) -> &'static str {
        match self {
            Language::Chinese => "已取消",
            Language::English => "Cancelled",
        }
    }

    /// 没得答（标准输入关着）：脚本里要写 `--yes`。
    pub(crate) fn unconfirmed(&self) -> &'static str {
        match self {
            Language::Chinese => "已取消：没有确认（不问用 --yes）",
            Language::English => "Cancelled: not confirmed (use --yes to skip the question)",
        }
    }
}

/// 做哪一件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Doing {
    Install,
    Restore,
    Remove,
}

/// 「包含」那一行除了程序的几样。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Carried<'a> {
    Command(&'a str),
    Page,
    Mascot,
    Connection(&'a str),
    SystemAccount,
    Settings(u64),
}

/// 一行开头的词。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlanLabel {
    Includes,
    Needs,
    AlsoDeletes,
    Size,
}
