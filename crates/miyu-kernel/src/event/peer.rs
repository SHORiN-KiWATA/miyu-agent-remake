//! 跨会话的事件 `peer.idle`（施工 C-1，`docs/blueprint/cross-session.md`「事件 peer.idle」，`03-事件模型.md` 第三节）：
//! 等的那个会话空下来了，或者等不到了，记在等的那一边。订的记录是那次调用的效果 `peer.watch`（`effect.rs`）。
//!
//! 在不在等、`by` 对不对得上，由账本查（`ledger/peers.rs`）。谁交来、到了叫不叫醒她、渲染成什么样，随施工 C-6。名字不叫
//! `session.*`：渲染表里 `session.*` 一律不进上下文（`kernel/request.md`「组装」第 3 条）。

use serde::{Deserialize, Serialize};

use crate::id::SessionId;
use crate::text_enum::text_enum;

/// `peer.idle`：等的那个会话空下来了，或者等不到了。`by`：`idle` 的是那个会话，`expired`、`gone` 的是内核。不带回合编号：
/// 它是别处来的，撤哪一轮都不拿走（`cross-session.md` 第七条第 2 款）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerIdle {
    /// 等的是哪个会话。
    pub session: SessionId,
    /// 为什么来。
    pub reason: IdleReason,
    /// `idle` 的才有：它最近结束的那一轮最后一条有字的回复的第一行，截到 `peers.status_chars`。那一轮没说话的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

text_enum!(
    /// `peer.idle` 为什么来。不认识的是新版本才有的：原样留着，也算等到了头，账本不查它的 `by`（`kernel/history.md`）。
    IdleReason {
        /// 空下来了：`by` 是那个会话。
        Idle = "idle",
        /// 订了 `peers.watch_hours` 没等到，作废了：`by` 是内核。
        Expired = "expired",
        /// 那个会话不在了：`by` 是内核。
        Gone = "gone",
    }
);

#[cfg(test)]
mod tests;
