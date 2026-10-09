//! 远程的 embedding（施工 R-5 补，`docs/blueprint/recall.md` 第四条第 1 款）：`models.embedding` 写 `<供应商>/<模型>` 的，
//! 照那一家 OpenAI 兼容的 `/embeddings` 一次算一句。
//!
//! - 端点照 [`reach`]（和 `provider.test` 同一段）：照调的一方手里的配置拼地址、key、另配的头，不走路由、池、冷却。认证头
//!   照这一家的驱动写，另配的头照种子 [`PURPOSE`] 换；客户端照地址挑（落在本机的不走代理，[`ModelData::fetcher_for`]）。
//! - 发 `{"model", "input"}`，最多等 [`WAIT`]；读 `data[0].embedding`，归一化（各家不一定归一化过）。
//! - 供应商报了 `usage.prompt_tokens` 的照一次性调用记一笔 `usage.oneshot`（用途 [`PURPOSE`]，有价格的算金额），记在调的那个
//!   账号名下；回的向量用不了的也记（那一家照样收了钱），没报的、出错的不记。
//! - 每一次记一行 `INFO model call`（成了）或 `INFO model call failed`（没成，带原话），目标 `miyu::session`，和一次性入口那一行
//!   同一个写法；key、地址不进日志。

use std::sync::Arc;
use std::time::{Duration, Instant};

use miyu_config::secret::Reference;
use miyu_kernel::event::Usage;
use miyu_kernel::id::AccountId;
use miyu_models::facts::facts;
use miyu_models::price::Tariff;
use miyu_models::provider::Provider;
use miyu_store::usage::OneShotCall;

use crate::TARGET;
use crate::blocking::blocking;
use crate::clock::wall_now;
use crate::config::TurnConfig;
use crate::route::endpoint::reach;
use crate::route::lists::listing_texts;
use crate::route::shared::ModelData;

use super::Unavailable;

/// 一次最多等多久：从发出到读完。
const WAIT: Duration = Duration::from_secs(10);

/// 回应最多多少字节：几千维的向量写成 JSON 也就几十 KiB。
const LIMIT: usize = 4 * 1024 * 1024;

/// 用途：记进 `usage.oneshot`、日志，也是另配的头的种子。
const PURPOSE: &str = "embedding";

/// 远程那一家：照谁的配置、记在谁的账上、哪一家的哪个模型。
#[derive(Debug, Clone)]
pub(crate) struct Remote {
    /// 核心一份的模型资料：查供应商、挑客户端、记账。
    pub(crate) data: Arc<ModelData>,
    /// 调的一方手里的配置：她的工具是这一轮冻结的，协议是这时的。
    pub(crate) config: TurnConfig,
    /// 记在谁的账上。
    pub(crate) owner: AccountId,
    /// 供应商编号。
    pub(crate) provider: String,
    /// 模型名。
    pub(crate) model: String,
}

impl Remote {
    /// `text` 的向量（归一化过的），记一行日志；回了、报了用量的记一笔账（向量用不了的也记：那一家照样收了钱）。
    ///
    /// # Errors
    ///
    /// 这一家没配、地址或 key 取不到、没有客户端（[`Unavailable::Off`]）；发不出去、回的不是 2xx、超时、读不懂、向量是空的或者
    /// 全是零（[`Unavailable::Failed`]）。
    pub(crate) async fn embed(&self, text: &str) -> Result<Vec<f32>, Unavailable> {
        let started = Instant::now();
        let asked = self.ask(text).await;
        let took_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let (provider, model) = (self.provider.as_str(), self.model.as_str());
        match &asked {
            Ok((_, input)) => {
                tracing::info!(target: TARGET, purpose = PURPOSE, provider, model, input, took_ms, "model call");
            }
            Err(why) => {
                tracing::info!(target: TARGET, purpose = PURPOSE, provider, model, took_ms, error = %why, "model call failed");
            }
        }
        asked.map(|(vector, _)| vector)
    }

    /// 发一次：交回向量和报了的输入 token 数。
    async fn ask(&self, text: &str) -> Result<(Vec<f32>, Option<u64>), Unavailable> {
        let values = self.config.resolved.values();
        let secret = |reference: &Reference| self.config.secret(reference);
        let reached = self
            .data
            .with(|knowledge| reach(&values, knowledge, &self.provider, &secret))
            .map_err(|why| Unavailable::Off(why.0))?;
        let client = self
            .data
            .fetcher_for(&reached.base_url)
            .ok_or_else(|| Unavailable::Off("no client to send with".to_string()))?;
        let driver = reached
            .provider
            .build(listing_texts().map_err(Unavailable::Off)?);
        let mut headers = reached
            .key
            .as_ref()
            .map(|key| driver.auth(key.expose()))
            .unwrap_or_default();
        headers.extend(reached.provider.headers(PURPOSE));
        let url = format!("{}/embeddings", reached.base_url.trim_end_matches('/'));
        let body = serde_json::json!({ "model": self.model, "input": text }).to_string();
        let bytes = miyu_http::post(miyu_http::Post {
            client,
            url: &url,
            headers: &headers,
            body: body.as_bytes(),
            timeout: WAIT,
            limit: LIMIT,
        })
        .await
        .map_err(|failed| Unavailable::Failed(failed.message))?;
        // 只看要的两样：`data[0].embedding`、`usage.prompt_tokens`（没报的不记账）。
        let reply: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
            Unavailable::Failed(format!("embedding reply not readable: {error}"))
        })?;
        let input = reply["usage"]["prompt_tokens"].as_u64();
        if let Some(input) = input {
            self.record(input, &reached.provider).await;
        }
        let vector = reply["data"][0]["embedding"]
            .as_array()
            .and_then(|values| {
                values
                    .iter()
                    .map(|value| value.as_f64().map(|value| value as f32))
                    .collect::<Option<Vec<f32>>>()
            })
            .and_then(normalized)
            .ok_or_else(|| Unavailable::Failed("embedding reply has no vector".to_string()))?;
        Ok((vector, input))
    }

    /// 记账：照这一份配置里 `provider` 这个模型的价格算好金额，在阻塞线程里写进账号日志和用量汇总。没交用量汇总的（测试里）
    /// 不记；写不进去的记一行 `WARN usage not indexed`。
    async fn record(&self, input: u64, provider: &Provider) {
        let Some(ledger) = self.data.ledger() else {
            return;
        };
        let usage = Usage {
            uncached: input,
            cache_read: 0,
            cache_write: 0,
            output: 0,
            reasoning: None,
        };
        let tariff = self.data.with(|knowledge| {
            let (facts, _) = facts(&self.config.resolved, knowledge, provider, &self.model);
            Tariff::of(&facts, &|layer| self.config.file(layer))
        });
        let call = OneShotCall {
            purpose: PURPOSE.to_string(),
            endpoint: self.provider.clone(),
            model: self.model.clone(),
            usage: Some(usage),
            cost: tariff.and_then(|tariff| tariff.cost(&usage)),
        };
        let owner = self.owner.clone();
        blocking(move || {
            if let Err(error) = ledger.one_shot(&owner, wall_now(), &call) {
                tracing::warn!(target: TARGET, purpose = PURPOSE, error = %error, "usage not indexed");
            }
        })
        .await;
    }
}

/// 归一化：长度是零的、有不是数的没有。
fn normalized(mut vector: Vec<f32>) -> Option<Vec<f32>> {
    let length = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if vector.is_empty() || !length.is_finite() || length == 0.0 {
        return None;
    }
    for x in &mut vector {
        *x /= length;
    }
    Some(vector)
}
