//! `miyu config` 给人看的字（施工 8-2，`docs/blueprint/cli/config.md`「给人看的字」，样子在 `config.md`「样子」）；
//! `miyu ask` 起头说配置有错、项目配置没信任的那两行。报错的整句话由核心照连接的语言说，这里只有命令行自己的几个词。
//! 改、写、信任的几个子命令的字在 `config_write.rs`（施工 8-3）。

use super::Language;

impl Language {
    /// 报错一行里的级别：`错误`、`警告`。
    pub(crate) fn severity(&self, level: &str) -> &'static str {
        match (self, level) {
            (Language::Chinese, "warning") => "警告",
            (Language::Chinese, _) => "错误",
            (Language::English, "warning") => "warning",
            (Language::English, _) => "error",
        }
    }

    /// `check` 最后一行的合计：没有的那一样不写，都没有的说「没有问题」。
    pub(crate) fn problems_total(&self, errors: usize, warnings: usize) -> String {
        let plural = |n: usize, one: &str, many: &str| match n {
            1 => format!("1 {one}"),
            n => format!("{n} {many}"),
        };
        let parts: Vec<String> = match self {
            Language::Chinese => [(errors, "处错误"), (warnings, "处警告")]
                .iter()
                .filter(|(n, _)| *n > 0)
                .map(|(n, what)| format!("{n} {what}"))
                .collect(),
            Language::English => [
                (errors, plural(errors, "error", "errors")),
                (warnings, plural(warnings, "warning", "warnings")),
            ]
            .into_iter()
            .filter(|(n, _)| *n > 0)
            .map(|(_, said)| said)
            .collect(),
        };
        match (self, parts.is_empty()) {
            (Language::Chinese, true) => "没有问题".to_string(),
            (Language::English, true) => "No problems".to_string(),
            (Language::Chinese, false) => parts.join("，"),
            (Language::English, false) => parts.join(", "),
        }
    }

    /// 命令行自己读文件时的整份问题（`check`）：读不了、太大、不是 UTF-8。话和核心的同一句。
    pub(crate) fn unreadable_config(&self, why: &str) -> String {
        match self {
            Language::Chinese => format!("读不了这份文件：{why}。"),
            Language::English => format!("Cannot read this file: {why}."),
        }
    }

    /// 太大的。
    pub(crate) fn config_too_big(&self) -> &'static str {
        match self {
            Language::Chinese => "这份文件超过 1 MiB，不读。",
            Language::English => "The file is over 1 MiB and is not read.",
        }
    }

    /// 不是 UTF-8 的。
    pub(crate) fn config_not_utf8(&self) -> &'static str {
        match self {
            Language::Chinese => "这份文件不是 UTF-8。",
            Language::English => "The file is not UTF-8.",
        }
    }

    /// `explain` 第一行：名字、键、说明、什么时候生效。
    pub(crate) fn explain_header(
        &self,
        name: &str,
        key: &str,
        description: &str,
        applies: &str,
    ) -> String {
        let applies = self.applies(applies);
        match self {
            Language::Chinese => format!("{name}（{key}）：{description}{applies}。"),
            Language::English => format!("{name} ({key}): {description} {applies}."),
        }
    }

    /// 什么时候生效，句首大写（`explain`）。
    fn applies(&self, applies: &str) -> &'static str {
        match (self, applies) {
            (Language::Chinese, "new_session") => "以后开的会话生效",
            (Language::Chinese, "head_start") => "下次打开界面时生效",
            (Language::Chinese, _) => "当场生效",
            (Language::English, "new_session") => "Applies to sessions opened from now on",
            (Language::English, "head_start") => "Takes effect the next time the interface opens",
            (Language::English, _) => "Takes effect at once",
        }
    }

    /// 什么时候生效，接在句子中间（`set`、`edit` 印的那一行，施工 8-3）：英文小写开头。
    pub(crate) fn applies_after(&self, applies: &str) -> String {
        match self {
            Language::Chinese => self.applies(applies).to_string(),
            Language::English => {
                let said = self.applies(applies);
                let mut chars = said.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_lowercase().chain(chars).collect()
                })
            }
        }
    }

    /// 一层的名字（`explain`）：`default`、`system`、`personal`、`project`、`env`。
    pub(crate) fn layer_name(&self, layer: &str) -> &'static str {
        match (self, layer) {
            (Language::Chinese, "system") => "系统配置",
            (Language::Chinese, "personal") => "个人设置",
            (Language::Chinese, "project") => "项目配置",
            (Language::Chinese, "env") => "环境变量",
            (Language::Chinese, _) => "默认值",
            (Language::English, "system") => "system config",
            (Language::English, "personal") => "personal settings",
            (Language::English, "project") => "project config",
            (Language::English, "env") => "environment",
            (Language::English, _) => "default",
        }
    }

    /// 生效的那一行的记号；环境变量压着的说只管这一次启动。
    pub(crate) fn in_effect(&self, env: bool) -> &'static str {
        match (self, env) {
            (Language::Chinese, false) => "← 生效",
            (Language::Chinese, true) => "← 生效，只管这一次启动",
            (Language::English, false) => "← in effect",
            (Language::English, true) => "← in effect, for this launch only",
        }
    }

    /// 写了、不算的那一行的记号，照原因码。
    pub(crate) fn not_counted(&self, problem: &str) -> String {
        let why = match (self, problem) {
            (Language::Chinese, "untrusted_project") => "还没信任",
            (Language::Chinese, "not_tightening") => "比下面几层宽",
            (Language::Chinese, _) => "不能写在这一层",
            (Language::English, "untrusted_project") => "not trusted yet",
            (Language::English, "not_tightening") => "looser than the layers below",
            (Language::English, _) => "not allowed in this layer",
        };
        match self {
            Language::Chinese => format!("← 不算：{why}"),
            Language::English => format!("← does not count: {why}"),
        }
    }

    /// `path --project` 还没有这个文件。
    pub(crate) fn no_file_yet(&self) -> &'static str {
        match self {
            Language::Chinese => "还没有这个文件",
            Language::English => "This file does not exist yet",
        }
    }

    /// `miyu ask` 起头：这里的项目配置 `file` 还没信任，这次没用它（施工 8-3）。
    pub(crate) fn untrusted_project(&self, file: &str) -> String {
        match self {
            Language::Chinese => format!(
                "· 这里的项目配置 {file} 还没信任，这次没用它：miyu config trust 看一眼再定"
            ),
            Language::English => format!(
                "· The project config at {file} is not trusted yet, so it was not used: run miyu config trust to review it"
            ),
        }
    }

    /// `miyu ask` 起头：配置里有几处错误。
    pub(crate) fn config_errors(&self, errors: u64) -> String {
        match (self, errors) {
            (Language::Chinese, n) => format!("· 配置里有 {n} 处错误：miyu config check 看是哪里"),
            (Language::English, 1) => {
                "· 1 error in the config: run miyu config check to see it".to_string()
            }
            (Language::English, n) => {
                format!("· {n} errors in the config: run miyu config check to see them")
            }
        }
    }
}
