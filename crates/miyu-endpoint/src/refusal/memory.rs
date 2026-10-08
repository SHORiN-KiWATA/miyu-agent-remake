//! 人碰记忆时的四种拒绝（施工 R-3 补，`memory.md`「协议」）：`refusal.rs` 到了 500 行，挪到这里。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 这个会话里没有记忆（施工 R-3 补，`memory.md`「协议」）：范围 `off`，或者是场所会话（群里的随 O 线）。
    pub(crate) const MEMORY_UNAVAILABLE: Refusal = Refusal {
        code: REFUSED,
        reason: "memory_unavailable",
        data: None,
    };
    /// 没有这一条记忆，或者听众不合（施工 R-3 补）：不合的当没有，不让人知道有。
    pub(crate) const UNKNOWN_MEMORY: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_memory",
        data: None,
    };
    /// 那一条已经改掉、作废、清掉了（施工 R-3 补）。
    pub(crate) const MEMORY_NOT_CURRENT: Refusal = Refusal {
        code: REFUSED,
        reason: "memory_not_current",
        data: None,
    };
    /// 一条记忆太长（施工 R-3 补）：`data.chars` 是它有几个字，`data.limit` 是上限。
    pub(crate) fn memory_too_long(chars: usize, limit: usize) -> Refusal {
        let mut refusal = Refusal::with("memory_too_long", "chars", serde_json::json!(chars));
        if let Some(data) = &mut refusal.data {
            data.insert("limit".to_string(), serde_json::json!(limit));
        }
        refusal
    }
}
