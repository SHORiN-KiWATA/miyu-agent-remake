//! 工具的事件（`docs/designs/03-事件模型.md` 第三节「消息和工具结果怎么写」「确认的事件
//! 怎么写」）：调用的结果，请人确认，人的决定。

use serde::{Deserialize, Serialize};

use crate::block::Block;
use crate::id::CallId;
use crate::raw::RawJson;
use crate::text_enum::text_enum;
use crate::tool::Access;

/// `tool.result`：一个工具调用的结果。结果按到达的先后记进日志，
/// 投影时按调用的先后排（03 第六节）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    /// 这是哪一次调用的结果。那次调用在哪条消息里、是第几个，编号本身就写着。
    pub call_id: CallId,
    /// 结果怎样。
    pub status: ToolStatus,
    /// 给模型看的内容。已取消、被拒绝、已跳过的，是内核写的一句英文短句。
    pub blocks: Vec<Block>,
    /// 执行用了多少毫秒：从开始执行算到结束，由执行器量好，随结果送进来。
    /// 等人确认在执行之前，不算在里面。没真执行过的（例如被拒绝、已跳过）没有这一格。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

text_enum!(
    /// 工具调用的结果怎样。
    ToolStatus {
        /// 成功。
        Ok = "ok",
        /// 失败：工具执行了，但是出了错。错在哪，写在给模型看的内容里。
        Error = "error",
        /// 已取消：回合被打断时，还没有结果的调用（`02-内核.md` 第六节）。
        Cancelled = "cancelled",
        /// 被拒绝：执行前被人或守卫拒绝了。谁拒的，看事件的 `by`。
        Denied = "denied",
        /// 已跳过：人急着插话，剩下还没跑的调用不跑了（`02-内核.md` 第六节）。
        Skipped = "skipped",
    }
);

/// `tool.approval_requested`：请人确认一次调用（`02-内核.md` 第六节「确认怎么走」）。
/// `by` 是提问的模块。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequested {
    /// 请人确认的是哪一次调用。
    pub call_id: CallId,
    /// 要的是哪一类访问。收紧成只读时，要写入的当场拦下。
    pub access: Access,
    /// 提的放行规则：选本会话都允许、这个工作区以后都允许时，放行的就是它。写法由提问的模块定，
    /// 内核原样记。没提的，只能选允许这一次或者拒绝。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<RawJson>,
    /// 给头看的：为什么要问。写法由提问的模块定，内核原样记。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<RawJson>,
}

/// `tool.approval_decided`：人对一个请求的决定。`by` 是回答的人。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalDecided {
    /// 决定的是哪一次调用的请求。
    pub call_id: CallId,
    /// 选了哪一项。
    pub decision: Decision,
    /// 拒绝的理由，会写进给她看的结果。只跟着拒绝，没写就没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

text_enum!(
    /// 人对确认请求的决定：问人时的四个选项（`11-权限与沙盒.md` 第二节）。
    Decision {
        /// 允许这一次。
        Once = "once",
        /// 本会话都允许。
        Session = "session",
        /// 这个工作区以后都允许。
        Workspace = "workspace",
        /// 拒绝。拒绝以后她接着干（A13）。
        Deny = "deny",
    }
);

#[cfg(test)]
mod tests;
