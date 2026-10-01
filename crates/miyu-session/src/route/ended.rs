//! 一次请求说完了以后路由记什么（`docs/blueprint/models.md`「怎么走」第四条第 3、5、6 条，第五条，施工 8-9）。
//!
//! - 成了：这个候选的失败次数清零，key 整个的认证失败次数也清零；钉住的池，钉着的成员换成它（限额跟着换成它的）；会话的
//!   key 换成它。主请求的「说到一半断了」放开。
//! - 出错：限速、可重试、认证失败三类照分类记冷却，记一行 `INFO endpoint cooling`。别的分类不记、不换。
//!   - 收到过增量才出错的（说到一半断了）：主请求记下它，下一次主请求还发给它；不换。
//!   - 只有这一个候选的：不换，照旧交供应商说的要等多久。
//!   - 还有别的候选这时就能用的：换（`failover`），不带要等多久，记一行 `INFO failover`。
//!   - 别的候选都在冷却、只剩等的：也算换，要等多久是所有候选里最早恢复的那一个还要多久，和供应商说的取长的。
//! - 被叫停：什么都不记，主请求的「说到一半断了」放开。

use std::sync::{Arc, Mutex};

use miyu_kernel::event::ErrorClass;
use miyu_kernel::time::Timestamp;

use super::choice::{Choice, Who, name, until};
use super::{Pinned, Routes, lock};
use crate::TARGET;
use crate::clock::wall_now;
use crate::config::TurnConfig;

/// 发出去的这一次：发给了谁、别的候选是谁，说完了照它记。
pub(in crate::route) struct Tried {
    /// 会话钉着的那一份。
    pub(in crate::route) pinned: Arc<Mutex<Pinned>>,
    /// 核心一份的：冷却表在模型资料里，换成员要重算限额。
    pub(in crate::route) routes: Routes,
    /// 这一轮的配置：换成员时照它算限额。
    pub(in crate::route) config: TurnConfig,
    /// 是主请求（摘要请求也算）：只有它认「说到一半断了」。
    pub(in crate::route) main: bool,
    /// 引用是钉住的池：成了就钉到这个成员。
    pub(in crate::route) pins: bool,
    /// 发给的那一个。
    pub(in crate::route) picked: Choice,
    /// 别的候选，照先后。
    pub(in crate::route) others: Vec<Who>,
}

/// 出错以后交给内核的：换没换端点、要等多久。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::route) struct Switch {
    /// 换了端点：内核不管分类当场再来。
    pub(in crate::route) failover: bool,
    /// 要等多久，毫秒；没有的不写。
    pub(in crate::route) wait_ms: Option<u64>,
}

impl Tried {
    /// 成了。
    pub(in crate::route) fn succeeded(self) {
        let picked = &self.picked;
        self.routes
            .data
            .cooldown(|table, _| table.succeed(&picked.who));
        let changed =
            self.pins && picked.member.is_some() && lock(&self.pinned).member != picked.member;
        // 限额照模型资料算，不拿着钉着的锁算。
        let limits = changed.then(|| self.routes.limits(&self.config, &picked.target));
        let mut pinned = lock(&self.pinned);
        if self.main {
            pinned.sticky = None;
        }
        if let Some(key) = &picked.who.key {
            pinned
                .moved
                .insert(picked.who.provider.clone(), key.clone());
        }
        if let Some(limits) = limits {
            pinned.member.clone_from(&picked.member);
            pinned.limits = limits;
        }
    }

    /// 出错了，分类 `class`，供应商说了要等 `said_ms` 毫秒（没说的没有）；`cut`：收到过增量才出错的。
    pub(in crate::route) fn failed(
        self,
        class: &ErrorClass,
        said_ms: Option<u64>,
        cut: bool,
    ) -> Switch {
        let now = wall_now();
        let picked = &self.picked;
        let data = &self.routes.data;
        let recorded =
            data.cooldown(|table, rules| table.fail(&picked.who, class, said_ms, rules, now));
        if let Some(recorded) = recorded {
            tracing::info!(
                target: TARGET,
                provider = picked.who.provider.as_str(),
                key = picked.at.map(|at| at + 1),
                model = picked.who.model.as_str(),
                class = class.as_str(),
                for_ms = recorded.for_ms,
                failures = recorded.failures,
                "endpoint cooling"
            );
        }
        if self.main {
            lock(&self.pinned).sticky = cut.then(|| picked.who.clone());
        }
        let stay = Switch {
            failover: false,
            wait_ms: said_ms,
        };
        if cut || recorded.is_none() || self.others.is_empty() {
            return stay;
        }
        let (next, earliest) = data.cooldown(|table, _| {
            let next = self
                .others
                .iter()
                .find(|other| table.cooling(&other.who, now).is_none());
            let earliest = self
                .others
                .iter()
                .map(|other| &other.who)
                .chain([&picked.who])
                .filter_map(|who| table.cooling(who, now))
                .map(|cooling| cooling.until)
                .min();
            (next.cloned(), earliest)
        });
        match next {
            Some(next) => {
                failover(picked, &next, class);
                Switch {
                    failover: true,
                    wait_ms: None,
                }
            }
            None => Switch {
                failover: true,
                wait_ms: Some(wait(now, earliest, said_ms)),
            },
        }
    }

    /// 被叫停了：什么都不记，主请求的「说到一半断了」放开。
    pub(in crate::route) fn cancelled(self) {
        if self.main {
            lock(&self.pinned).sticky = None;
        }
    }
}

/// 别的候选都在冷却时要等多久：最早恢复的那一个还要多久，和供应商说的取长的。
fn wait(now: Timestamp, earliest: Option<Timestamp>, said_ms: Option<u64>) -> u64 {
    let earliest = earliest.map_or(0, |then| until(now, then));
    earliest.max(said_ms.unwrap_or(0))
}

/// 记一行换端点：从哪个到哪个；只换 key、模型没变的写换到第几个 key（「施工时定的」8-9）。
fn failover(from: &Choice, to: &Who, class: &ErrorClass) {
    let same = to.who.provider == from.who.provider && to.who.model == from.who.model;
    let from_name = format!("{}/{}", from.who.provider, from.who.model);
    match (same, to.at) {
        (true, Some(at)) => tracing::info!(
            target: TARGET,
            from = from_name.as_str(),
            key = at + 1,
            class = class.as_str(),
            "failover"
        ),
        _ => tracing::info!(
            target: TARGET,
            from = from_name.as_str(),
            to = name(&to.who, None).as_str(),
            class = class.as_str(),
            "failover"
        ),
    }
}
