//! 给头看的那一档思考强度（施工 8-18；8-18（补）去掉会话那一层，`docs/blueprint/models.md`「怎么走」第十一条第 4、6、7
//! 条）。
//!
//! - 每次请求挑好端点以后，照真发的那个模型配置的默认（`route/base.rs` 直接读 `facts.effort.value`）。
//! - 空闲超时照那一档放大（`miyu_models::effort::idle_factor`）。
//! - 给头看的那一档：限额里的那个模型（轮换的池没有），照端口记着的配置现算，连同从配置的哪一层来
//!   （`miyu_models::effort::in_use`）。

use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::event::EffortInUse;
use miyu_models::effort;
use miyu_models::facts::facts;
use miyu_models::provider::{self, Target};

use super::{NONE, Route};

impl Route {
    /// 给头看的那一档：限额里的那个模型照记着的配置算。轮换的池、还没有模型的、那一家这时用不了的没有。
    pub(super) fn shown_effort(&self) -> Option<EffortInUse> {
        let pinned = self.lock();
        let model = pinned.limits.model.clone();
        if model.endpoint.as_str() == NONE {
            return None;
        }
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
        effort::in_use(&facts.effort)
    }
}

/// 这一次的空闲超时：基数照那一档放大。
pub(super) fn idle(base: Duration, level: Option<&str>) -> Duration {
    base.saturating_mul(effort::idle_factor(level))
}
