//! 确认、提问那四种事件（核心 D-1、D-2，`kernel/asking.md`）：界面照它开、收抽屉，正文里写结果。这里只认出是哪一种、
//! 原样交 `body`，读成抽屉的形状在 `drawer/event.rs`（核心加了字段，头照旧能读）。

use serde_json::Value;

/// 确认、提问的一条事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asking {
    /// `question.asked`：她问的几道题。
    Asked(Value),
    /// `tool.approval_requested`：要确认的一次调用。
    Approval(Value),
    /// `question.answered`：答了（哪个头答的都算）。
    Answered(Value),
    /// `tool.approval_decided`：确认定了。
    Decided(Value),
}

impl Asking {
    /// 照事件的种类认；不是这四种的是 `None`。
    pub fn read(kind: &str, body: &Value) -> Option<Self> {
        let body = body.clone();
        Some(match kind {
            "question.asked" => Self::Asked(body),
            "tool.approval_requested" => Self::Approval(body),
            "question.answered" => Self::Answered(body),
            "tool.approval_decided" => Self::Decided(body),
            _ => return None,
        })
    }
}
