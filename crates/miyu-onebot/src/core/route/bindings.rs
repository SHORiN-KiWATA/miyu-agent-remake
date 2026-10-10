//! 谁是终端管理员（施工 O-31，`onebot.md` 第一条「平台工具（一）」第 3 条，「施工时定的」第 183 条）：平台身份在核心的主人对应表
//! 里对不对着本机账号，经核心的 `venue.binding` 问（`venues.md`「问对应表」），问到的按号记 `bridge.json` 的 `binding_seconds`。
//! 桥只用它挡在场所里做的事：禁言不动终端管理员（没在这个群说过话的也认得出），终端管理员叫她做的算管理的人。时刻由用的一方
//! 交进来，测试不用等。

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 问到的「是不是终端管理员」。
#[derive(Debug)]
pub(super) struct Bindings {
    /// 记多久。
    keep: Duration,
    /// 平台身份的原文 →（是不是，问到的时刻）。
    known: HashMap<String, (bool, Instant)>,
}

impl Bindings {
    /// 一个空的，问到的记 `keep`。
    pub(super) fn new(keep: Duration) -> Bindings {
        Bindings {
            keep,
            known: HashMap::new(),
        }
    }

    /// 平台身份 `id` 此刻 `now` 问到了：`admin` 是不是终端管理员。记下（盖掉原来的），顺手扔掉过了的。
    pub(super) fn note(&mut self, id: &str, admin: bool, now: Instant) {
        let keep = self.keep;
        self.known
            .retain(|_, (_, at)| now.saturating_duration_since(*at) < keep);
        self.known.insert(id.to_string(), (admin, now));
    }

    /// 平台身份 `id` 是不是终端管理员：问到以后到此刻 `now` 还不到 `keep` 的才知道，别的是空的（要问）。
    pub(super) fn known(&self, id: &str, now: Instant) -> Option<bool> {
        self.known
            .get(id)
            .filter(|(_, at)| now.saturating_duration_since(*at) < self.keep)
            .map(|(admin, _)| *admin)
    }
}

#[cfg(test)]
mod tests;
