//! 条目的编号（`docs/blueprint/view.md`「编号」）：只看日志里的位置，谁算、从哪一条开始算都一样。

use std::fmt;

use serde::Serialize;

use miyu_kernel::id::Seq;

/// 一条的编号。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct EntryId(String);

impl EntryId {
    /// 一条消息：`m<序号>`。
    #[must_use]
    pub fn message(seq: Seq) -> EntryId {
        EntryId(format!("m{seq}"))
    }

    /// 模型回复里的一块：`b<seen>.<块号>`。
    #[must_use]
    pub fn block(seen: Seq, index: usize) -> EntryId {
        EntryId(format!("b{seen}.{index}"))
    }

    /// 时间线的一段：第一步是 `first` 的那一段，`g` 加它的编号去掉 `b`。
    #[must_use]
    pub fn group(first: &EntryId) -> EntryId {
        EntryId(format!("g{}", first.0.trim_start_matches('b')))
    }

    /// 别的：引出它的那一条事件，`e<序号>`。
    #[must_use]
    pub fn event(seq: Seq) -> EntryId {
        EntryId(format!("e{seq}"))
    }

    /// 压缩那一条：照替代到的那一条，`c<序号>`（流式的进度、落了盘的检查点、摘要请求的记录是同一次压缩）。
    #[must_use]
    pub fn compaction(upto: Seq) -> EntryId {
        EntryId(format!("c{upto}"))
    }

    /// 只在视图流里有的旁白：排在序号是 `after` 的那一条后面的第 `n` 个，`x<序号>.<n>`。
    #[must_use]
    pub fn transient(after: u64, n: u32) -> EntryId {
        EntryId(format!("x{after}.{n}"))
    }

    /// 写成字。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
