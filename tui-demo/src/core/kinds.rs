//! 协议里几样有固定取值的字：权限级别、工具结果的状态、一轮结束的原因。读推送时就换成类型，
//! 之后代码里只比类型，不拿字符串比（`00-设计理念.md` 第四节「依赖与接口的规矩」第 5 条）。

use serde::Deserialize;

/// 实际的权限级别：常用的那一级，或者开着只读（`kernel/events-bodies.md` 的 `permission`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// 工作区。
    Workspace,
    /// 完全放开（界面上叫「开放权限」）。
    Full,
    /// 只读。
    ReadOnly,
}

impl Level {
    /// 协议里常用的那一级的写法：`workspace`、`full`；认不出的是 `None`。
    pub fn parse(text: &str) -> Option<Level> {
        match text {
            "workspace" => Some(Level::Workspace),
            "full" => Some(Level::Full),
            _ => None,
        }
    }
}

/// 工具结果的状态（`tool.result` 的 `status`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStatus {
    /// 成了。
    Ok,
    /// 出错。
    Error,
    /// 没做：被拒了。
    Denied,
    /// 打断了。
    Cancelled,
    /// 跳过了。
    Skipped,
    /// 新版本才有的取值：照「成了」画。
    Other,
}

impl ToolStatus {
    /// 照协议的写法读。
    pub fn parse(text: &str) -> ToolStatus {
        match text {
            "ok" => ToolStatus::Ok,
            "error" => ToolStatus::Error,
            "denied" => ToolStatus::Denied,
            "cancelled" => ToolStatus::Cancelled,
            "skipped" => ToolStatus::Skipped,
            _ => ToolStatus::Other,
        }
    }
}

/// 一轮结束的原因（`turn.ended` 的 `reason`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndReason {
    /// 照常结束。
    Completed,
    /// 打断了。
    Interrupted,
    /// 出错了。
    Error,
    /// 别的原因，照原样写出来。
    Other(String),
}

impl EndReason {
    /// 照协议的写法读。
    pub fn parse(text: &str) -> EndReason {
        match text {
            "completed" => EndReason::Completed,
            "interrupted" => EndReason::Interrupted,
            "error" => EndReason::Error,
            other => EndReason::Other(other.to_string()),
        }
    }
}
