//! 回合开始时重新解析（施工 8-10，`docs/blueprint/models.md`「怎么走」第六条第 3、4 条）：照这一轮冻结的配置解析会话的
//! 引用，端点、限额换成解析出的。
//!
//! - 内核交了引用的，换成它（变了的，钉着的成员清掉）；没交的（以前的日志、造的时候连 `models.chat` 都没配），照路由记在
//!   内存里的（「施工时定的」8-10）。
//! - 解析不出的（供应商、池删了，池空了），退回这一轮的 `models.chat`，以后钉在它上面，记一行 `INFO model fallback`；内核
//!   交了引用的，交回原来的和退回的，内核记一条 `session.policy_changed`。`models.chat` 也解析不出的不退，端口是「没有
//!   模型」，这一轮的请求当场 `no_model`。
//! - 钉住的池：钉着的成员还在池里的照旧，不在了、刚换过来的取指针指的（`route/pool.rs`）；会话换过去的 key 照旧
//!   （`route/choice.rs` 照名字认，不在了的照会话编号钉着的）。
//! - 限额照解析出的重算：配置里改了窗口、用出来的窗口，这一轮就用上。

use miyu_kernel::id::{ModelName, ProviderId};
use miyu_kernel::origin::Model;
use miyu_kernel::session::Replaced;
use miyu_models::pools::Member;
use miyu_models::provider;
use miyu_models::reference::{Resolved, resolve};

use super::{Route, nothing};
use crate::TARGET;
use crate::config::TurnConfig;

impl Route {
    /// 回合开始：照这一轮的配置 `config` 重新解析，`recorded` 是内核记着的引用（见模块的说明）。交回退回了默认的那一次。
    pub(super) fn begin(&self, config: &TurnConfig, recorded: Option<&str>) -> Option<Replaced> {
        let values = config.resolved.values();
        let data = &self.shared.data;
        let resolve = |text: &str| {
            data.with(|knowledge| resolve(&values, knowledge, text))
                .ok()
        };
        let mut pinned = self.lock();
        if let Some(recorded) = recorded
            && pinned.reference.as_deref() != Some(recorded)
        {
            pinned.reference = Some(recorded.to_string());
            pinned.member = None;
        }
        let mut replaced = None;
        let resolved = match pinned.reference.as_deref().and_then(resolve) {
            Some(resolved) => Some(resolved),
            None => {
                let chat = provider::chat(&values);
                let fallback = chat.as_deref().and_then(resolve);
                if let (Some(_), Some(to)) = (&fallback, chat) {
                    if let Some(from) = &pinned.reference {
                        tracing::info!(target: TARGET, from = from.as_str(), to = to.as_str(), "model fallback");
                    }
                    replaced = recorded.map(|from| Replaced {
                        from: from.to_string(),
                        to: to.clone(),
                    });
                    pinned.reference = Some(to);
                    pinned.member = None;
                }
                fallback
            }
        };
        let (member, limits) = match resolved {
            Some(Resolved::Model(target)) => (None, self.shared.limits(config, &target)),
            Some(Resolved::Pool(pool)) => {
                let held = pinned.member.as_ref().and_then(model_of);
                self.shared.pool_start(config, &pool, held.as_ref())
            }
            None => (None, nothing()),
        };
        pinned.member = member;
        pinned.last = limits.model.clone();
        pinned.limits = limits;
        replaced
    }
}

/// 池里一个成员写成端点和模型：钉住的池照它认回钉着的那一个（[`super::Routes::pool_start`]）。
fn model_of(member: &Member) -> Option<Model> {
    Some(Model {
        endpoint: ProviderId::parse(&member.provider).ok()?,
        model: ModelName::parse(&member.model).ok()?,
    })
}
