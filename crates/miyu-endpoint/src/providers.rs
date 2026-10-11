//! 第一次接入的三个方法（`docs/blueprint/models.md`「协议」、「怎么走」第七条，施工 8-11）：`provider.detect` 找现成的
//! key 和本机的模型服务，`provider.catalog` 搜目录，`provider.test` 试一家（`providers/trial.rs`）。
//!
//! - 都先等目录读完（核心写了 `ready` 以后才读）。
//! - `provider.detect` 照核心的环境（配置服务手里的那一份，测试换成手写的）查变量有没有设，值不看、不交；本机的服务交给
//!   会话那一层一起探（`miyu_session::find_local`），探之前放开配置服务的锁。
//! - 已经配好的：`key` 引用了这个变量的（施工 8-25：一家一个 key）、推出来的地址和本机服务的一样的，写上那一家的编号（编号照字节排第一的）。

mod trial;

pub(crate) use trial::test;

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::Address;
use miyu_config::secret::Reference;
use miyu_models::onboard::{self, Listed};
use miyu_models::provider::{self, configured};
use miyu_models::settings::ProviderSettings;
use miyu_session::find_local;

use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// `provider.catalog` 不写 `limit` 时最多几家。
const LIMIT: u64 = 50;

/// `provider.catalog` 的参数：都可以不写，写 `null` 等于没写。
#[derive(Debug, Deserialize)]
struct CatalogParams {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    limit: Option<u64>,
    /// 只要常用的几家（施工 8-11 再补）：照资源目录 `models/featured.toml` 的先后，不看 `query`、`limit`。
    #[serde(default)]
    featured: Option<bool>,
}

/// `provider.detect`：没有参数，写了的不看（参数是不是对象，读请求时已经查过）。
pub(crate) async fn detect(core: &Core) -> Result<Value, Refusal> {
    let data = Arc::clone(&core.model_data);
    data.wait().await;
    let listed = data.with(onboard::listed);
    let values = core.config().resolved().values();
    let (by_env, by_url) = configured_by(&data, &values);
    let keys: Vec<Value> = {
        let config = core.config();
        onboard::key_vars(&listed)
            .into_iter()
            .filter(|(env, _)| config.env_set(env))
            .map(|(env, entry)| {
                let mut key = json!({
                    "env": env,
                    "provider": entry.id,
                    "name": entry.name,
                    "driver": entry.driver,
                    "supported": entry.supported,
                });
                if let Some(id) = by_env.get(&env) {
                    key["configured"] = json!(id);
                }
                key
            })
            .collect()
    };
    let services: Vec<Listed> = onboard::local_services(&listed)
        .into_iter()
        .cloned()
        .collect();
    let local: Vec<Value> = find_local(&data, services)
        .await
        .into_iter()
        .map(|running| {
            let base_url = running.listed.base_url.clone().unwrap_or_default();
            let mut service = json!({
                "provider": running.listed.id,
                "name": running.listed.name,
                "base_url": base_url,
                "models": running.models,
            });
            if let Some(id) = by_url.get(trimmed(&base_url)) {
                service["configured"] = json!(id);
            }
            service
        })
        .collect();
    Ok(json!({
        "keys": keys,
        "local": local,
        "looked_for": onboard::looked_for(&listed),
    }))
}

/// `provider.catalog`：编号、名字里有 `query` 的，能用的在前，最多 `limit` 家。写了 `featured` 的交回常用的几家（施工
/// 8-11 再补，[`featured`]）。
pub(crate) async fn catalog(core: &Core, peer: Peer, params: Value) -> Result<Value, Refusal> {
    let params: CatalogParams = serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?;
    if params.featured == Some(true) {
        return featured(core, peer.language).await;
    }
    let limit = match params.limit.unwrap_or(LIMIT) {
        0 => return Err(Refusal::BAD_PARAMS),
        limit => usize::try_from(limit).unwrap_or(usize::MAX),
    };
    let data = Arc::clone(&core.model_data);
    data.wait().await;
    let listed = data.with(onboard::listed);
    let providers: Vec<Value> = onboard::search(&listed, params.query.as_deref(), limit)
        .into_iter()
        .map(|entry| with_logo(&data, entry))
        .collect();
    Ok(json!({ "providers": providers }))
}

/// 目录里的一家，多一格图标（施工 8-31）。
fn with_logo(data: &miyu_session::ModelData, entry: &Listed) -> Value {
    let mut item = entry.json();
    item["logo"] = logo(data, Some(&entry.id));
    item
}

/// 目录里编号 `id` 那一家的图标（施工 8-31，`models.md`「图标」）：`{"svg", "tint"}`，没有的、不知道是哪一家的是 null。
pub(crate) fn logo(data: &miyu_session::ModelData, id: Option<&str>) -> Value {
    match id.and_then(|id| data.logo(id)) {
        Some(logo) => json!({"svg": logo.svg, "tint": logo.tint}),
        None => Value::Null,
    }
}

/// 常用的几家（施工 8-11 再补，`models.md`「协议」）：照 `featured.toml` 的先后，目录、档案里有的才交，写法同一家目录，
/// `name` 换成它写的（照连接的语言 `language` 挑），编号照语言挑（中文的有国内的用国内的）。资源读不了、写错了的是内部出错，
/// 记一行运行日志。
async fn featured(core: &Core, language: &str) -> Result<Value, Refusal> {
    let read = core
        .resources
        .featured_providers()
        .map_err(|error| error.to_string())
        .and_then(|text| onboard::featured(&text));
    let wanted = read.map_err(|error| {
        tracing::warn!(target: "miyu::endpoint", error = error.as_str(), "featured providers unreadable");
        Refusal::INTERNAL
    })?;
    let data = Arc::clone(&core.model_data);
    data.wait().await;
    let listed = data.with(onboard::listed);
    let providers: Vec<Value> = wanted
        .iter()
        .filter_map(|one| {
            let id = one.id(language);
            let found = listed.iter().find(|entry| entry.id == id)?;
            let mut item = with_logo(&data, found);
            item["name"] = json!(one.name.pick(language).unwrap_or(&found.name));
            Some(item)
        })
        .collect();
    Ok(json!({ "providers": providers }))
}

/// 已经配好的几家：`key` 引用的环境变量 → 编号（施工 8-25：一家一个 key），推出来的地址（去掉末尾的 `/`）→ 编号；照编号排，先占的算。
fn configured_by(
    data: &miyu_session::ModelData,
    values: &miyu_config::Values,
) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    let (mut by_env, mut by_url) = (BTreeMap::new(), BTreeMap::new());
    for id in configured(values) {
        if let Some(Reference::Env(name)) = ProviderSettings::at(values, &[&id]).key {
            by_env.entry(name).or_insert_with(|| id.clone());
        }
        let found = data.with(|knowledge| provider::provider(values, knowledge, &id));
        if let Ok(Address::Literal(url)) = found.map(|provider| provider.base_url) {
            by_url
                .entry(trimmed(&url).to_string())
                .or_insert_with(|| id.clone());
        }
    }
    (by_env, by_url)
}

/// 地址去掉末尾的 `/` 再比。
fn trimmed(url: &str) -> &str {
    url.trim_end_matches('/')
}
