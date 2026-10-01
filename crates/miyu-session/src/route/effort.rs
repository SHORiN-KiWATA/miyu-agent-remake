//! 一次请求带哪一档思考强度（施工 8-18，`docs/blueprint/models.md`「怎么走」第十一条第 4、6、7 条）。
//!
//! - 每次请求挑好端点以后，照真发的那个模型：会话给它记的一格（在这时的档位里才算），再是配置的默认，再没有就不带
//!   （`miyu_models::effort::pick`）。会话记的不在档位里的记一行 `WARN effort not available`，日志里那一格不改。
//! - 空闲超时照那一档放大（`miyu_models::effort::idle_factor`）。
//! - 给头看的那一档：限额里的那个模型（轮换的池没有），照端口记着的配置现算，算法同上，不记 `WARN`。

use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::event::EffortInUse;
use miyu_models::effort;
use miyu_models::facts::{Facts, facts};
use miyu_models::provider::{self, Target};

use super::{NONE, Pinned, Route};
use crate::TARGET;

impl Route {
    /// 给头看的那一档：限额里的那个模型照记着的配置算。轮换的池、还没有模型的、那一家这时用不了的没有。
    pub(super) fn shown_effort(&self) -> Option<EffortInUse> {
        let pinned = self.lock();
        let model = pinned.limits.model.clone();
        if model.endpoint.as_str() == NONE {
            return None;
        }
        let name = effort::key(model.endpoint.as_str(), model.model.as_str());
        let cell = pinned.efforts.get(&name).cloned();
        let config = Arc::clone(&pinned.config);
        drop(pinned);
        let values = config.resolved.values();
        let target = Target {
            provider: self
                .shared
                .data
                .with(|knowledge| provider::provider(&values, knowledge, model.endpoint.as_str()))
                .ok()?,
            model: model.model.as_str().to_string(),
        };
        let (facts, _) = self
            .shared
            .data
            .with(|knowledge| facts(&config.resolved, knowledge, &target.provider, &target.model));
        chosen(cell.as_deref(), &facts, &target, false)
    }
}

/// 会话给 `target` 那个模型记的那一格。
pub(super) fn cell(pinned: &Pinned, target: &Target) -> Option<String> {
    pinned
        .efforts
        .get(&effort::key(&target.provider.id, &target.model))
        .cloned()
}

/// 照会话记的一格 `cell` 和 `target` 这时的资料 `facts` 挑一档；`warn` 的，会话记的不在档位里了记一行。
pub(super) fn chosen(
    cell: Option<&str>,
    facts: &Facts,
    target: &Target,
    warn: bool,
) -> Option<EffortInUse> {
    let picked = effort::pick(cell, facts.effort.value.as_deref(), facts.levels());
    if warn && let Some(level) = &picked.stale {
        let model = effort::key(&target.provider.id, &target.model);
        tracing::warn!(target: TARGET, model = model.as_str(), level = level.as_str(), "effort not available");
    }
    picked.used
}

/// 这一次的空闲超时：基数照那一档放大。
pub(super) fn idle(base: Duration, level: Option<&str>) -> Duration {
    base.saturating_mul(effort::idle_factor(level))
}
