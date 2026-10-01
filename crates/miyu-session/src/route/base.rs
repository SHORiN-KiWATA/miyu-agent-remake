//! 模型调用口的底子（`docs/blueprint/models.md`「怎么走」第十二条第 1 条，施工 8-20 从会话的路由里拆出来）：会话入口
//! （`route.rs` 的 `Route`）和一次性入口（`route/once.rs` 的 [`super::OneShot`]）都只调它。冷却表、池的指针在核心一份的
//! 模型资料里（`route/shared.rs`），两个入口共用：会话撞了 429，同一时刻的一次性调用自动避开那个端点，反过来也一样。
//!
//! - 挑（[`Routes::pick`]）：照解析出的模型或 `@池` 排候选（`route/choice.rs`、`route/pool.rs`，第四条那张表），取排在最前、
//!   没在冷却的（`choice::pick`）。交回挑中的这一次（[`Picked`]，`route/ended.rs`）：发给谁、别的候选是谁，说完了照它记
//!   冷却、说换没换端点。
//! - 备好（[`Routes::ready`]）：照真发的那个模型查资料、挑思考强度（会话没记的照配置的默认，`route/effort.rs`）、挑客户端
//!   （地址落在本机的不走代理），造驱动和这一次的调用（[`Ready`]）。
//! - 发：`route/exchange.rs`。
//!
//! 底子不认会话，只认「谁在挑」（[`Seat`]）：会话交它自己的，一次性的种子是用途，别的都没有。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use miyu_config::Values;
use miyu_drivers::{Call, DriverTexts, OpenAiChat};
use miyu_http::{Client, Endpoint, is_loopback_url};
use miyu_kernel::id::ModelName;
use miyu_kernel::origin::Model;
use miyu_models::cooldown::Candidate;
use miyu_models::facts::{Facts, facts};
use miyu_models::pools::{Member, Strategy};
use miyu_models::provider::NoModel;
use miyu_models::reference::{Resolved, resolve};

use super::choice::{self, Choice, Unsent};
use super::ended::Picked;
use super::exchange::Learn;
use super::{Routes, effort, model_of};
use crate::clock::wall_now;
use crate::config::TurnConfig;

/// 谁在挑（施工 8-20）：底子只认这四样。
pub(super) struct Seat<'a> {
    /// key 照它钉：会话编号，或者一次性调用的用途（`miyu_models::keys::order`）。
    pub(super) seed: &'a str,
    /// 出错换过去、成了的 key：供应商的编号 → key 的名字，这一家先用它。一次性的是空的。
    pub(super) moved: &'a BTreeMap<String, String>,
    /// 钉住的池钉着的成员：从它起排。没有的（一次性的第一次、还没钉上的）取指针指的。
    pub(super) held: Option<&'a Member>,
    /// 说到一半断了的那一个：不挑，还发给它。只有会话的主请求有。
    pub(super) sticky: Option<&'a Candidate>,
}

/// 备好了的这一次：发给谁、怎么编码、空闲多久算断，报了上限记到哪。
pub(super) struct Ready {
    /// 照地址挑的客户端。
    pub(super) client: Client,
    /// 记进 `model.called` 的端点和模型。
    pub(super) model: Model,
    /// 地址和 key 的值。
    pub(super) endpoint: Endpoint,
    /// 驱动：开关照这一家的档案，占位照交进来的。
    pub(super) driver: OpenAiChat,
    /// 这一次的调用：模型名、输出上限、能收什么、思考强度。
    pub(super) call: Call,
    /// 空闲超时，照思考强度放大过。
    pub(super) idle: Duration,
    /// 报了上限时记到哪。
    pub(super) learn: Learn,
}

impl Routes {
    /// 引用 `text` 这一轮指到哪，照这一轮的配置 `values` 和手头的资料。
    pub(super) fn resolve(&self, values: &Values, text: &str) -> Result<Resolved, NoModel> {
        self.data.with(|knowledge| resolve(values, knowledge, text))
    }

    /// 照 `resolved` 排候选、挑一个（第四条）。交回挑中的这一次，和引用是不是钉住的池。
    pub(super) fn pick(
        &self,
        config: &TurnConfig,
        values: &Values,
        resolved: &Resolved,
        seat: &Seat<'_>,
    ) -> Result<(Picked, bool), Unsent> {
        let (mut choices, pins) = match resolved {
            Resolved::Model(target) => (self.choices(config, target, None, seat)?, false),
            Resolved::Pool(pool) => (
                self.members(config, values, pool, seat)?,
                pool.strategy == Strategy::Pin,
            ),
        };
        let at = choice::pick(&choices, seat.sticky, &self.data, wall_now())?;
        let choice = choices.remove(at);
        let picked = Picked {
            routes: self.clone(),
            choice,
            others: choices.into_iter().map(Choice::who).collect(),
        };
        Ok((picked, pins))
    }

    /// 挑定了 `choice`：照真发的那个模型查资料，思考强度照会话记的一格 `cell`（没有的照配置的默认），驱动的占位是
    /// `texts`，输出上限 `max_output`（没有的照供应商的默认）。
    pub(super) fn ready(
        &self,
        config: &TurnConfig,
        choice: &Choice,
        cell: Option<&str>,
        texts: DriverTexts,
        max_output: Option<u32>,
    ) -> Result<Ready, NoModel> {
        let target = &choice.target;
        let model = ModelName::parse(&target.model).map_err(|error| NoModel(error.to_string()))?;
        let (facts, _): (Facts, _) = self
            .data
            .with(|knowledge| facts(&config.resolved, knowledge, &target.provider, &target.model));
        let endpoint = choice.endpoint.clone();
        // 思考强度照真发的这个模型挑（施工 8-18）：会话记的不在档位里的记一行。
        let effort = effort::chosen(cell, &facts, target, true).map(|used| used.level);
        // 地址落在本机的不走代理（施工 8-11 补）：和探本机的服务、拉列表一样。
        let client = match is_loopback_url(&endpoint.base_url) {
            true => self.direct.clone(),
            false => self.client.clone(),
        };
        Ok(Ready {
            client,
            model: model_of(target),
            endpoint,
            driver: OpenAiChat::new(target.provider.compat.clone(), texts),
            idle: effort::idle(self.idle, effort.as_deref()),
            call: Call {
                model,
                max_output,
                inputs: facts.driver_inputs(),
                effort,
            },
            learn: Learn {
                data: Arc::clone(&self.data),
                provider: target.provider.id.clone(),
                model: target.model.clone(),
                window: facts.window.value,
            },
        })
    }
}
