//! 下架的模型（施工 8-23，`docs/construction/8-23-下架的模型移出池（要补）.md`）：池的成员回了 404，在后台当场再拉一次这一家的
//! 模型列表，拉成了、里面没有它，才算下架（两样都说没有才算，2026-10-08 项目主人定）。确认完了交给端口（[`Retirement`]，端点
//! 装上：下架了的从池里拿掉、写配置）。不挡这一次的出错收场；同一个模型同一时间只确认一次。
//!
//! - 只查池的成员：直接写在 `models.chat`、`models.vision` 的是人明着写的，撞上了照旧报错，人自己改。
//! - 判 404 照状态码，不照报错的原话：几家的原话各不一样。

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use crate::TARGET;
use crate::config::TurnConfig;

use super::ended::Picked;
use super::lists::refresh_list;

/// 模型不存在的状态码。
const NOT_FOUND: u16 = 404;

/// 确认完了交给谁（施工 8-23）：端点在核心起来时装上（[`super::ModelData::on_retirement`]），下架了的从池里拿掉、写配置。
pub trait Retirement: Send + Sync {
    /// 编号 `provider` 这一家的 `model` 确认完了：`gone` 是当场拉的列表里没有它；列表里还有它、拉不成的是假。在阻塞线程里调。
    fn concluded(&self, provider: &str, model: &str, gone: bool);
}

/// 装上的端口，和正在确认的几个（供应商、模型）。
#[derive(Default)]
pub(crate) struct Retiring {
    pub(super) port: Option<Arc<dyn Retirement>>,
    checking: BTreeSet<(String, String)>,
}

impl fmt::Debug for Retiring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Retiring")
            .field("port", &self.port.is_some())
            .field("checking", &self.checking)
            .finish()
    }
}

impl Picked {
    /// 出错收场时（施工 8-23）：状态码 `status` 是 404、发的是池的成员、端口装上了的，在后台确认它是不是下架了。
    pub(in crate::route) fn check_gone(&self, config: &TurnConfig, status: Option<u16>) {
        if status != Some(NOT_FOUND) || self.choice.member.is_none() {
            return;
        }
        let who = &self.choice.who;
        let key = (who.provider.clone(), who.model.clone());
        let port = {
            let mut retiring = self.routes.data.retiring();
            let Some(port) = retiring.port.clone() else {
                return;
            };
            if !retiring.checking.insert(key.clone()) {
                return;
            }
            port
        };
        let (data, config) = (Arc::clone(&self.routes.data), Arc::clone(config));
        tokio::spawn(async move {
            let (provider, model) = key.clone();
            let secret = |reference: &_| config.secret(reference);
            let listed = refresh_list(&data, &config.resolved.values(), &secret, &provider).await;
            let gone = listed.is_ok()
                && data.with(|knowledge| {
                    knowledge
                        .lists
                        .get(&provider)
                        .is_some_and(|list| !list.models.iter().any(|each| each.id == model))
                });
            match gone {
                true => {
                    tracing::info!(target: TARGET, provider = provider.as_str(), model = model.as_str(), "model gone")
                }
                false => {
                    tracing::debug!(target: TARGET, provider = provider.as_str(), model = model.as_str(), "model not gone")
                }
            }
            let told = tokio::task::spawn_blocking(move || port.concluded(&provider, &model, gone));
            if told.await.is_err() {
                tracing::warn!(target: TARGET, "retirement panicked");
            }
            data.retiring().checking.remove(&key);
        });
    }
}
