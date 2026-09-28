//! 这台机器上的沙盒能不能用、为什么（`docs/blueprint/sandbox.md`「能不能用」，施工 5-4 下）：核心起来时探一次得出来，
//! 交给协议端点。能用的带着助手的路径，造会话、载入时交给会话；用不了的带着原因，握手时报给头。

use std::path::{Path, PathBuf};

/// 这台机器上的沙盒能不能用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// 能用：助手在这里。
    Usable(PathBuf),
    /// 用不了，为什么。
    Unusable(Unusable),
}

/// 沙盒用不了的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unusable {
    /// 主程序旁边没有助手，或者不知道主程序在哪。
    HelperMissing,
    /// 助手跑不起来、超时、退出码不是 0、说的读不懂。
    HelperFailed,
    /// 探成了，这台机器上却没有能用的手段。
    NoMechanism,
}

impl Availability {
    /// 能用的，助手在哪；用不了的是空的。
    pub fn helper(&self) -> Option<&Path> {
        match self {
            Availability::Usable(helper) => Some(helper),
            Availability::Unusable(_) => None,
        }
    }
}

impl Unusable {
    /// 三种原因，照先后。
    pub const ALL: [Unusable; 3] = [
        Unusable::HelperMissing,
        Unusable::HelperFailed,
        Unusable::NoMechanism,
    ];

    /// 协议上的写法（`docs/blueprint/protocol.md` 握手的回应）。
    pub const fn code(self) -> &'static str {
        match self {
            Unusable::HelperMissing => "helper_missing",
            Unusable::HelperFailed => "helper_failed",
            Unusable::NoMechanism => "no_mechanism",
        }
    }

    /// 照协议上的写法读回来；不认得的是空的。
    pub fn from_code(code: &str) -> Option<Unusable> {
        Unusable::ALL
            .into_iter()
            .find(|reason| reason.code() == code)
    }
}

#[cfg(test)]
mod tests;
