//! 起来时找沙盒的助手、探一次（`docs/blueprint/sandbox.md`「怎么走」第 1、2 条，施工 5-1）：记一行日志，探到的结果交给
//! 协议端点：握手时报给头，能用的把助手交给会话（施工 5-4 上、下）。再算出沙盒的缓存放在哪（施工 5-4 下）。起不起得来
//! 不看这两样。

use std::path::{Path, PathBuf};

use miyu_sandbox::{Availability, Unusable};
use miyu_store::env::Env;
use miyu_store::root::{RootError, cache_root};
use std::time::Duration;

use crate::TARGET;

/// 最多等助手多久。
const WAIT: Duration = Duration::from_secs(5);

/// 主程序的真实位置是 `exe`（环境快照里的，顺着链接找到的本体）：找旁边的助手、探一次、记一行，`INFO sandbox`
/// 或者 `WARN sandbox unavailable`。不知道主程序在哪的，也当找不到。探到了手段的能用，带着助手；手段是空的、找不到、
/// 探不成的用不了，带着原因。
pub(crate) fn probe(exe: Option<&Path>) -> Availability {
    let Some(helper) = exe.and_then(miyu_sandbox::locate) else {
        tracing::warn!(target: TARGET, reason = "helper not found", "sandbox unavailable");
        return Availability::Unusable(Unusable::HelperMissing);
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
            if probe.mechanisms.is_empty() {
                Availability::Unusable(Unusable::NoMechanism)
            } else {
                Availability::Usable(helper)
            }
        }
        Err(error) => {
            tracing::warn!(target: TARGET, reason = %error, "sandbox unavailable");
            Availability::Unusable(Unusable::HelperFailed)
        }
    }
}

/// 沙盒的缓存放在哪（施工 5-4 下）：缓存目录（`store.md` 第 3 条）下的 `sandbox`，各账号一份在它下面；再带上你的 cargo
/// 目录：`cargo_home` 是核心的环境里的 `CARGO_HOME`，设了、不是空的照它，不然 `~/.cargo`。算不出缓存目录的，记一行
/// `WARN sandbox cache unavailable`，交回空的：沙盒里不设工具链的变量。
pub(crate) fn cache(
    env: &Env,
    cargo_home: Option<std::ffi::OsString>,
) -> Option<(PathBuf, Option<PathBuf>)> {
    let root = match cache_root(env) {
        Ok(root) => root.join("sandbox"),
        Err(error) => {
            tracing::warn!(target: TARGET, reason = why(&error), "sandbox cache unavailable");
            return None;
        }
    };
    let cargo_home = cargo_home
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| env.home.as_ref().map(|home| home.join(".cargo")));
    Some((root, cargo_home))
}

/// 算不出缓存目录的原因，运行日志里一律英文。
fn why(error: &RootError) -> &'static str {
    match error {
        RootError::NoHome => "no home directory",
        RootError::NoLocalAppData => "no LOCALAPPDATA",
        RootError::RelativeMiyuHome(_) => "relative MIYU_HOME",
    }
}

#[cfg(test)]
mod tests;
