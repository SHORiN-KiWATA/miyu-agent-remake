//! `miyu pkg` 给人看的字（施工 T-3，`docs/blueprint/cli/pkg.md`「给人看的字」）。

use super::Language;

impl Language {
    /// `install` 装好了 `id`。
    pub fn installed(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("装好了：{id}"),
            Language::English => format!("Installed {id}"),
        }
    }

    /// `remove` 卸掉了 `id`。
    pub fn uninstalled(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("卸掉了：{id}"),
            Language::English => format!("Removed {id}"),
        }
    }

    /// 清单装不上：哪里不对，知道第几行的带上。
    pub(crate) fn cannot_install(&self, problem: &str, line: Option<u64>) -> String {
        match (self, line) {
            (Language::Chinese, Some(line)) => format!("装不上：{problem}（第 {line} 行）"),
            (Language::Chinese, None) => format!("装不上：{problem}"),
            (Language::English, Some(line)) => format!("Cannot install: {problem} (line {line})"),
            (Language::English, None) => format!("Cannot install: {problem}"),
        }
    }

    /// 卸掉了的出厂的包接在名字后面的那一截。
    pub(crate) fn removed_mark(&self) -> &'static str {
        match self {
            Language::Chinese => "（已卸载）",
            Language::English => " (removed)",
        }
    }

    /// 写错的包接在后面的那一截：哪里不对。
    pub(crate) fn broken_mark(&self, problem: &str) -> String {
        match self {
            Language::Chinese => format!("（写错了：{problem}）"),
            Language::English => format!(" (broken: {problem})"),
        }
    }
}
