//! 起来时找沙盒的助手、探一次（`docs/blueprint/sandbox.md`「怎么走」第 1、2 条，施工 5-1）：只记日志，不影响别的。
//! 探到了什么，5-4 起权限策略照它定沙盒用不用得了。

use std::path::Path;
use std::time::Duration;

use crate::TARGET;

/// 最多等助手多久。
const WAIT: Duration = Duration::from_secs(5);

/// 主程序的真实位置是 `exe`（环境快照里的，顺着链接找到的本体）：找旁边的助手、探一次、记一行，`INFO sandbox`
/// 或者 `WARN sandbox unavailable`。不知道主程序在哪的，也当找不到。
pub(crate) fn probe(exe: Option<&Path>) {
    let Some(helper) = exe.and_then(miyu_sandbox::locate) else {
        tracing::warn!(target: TARGET, reason = "helper not found", "sandbox unavailable");
        return;
    };
    match miyu_sandbox::probe(&helper, WAIT) {
        Ok(probe) => tracing::info!(
            target: TARGET,
            helper = %helper.display(),
            platform = probe.platform.name(),
            mechanisms = %probe.mechanisms_text(),
            "sandbox"
        ),
        Err(error) => tracing::warn!(target: TARGET, reason = %error, "sandbox unavailable"),
    }
}

#[cfg(test)]
mod tests;
