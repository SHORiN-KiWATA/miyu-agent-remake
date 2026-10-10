//! 压缩用的数（`compaction.md`「对外的样子」的策略数据）：装进快照，以前造的快照里没有的几格读成出厂的。`snapshot.rs` 到了
//! 500 行，挪到这里（施工 6-11 再补）。

use serde::{Deserialize, Serialize};

use crate::pause::PauseNumbers;
use crate::rebuild::RebuildNumbers;
use crate::shorten::ShortenNumbers;

/// 压缩用的数（`compaction.md`「对外的样子」的策略数据）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionNumbers {
    /// 输出预留的上限。
    pub reserve_cap: u64,
    /// 余量。
    pub margin: u64,
    /// 压缩线至多是窗口的百分之几（施工 6-11 再补）。以前造的快照里没有，读成出厂的 85：老会话也照它。
    #[serde(default = "default_line_percent")]
    pub line_percent: u64,
    /// 余量至多是窗口的百分之几（施工 6-11 再补）。以前造的快照里没有，读成出厂的 5。
    #[serde(default = "default_margin_percent")]
    pub margin_percent: u64,
    /// 本地估算时一张图算多少 token。
    pub image: u64,
    /// 本地估算时一个文件算多少 token。
    pub file: u64,
    /// 尾巴至多多少 token（施工 6-2 下）。6-2（上）造的快照里没有，读成出厂的 16000。
    #[serde(default = "default_tail")]
    pub tail: u64,
    /// 提前压好的提前量的上限（施工 6-11 上）。以前造的快照里没有，读成出厂的 16000：老会话也提前压。
    #[serde(default = "default_lead")]
    pub lead: u64,
    /// 压后重建的数（施工 6-5）。以前造的快照里没有，读成没有：不重读。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebuild: Option<RebuildNumbers>,
    /// 熔断的数（施工 6-6 上）。以前造的快照里没有，读成没有：不熔断。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause: Option<PauseNumbers>,
    /// 截短重试的数（施工 6-6 中）。以前造的快照里没有，读成没有：摘要请求超长照失败算。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shorten: Option<ShortenNumbers>,
}

/// 尾巴的预算上限的出厂值（`compaction.md` 第三条第 2 条，2026-09-29 项目主人定）。
pub const TAIL: u64 = 16_000;

/// 读 6-2（上）造的快照时，没有 `tail` 的那一格。
fn default_tail() -> u64 {
    TAIL
}

/// 提前量的上限的出厂值（`09-压缩.md` 第十节，2026-10-07 项目主人定）。
pub const LEAD: u64 = 16_000;

/// 读 6-11（上）以前造的快照时，没有 `lead` 的那一格。
fn default_lead() -> u64 {
    LEAD
}

/// 压缩线至多是窗口的百分之几的出厂值（`09-压缩.md` 第十节，2026-10-10 项目主人定）。
pub const LINE_PERCENT: u64 = 85;

/// 读 6-11 再补以前造的快照时，没有 `line_percent` 的那一格。
fn default_line_percent() -> u64 {
    LINE_PERCENT
}

/// 余量至多是窗口的百分之几的出厂值（施工 6-11 再补）：窗口小的不至于减成没有线。
pub const MARGIN_PERCENT: u64 = 5;

/// 读 6-11 再补以前造的快照时，没有 `margin_percent` 的那一格。
fn default_margin_percent() -> u64 {
    MARGIN_PERCENT
}
