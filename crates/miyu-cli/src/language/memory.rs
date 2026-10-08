//! `miyu memory` 给人看的字（施工 R-3 再补，`docs/blueprint/cli/memory.md`「给人看的字」）。

use super::Language;

impl Language {
    /// `list` 一条都没有。
    pub fn no_memories(&self) -> &'static str {
        match self {
            Language::Chinese => "还没有记忆。",
            Language::English => "No memories yet.",
        }
    }

    /// `search` 一条都没找到。
    pub fn nothing_found(&self) -> &'static str {
        match self {
            Language::Chinese => "没找到。",
            Language::English => "Nothing found.",
        }
    }

    /// `add` 记成了 `id`。
    pub fn remembered(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("记下了：{id}"),
            Language::English => format!("Remembered as {id}"),
        }
    }

    /// `edit` 改好了，新的一条是 `id`。
    pub fn changed(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("改好了：{id}"),
            Language::English => format!("Changed; now {id}"),
        }
    }

    /// 清空清掉了 `count` 条。
    pub fn cleared(&self, count: u64) -> String {
        match self {
            Language::Chinese => format!("清掉了 {count} 条。"),
            Language::English => format!("Cleared {count}."),
        }
    }

    /// 记的、改的太长：这一条 `chars` 个字，一条最多 `limit` 个。
    pub(crate) fn too_long(&self, chars: u64, limit: u64) -> String {
        match self {
            Language::Chinese => format!("太长了：这一条 {chars} 个字，一条最多 {limit} 个字。"),
            Language::English => {
                format!(
                    "Too long: this one has {chars} characters, a memory takes at most {limit}."
                )
            }
        }
    }

    /// 作废了的一条接在正文后面的那一截：`why` 是空的只说作废了。
    pub(crate) fn forgotten_mark(&self, why: &str) -> String {
        match (self, why.is_empty()) {
            (Language::Chinese, true) => "（已作废）".to_string(),
            (Language::Chinese, false) => format!("（已作废：{why}）"),
            (Language::English, true) => " (forgotten)".to_string(),
            (Language::English, false) => format!(" (forgotten: {why})"),
        }
    }
}
