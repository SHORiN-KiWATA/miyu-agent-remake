//! 给头看的限额（施工 6-3 补，`docs/blueprint/kernel/session.md` 的 `context_limits()`、`protocol.md` 的 `subscribe`）：
//! 会话实际用的模型的窗口，和内核自己判到线用的那一条压缩线。只读，不出动作。

use serde::Serialize;

use super::Session;

/// 给头看的限额：头照它画「用量 / 窗口」、算离压缩还有多少。写成 JSON 就是协议里 `subscribe` 回应的 `limits`：
/// 没有的格不写，从不写 `null`（`protocol.md`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ContextLimits {
    /// 上下文窗口，token 数。没交过限额的、模型的资料没报的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<u64>,
    /// 压缩线（`compaction.md` 第二条第 2 条），和内核判到线用的是同一条。头不照公式自己算：公式里的输出预留、余量在
    /// 策略里。没有窗口的、策略里没有压缩的、算不出正数的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compaction_line: Option<u64>,
}

impl Session {
    /// 给头看的限额（施工 6-3 补）：执行器交完 `Input::Limits` 向内核要一份，交给头（`session/actor.md`）。
    pub fn context_limits(&self) -> ContextLimits {
        ContextLimits {
            window: self.limits.as_ref().and_then(|limits| limits.window),
            compaction_line: self.line(),
        }
    }
}
