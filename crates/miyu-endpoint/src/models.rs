//! 协议上的 `model.list`（`docs/blueprint/models.md`「协议」，施工 8-7）：配好的供应商，每家的 key、对上了目录里的哪一家、
//! 模型，每个模型每一格资料的值和来源、状态；在用的目录。池、挡位、用途的 `vision`（施工 8-8）：池写的成员和怎么分，四个挡位、
//! 两种用途各配的引用，没配的是 `null`。模型、key 的冷却（施工 8-9）照核心一份的冷却表，照这一刻说。`session.create` 的 `model` 怎么解析也在这里（[`record`]，施工 8-8）。
//! `session.configure` 的参数（[`ConfigureParams`]）、`subscribe` 回应的 `model`（[`next`]）也在这里（施工 8-10）。
//!
//! 1. 先等目录读完（核心写了 `ready` 以后才读）。
//! 2. `provider` 写了、不是配好了的：`unknown_provider`。
//! 3. `refresh` 是真的：先拉一遍这几家的模型列表（`miyu_session::refresh_list`），拉完再答，拉不到的照旧用上一份。不是的：
//!    没有列表、旧过 24 小时的几家在后台拉，这一次先照手头的答。
//! 4. 照不算项目配置的最终值答。key 的值从不交出去，只说有没有值。

mod entry;

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::secret::{Reference, Secret};
use miyu_config::{Layer, Values};
use miyu_models::pools;
use miyu_models::provider::{self, NoModel};
use miyu_models::settings::{ProviderSettings, TierSettings, UseSettings};
use miyu_session::{ModelData, Next, STALE, refresh_list};

use crate::Core;
use crate::refusal::Refusal;

/// `model.list` 的参数：都可以不写，写 `null` 等于没写。
#[derive(Debug, Default, Deserialize)]
struct ListParams {
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    refresh: Option<bool>,
}

/// 答 `model.list` 要的那一刻的配置：最终值、各层的文件、引用到的 key 有没有值（拿着配置服务的锁抄一份，抄完就放开）。
pub(crate) struct Snapshot {
    pub(crate) resolved: miyu_config::merge::Resolved,
    pub(crate) files: Vec<(Layer, String)>,
    pub(crate) secrets: Vec<(Reference, Option<Secret>)>,
}

impl Snapshot {
    /// 这一层的文件写成什么（数据根里的相对路径）。
    pub(crate) fn file(&self, layer: Layer) -> String {
        self.files
            .iter()
            .find(|(at, _)| *at == layer)
            .map_or_else(String::new, |(_, file)| file.clone())
    }

    /// 引用 `reference` 的值。
    pub(crate) fn secret(&self, reference: &Reference) -> Option<Secret> {
        self.secrets
            .iter()
            .find(|(at, _)| at == reference)
            .and_then(|(_, secret)| secret.clone())
    }
}

/// `model.list`。
pub(crate) async fn list(core: &Core, params: Value) -> Result<Value, Refusal> {
    let params: ListParams = match params {
        Value::Null => ListParams::default(),
        params => serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?,
    };
    let data = Arc::clone(&core.model_data);
    data.wait().await;
    let snapshot = snapshot(core);
    let values = snapshot.resolved.values();
    let configured = miyu_config::key::names(values.keys(), "providers.<id>", &[]);
    let chosen: Vec<String> = match &params.provider {
        Some(id) if !configured.contains(id) => return Err(Refusal::UNKNOWN_PROVIDER),
        Some(id) => vec![id.clone()],
        None => configured,
    };
    let secret = |reference: &Reference| snapshot.secret(reference);
    if params.refresh == Some(true) {
        for id in &chosen {
            // 拉不到的已经记过一行 `WARN`，照旧用上一份。
            refresh_list(&data, &values, &secret, id)
                .await
                .unwrap_or(());
        }
    } else {
        refresh_stale(&data, &snapshot, &chosen);
    }
    let now = crate::sessions::now();
    let providers: Vec<Value> = chosen
        .iter()
        .map(|id| entry::provider(&data, &snapshot, id, now))
        .collect();
    let catalog = data.catalog().map_or(
        Value::Null,
        |loaded| json!({"source": loaded.source.as_str(), "fetched": loaded.fetched}),
    );
    let uses = UseSettings::from(&values);
    let tiers = TierSettings::from(&values);
    Ok(json!({
        "providers": providers,
        "pools": pools_json(&values),
        "tiers": {
            "lite": tiers.lite,
            "cheap": tiers.cheap,
            "standard": tiers.standard,
            "flagship": tiers.flagship,
        },
        "uses": {"chat": uses.chat, "vision": uses.vision},
        "catalog": catalog,
    }))
}

/// 每个池：名字、怎么分（没写的照成员定）、写的成员（照写的原样）。照名字排。
fn pools_json(values: &Values) -> Vec<Value> {
    pools::names(values)
        .into_iter()
        .filter_map(|name| {
            let (models, strategy) = pools::listed(values, &name)?;
            Some(json!({"name": name, "strategy": strategy.as_str(), "models": models}))
        })
        .collect()
}

