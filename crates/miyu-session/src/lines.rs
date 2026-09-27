//! 运行日志里的几个写法（`28-运行日志.md`），actor 和执行工具的端口共用（施工 4-2 从 actor 挪出来）。

use std::time::Duration;

use miyu_kernel::event::{Transient, TransientBody};
use miyu_kernel::request::{Difference, Role};

use crate::TARGET;

/// 前缀第一处不同在哪，写成一个词：`tools`、`system`，或者 `message:<第几条，从 0 数起>:<角色>`。
pub(crate) fn where_(changed: &Difference) -> String {
    match changed {
        Difference::Tools => "tools".to_string(),
        Difference::System => "system".to_string(),
        Difference::Message { index, role } => {
            let role = match role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool",
            };
            format!("message:{index}:{role}")
        }
    }
}

/// 等着重试的状态提示，记一条 `WARN`：第几次、一共几次、等多久、出错的分类。原话不写：供应商的
/// 出错信息里可能回显请求里的字。
pub(crate) fn retrying(transient: &Transient) {
    if let TransientBody::Status(status) = &transient.body {
        let retry = &status.retry;
        tracing::warn!(
            target: TARGET,
            seen = status.seen.get(),
            attempt = retry.attempt,
            limit = retry.limit,
            wait_ms = retry.wait_ms,
            class = retry.class.as_str(),
            "retrying"
        );
    }
}

/// 一段时间，毫秒。
pub(crate) fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
