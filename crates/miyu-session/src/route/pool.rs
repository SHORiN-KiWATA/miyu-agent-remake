//! 池里挑哪一个成员（`docs/blueprint/models.md`「怎么走」第三条第 6、7 条、第四条那张表，施工 8-8）。
//!
//! - 钉住：造端口时钉上一个成员：载入的、最近一条发出去了的 `model.called` 是这个池的成员的，就是它；不是的、新造的，取
//!   指针指的那个，指针加一。以后每次请求先发给它。
//! - 轮换：每次请求从指针指的那个成员起，指针加一。
//! - 候选：从挑中的那个起，照写的先后绕一圈。这时用不了的（那一家推不出驱动、地址，地址、key 取不到）跳过、取下一个；钉住的
//!   池，钉着的换成真发的那一个（8-8 只因为「这时用不了」换，出错才换随 8-9）。都用不了的，交第一个的原话。
//! - 认不出的成员（那一家没配）每次解析都记一行 `WARN pool member skipped`。
//! - 指针往前走一次，在阻塞线程里写一次 `state/models/pools.json`（[`ModelData::save_pointers`]）。
//! - 限额：钉住的照钉着的那个成员。轮换的取成员里窗口最小的、最大输出最小的（说得出的里面），一张图的算法只在成员都一样时
//!   给，模型写 `none`：每次请求的模型都不一样，锚总是对不上，用量全靠本地估（第三条第 7 条，认了的）。

use std::sync::Arc;

use miyu_config::Values;
use miyu_http::Endpoint;
use miyu_kernel::origin::Model;
use miyu_kernel::session::Limits;
use miyu_models::facts::facts;
use miyu_models::pools::{Member, Pool, Strategy};
use miyu_models::provider::{self, NoModel, Target};

use super::{Pinned, Route, Routes, images, none, nothing};
use crate::TARGET;
use crate::config::TurnConfig;
use crate::route::shared::ModelData;

impl Routes {
    /// 造端口时：钉住的池钉上哪个成员、限额是什么（见模块的说明）。`sent` 是最近一条发出去了的 `model.called` 发给了谁。
    pub(super) fn pool_start(
        &self,
        config: &TurnConfig,
        pool: &Pool,
        sent: Option<&Model>,
    ) -> (Option<Member>, Limits) {
        let values = config.resolved.values();
        match pool.strategy {
            Strategy::Pin => {
                let at = sent
                    .and_then(|sent| pool.find(sent.endpoint.as_str(), sent.model.as_str()))
                    .unwrap_or_else(|| take(&self.data, pool));
                let member = pool.members[at].clone();
                let limits = target(&self.data, &values, &member)
                    .map_or_else(|_| nothing(), |target| self.limits(config, &target));
                (Some(member), limits)
            }
            Strategy::Rotate => (None, self.rotating(config, &values, pool)),
        }
    }

    /// 轮换的池的限额：说得出的窗口、最大输出里小的；一张图的算法成员都一样才给。
    fn rotating(&self, config: &TurnConfig, values: &Values, pool: &Pool) -> Limits {
        let mut limits = nothing();
        let mut tokens = Vec::new();
        for member in &pool.members {
            let Ok(target) = target(&self.data, values, member) else {
                continue;
            };
            let (facts, _) = self.data.with(|knowledge| {
                facts(&config.resolved, knowledge, &target.provider, &target.model)
            });
            limits.window = smaller(limits.window, facts.window.value);
            limits.max_output = smaller(limits.max_output, facts.max_output.value);
            tokens.push(target.provider.images);
        }
        if let Some(first) = tokens.first()
            && tokens.iter().all(|each| each == first)
        {
            limits.images = images(*first);
        }
        limits.model = none();
        limits
    }
}

impl Route {
    /// 池这一次发给哪个成员：从钉着的（钉住）、指针指的（轮换，或者还没钉上的）起，照写的先后取第一个这时用得了的。
    pub(super) fn member(
        &self,
        config: &TurnConfig,
        values: &Values,
        pool: &Pool,
        pinned: &mut Pinned,
    ) -> Result<(Target, Endpoint), NoModel> {
        let data = &self.shared.data;
        for text in &pool.skipped {
            tracing::warn!(target: TARGET, pool = pool.name.as_str(), member = text.as_str(), "pool member skipped");
        }
        let held = pinned
            .member
            .as_ref()
            .filter(|_| pool.strategy == Strategy::Pin)
            .and_then(|member| pool.find(&member.provider, &member.model));
        let first = held.unwrap_or_else(|| take(data, pool));
        let mut problem = None;
        for at in pool.order(first) {
            let member = &pool.members[at];
            let tried = target(data, values, member)
                .and_then(|target| Ok((self.endpoint(config, &target)?, target)));
            match tried {
                Ok((endpoint, target)) => {
                    if pool.strategy == Strategy::Pin {
                        pinned.member = Some(member.clone());
                    }
                    return Ok((target, endpoint));
                }
                Err(error) => {
                    problem.get_or_insert(error);
                }
            }
        }
        Err(problem.unwrap_or_else(|| NoModel(format!("pool {:?} has no models", pool.name))))
    }
}

/// 池里一个成员这一轮发给谁：那一家这一轮的样子、模型名。
fn target(data: &ModelData, values: &Values, member: &Member) -> Result<Target, NoModel> {
    let provider =
        data.with(|knowledge| provider::provider(values, knowledge, &member.provider))?;
    Ok(Target {
        provider,
        model: member.model.clone(),
    })
}

/// 取指针指的那个成员，指针往前走一个，在阻塞线程里写盘。
fn take(data: &Arc<ModelData>, pool: &Pool) -> usize {
    let at = data.take(&pool.name, pool.members.len());
    let saving = Arc::clone(data);
    drop(tokio::task::spawn_blocking(move || saving.save_pointers()));
    at
}

/// 两个上限里小的；没有的不算。
fn smaller(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}
