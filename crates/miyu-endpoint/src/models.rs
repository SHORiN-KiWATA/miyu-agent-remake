//! 协议上的 `model.list`（`docs/blueprint/models.md`「协议」，施工 8-7）：配好的供应商，每家的 key、对上了目录里的哪一家、
//! 模型，每个模型每一格资料的值和来源、状态；在用的目录。池、挡位随 8-8，冷却随 8-9。
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

use miyu_config::Layer;
use miyu_config::secret::{Reference, Secret};
use miyu_models::provider;
use miyu_models::settings::ProviderSettings;
use miyu_session::{ModelData, STALE, refresh_list};

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
    let providers: Vec<Value> = chosen
        .iter()
        .map(|id| entry::provider(&data, &snapshot, id))
        .collect();
    let catalog = data.catalog().map_or(
        Value::Null,
        |loaded| json!({"source": loaded.source.as_str(), "fetched": loaded.fetched}),
    );
    Ok(json!({
        "providers": providers,
        "uses": {"chat": provider::chat(&values)},
        "catalog": catalog,
    }))
}

/// 抄一份这一刻的配置。
fn snapshot(core: &Core) -> Snapshot {
    let config = core.config();
    let resolved = config.resolved().clone();
    let values = resolved.values();
    let files = [Layer::System, Layer::Personal]
        .into_iter()
        .map(|layer| (layer, config.file(layer).shown.clone()))
        .collect();
    let secrets = miyu_config::key::names(values.keys(), "providers.<id>", &[])
        .iter()
        .flat_map(|id| ProviderSettings::at(&values, &[id]).keys)
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