/// `session.configure` 的参数（施工 8-10，`docs/blueprint/models.md`「协议」）：哪个会话、换成的模型、`@池` 或者挡位，两格都
/// 必写，不是字的读不成（`bad_params`）。
#[derive(Debug, Deserialize)]
pub(crate) struct ConfigureParams {
    /// 哪个会话。
    pub(crate) session: String,
    /// 换成的引用，还没解析。
    model: String,
}

impl ConfigureParams {
    /// 换成的引用，原样：空字是参数不对（「施工时定的」8-10）。
    ///
    /// # Errors
    ///
    /// 空字：`bad_params`。
    pub(crate) fn model(&self) -> Result<&str, Refusal> {
        match self.model.is_empty() {
            true => Err(Refusal::BAD_PARAMS),
            false => Ok(&self.model),
        }
    }
}

/// `subscribe` 回应的 `model`（施工 8-10）：`{"ref":…,"endpoint":…,"model":…}`，会话接下来请求的；轮换的池没有 `endpoint`、
/// `model`，一个模型都没有的没有这一格。
pub(crate) fn next(next: &Next) -> Option<Value> {
    if next.is_empty() {
        return None;
    }
    let mut written = serde_json::Map::new();
    if let Some(reference) = &next.reference {
        written.insert("ref".to_string(), json!(reference));
    }
    if let Some(model) = &next.model {
        written.insert("endpoint".to_string(), json!(model.endpoint.as_str()));
        written.insert("model".to_string(), json!(model.model.as_str()));
    }
    Some(Value::Object(written))
}

/// `session.create` 的 `model`（施工 8-8）：照这时的配置解析成会话记下的引用（模型或 `@池`，挡位换成它这时的值）。照不算项目
/// 配置的最终值：模型这一块项目配置里本来就不能写。
///
/// # Errors
///
/// 解析不出：没有这家供应商、没有这个池、池是空的、挡位没配又没有 `models.chat`（`unknown_model`，原话记一行 `DEBUG`）。
pub(crate) fn record(core: &Core, text: &str) -> Result<String, Refusal> {
    let values = core.config().resolved().values();
    miyu_models::reference::record(&values, text).map_err(|NoModel(why)| {
        tracing::debug!(target: "miyu::endpoint", why = %why, "unknown model");
        Refusal::UNKNOWN_MODEL
    })
}

/// 抄一份这一刻的配置：每个用得到的引用（key，和地址是环境变量的引用时，施工 8-6b）都先取好值，拉列表、`model.list`
/// 用的是同一份，不会各自再问一次核心的环境。`provider.detect`、`provider.test` 也用（施工 8-11）。
pub(crate) fn snapshot(core: &Core) -> Snapshot {
    let config = core.config();
    let resolved = config.resolved().clone();
    let values = resolved.values();
    let files = [Layer::System, Layer::Personal]
        .into_iter()
        .map(|layer| (layer, config.file(layer).shown.clone()))
        .collect();
    let secrets = miyu_config::key::names(values.keys(), "providers.<id>", &[])
        .iter()
        .flat_map(|id| {
            let settings = ProviderSettings::at(&values, &[id]);
            let base_url = match settings.base_url {
                Some(miyu_config::Address::Env(name)) => Some(Reference::Env(name)),
                _ => None,
            };
            settings.keys.into_iter().chain(base_url)
        })
        .map(|reference| {
            let secret = config.secret(&reference);
            (reference, secret)
        })
        .collect();
    Snapshot {
        resolved,
        files,
        secrets,
    }
}

/// 没有列表、旧过 24 小时的几家：在后台拉，这一次不等。这一家自己用不了的不拉（拉了也是白拉，每问一次记一行）。
fn refresh_stale(data: &Arc<ModelData>, snapshot: &Snapshot, chosen: &[String]) {
    let values = snapshot.resolved.values();
    let now = miyu_kernel::time::Timestamp::from_unix_millis(now_millis());
    for id in chosen {
        let fresh = data
            .list_fetched(id)
            .zip(now)
            .is_some_and(|(fetched, now)| {
                let age = now.unix_millis().saturating_sub(fetched.unix_millis());
                u128::try_from(age).is_ok_and(|age| age < STALE.as_millis())
            });
        let usable = data.with(|knowledge| provider::provider(&values, knowledge, id).is_ok());
        if fresh || !usable {
            continue;
        }
        let (data, id) = (Arc::clone(data), id.clone());
        let values = values.clone();
        let secrets = snapshot.secrets.clone();
        tokio::spawn(async move {
            let secret = |reference: &Reference| {
                secrets
                    .iter()
                    .find(|(at, _)| at == reference)
                    .and_then(|(_, secret)| secret.clone())
            };
            // 拉不到的已经记过一行 `WARN`。
            refresh_list(&data, &values, &secret, &id)
                .await
                .unwrap_or(());
        });
    }
}

/// 此刻，Unix 毫秒。
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
        })
}
