//! 群会话（施工 O-13 中，`docs/construction/O-13-群里的一行和格式说明（中）.md`）：造会话时 system 接上格式说明，快照记下这台
//! 机器的时区和空的一条写什么；组装时群里的人说的照它渲染成一行一条（`miyu-assemble` 的 `group.rs`）。说明只在造的时候拼，
//! 换人格、换预设重拼时照旧快照记的时区。

use miyu_kernel::template::Template;
use miyu_kernel::time::UtcOffset;
use serde::{Deserialize, Serialize};

use crate::snapshot::{BuildError, Snapshot};

/// 群会话钉下的（快照的 `group`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupChat {
    /// 造会话时这台机器的时区：比 UTC 早多少分钟，东边是正的。换了时区的机器上载入也照它。
    pub offset: i32,
    /// 正文、带的东西都没有的那一条写什么（`core/venues/no-text.txt` 的原文，行尾的换行也算）。
    pub no_text: String,
    /// 群聊近况（施工 O-13 下）。O-13（中）造的没有：不出近况；没有的不写，字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recent: Option<GroupRecent>,
}

/// 群聊近况一块最多多少字节（照旧版的 80000）。
pub const RECENT_BUDGET: u64 = 80_000;

/// 群聊近况的字和预算（施工 O-13 下）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupRecent {
    /// 块头那一行（`core/venues/recent-open.txt` 的原文）。
    pub open: String,
    /// 缺口提示（`core/venues/recent-omitted.txt` 的原文）：字段 `count` 是没装下的条数。
    pub omitted: String,
    /// 一块最多多少字节。
    pub budget: u64,
}

impl GroupChat {
    /// 组装器要的：时区换成内核的类型，空的那一句去掉行尾的空白。
    ///
    /// # Errors
    ///
    /// 时区超出了 −14:00 到 +14:00：快照坏了。
    pub(crate) fn texts(&self) -> Result<miyu_assemble::GroupChat, BuildError> {
        let offset = UtcOffset::from_minutes(self.offset).ok_or(BuildError::Offset(self.offset))?;
        let recent = self.recent.as_ref().map(GroupRecent::texts).transpose()?;
        Ok(miyu_assemble::GroupChat {
            offset,
            no_text: self.no_text.trim_end().to_string(),
            recent,
        })
    }
}

impl GroupRecent {
    /// 组装器要的：缺口提示照模板读，字段只能是 `count`。
    fn texts(&self) -> Result<miyu_assemble::GroupRecent, BuildError> {
        let broken = |error| BuildError::Texts {
            which: "group chat recent",
            error,
        };
        let omitted = Template::parse(&self.omitted).map_err(broken)?;
        omitted
            .render(&[("count", "1")].into_iter().collect())
            .map_err(broken)?;
        Ok(miyu_assemble::GroupRecent {
            open: self.open.clone(),
            omitted,
            budget: usize::try_from(self.budget).unwrap_or(usize::MAX),
        })
    }
}

impl Snapshot {
    /// 带上群会话：system 接上格式说明 `note`（第二块，和子会话的场所说明同一个位置，[`Snapshot::with_venue`]），记下 `group`。
    #[must_use]
    pub fn with_group(self, note: &str, group: GroupChat) -> Snapshot {
        let mut snapshot = self.with_venue(note);
        snapshot.group = Some(group);
        snapshot
    }
}

#[cfg(test)]
mod tests;
