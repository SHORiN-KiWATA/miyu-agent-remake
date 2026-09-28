//! 起来时找沙盒的助手、探一次（`docs/blueprint/sandbox.md`「怎么走」第 1、2 条，施工 5-1）：记一行日志；探到了手段的，
//! 助手交给协议端点，会话照它判执行命令、给命令写沙盒（施工 5-4 上）。起不起得来不看它。

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::TARGET;

/// 最多等助手多久。
const WAIT: Duration = Duration::from_secs(5);

/// 主程序的真实位置是 `exe`（环境快照里的，顺着链接找到的本体）：找旁边的助手、探一次、记一行，`INFO sandbox`
/// 或者 `WARN sandbox unavailable`。不知道主程序在哪的，也当找不到。探到了手段的，交回助手；手段是空的，照沙盒
/// 用不了办，交回空的。
pub(crate) fn probe(exe: Option<&Path>) -> Option<PathBuf> {
    let Some(helper) = exe.and_then(miyu_sandbox::locate) else {
        tracing::warn!(target: TARGET, reason = "helper not found", "sandbox unavailable");
        return None;
    };
    match miyu_sandbox::probe(&helper, WAIT) {
        Ok(probe) => {
            tracing::info!(
                target: TARGET,
                helper = %helper.display(),
                platform = probe.platform.name(),
                mechanisms = %probe.mechanisms_text(),
                "sandbox"
            );
            (!probe.mechanisms.is_empty()).then_some(helper)
        }
        Err(error) => {
            tracing::warn!(target: TARGET, reason = %error, "sandbox unavailable");
            None
        }
    }
}

#[cfg(test)]
mod tests;
