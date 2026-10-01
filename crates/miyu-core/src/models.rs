//! 模型（`docs/blueprint/models.md`、`core.md`「模型」，施工 8-6、8-7）：起来时读资源目录里的供应商档案
//! （`models/profiles.toml`）、认原厂的表（`models/vendors.toml`），造每个会话的路由（`miyu_session::Routes`）和核心一份的
//! 模型资料（`miyu_session::ModelData`）。写了 `ready` 以后在阻塞线程里读目录（[`catalog`]）、用出来的、供应商的列表，
//! 读完放行等着它的；再在后台更新目录（[`refresh`]）。用哪家供应商、哪个模型、哪个 key，全照配置。
//!
//! 档案、认原厂的表是 TOML，这里读成 JSON 交给 `miyu-models`（那一层只用白名单里的 `serde_json`）。

pub mod catalog;
pub mod refresh;

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::watch;

use miyu_config::secret::Reference;
use miyu_http::{Proxy, client, fetcher};
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_models::settings::CatalogSettings;
use miyu_session::{IDLE, ModelData, Models, Observed, Routes, read_observed};
use miyu_store::resources::ResourceRoot;

use crate::TARGET;
use catalog::Places;
use refresh::{Refresher, Schedule};

/// 照资源目录造路由，模型资料还没读（目录、用出来的、供应商的列表随 [`start`]）。用出来的、列表写进 `state`
/// （`state/models`，没有的不写）。
///
/// # Errors
///
/// 档案、认原厂的表读不出来、写法不对（安装坏了）；HTTP 客户端造不出来（系统的证书读不了之类）。交回原因。
pub fn prepare(resources: &ResourceRoot, state: Option<PathBuf>) -> Result<Routes, String> {
    let profiles = resources
        .profiles()
        .map_err(|error| error.to_string())
        .and_then(|text| profiles(&text))?;
    let vendors = resources
        .vendors()
        .map_err(|error| error.to_string())
        .and_then(|text| vendors(&text))?;
    tracing::info!(
        target: TARGET,
        profiles = profiles.providers.len(),
        "model profiles loaded"
    );
    let client = client(Proxy::FromEnvironment).map_err(|error| error.to_string())?;
    let lists = fetcher(Proxy::FromEnvironment).map_err(|error| error.to_string())?;
    Ok(Routes {
        client,
        data: Arc::new(ModelData::new(profiles, vendors, state).with_fetcher(lists)),
        idle: IDLE,
    })
}

/// 照资源目录造路由，当场读完快照（不看缓存、不后台更新、不写 `state`）：测试、进程内的头用。
///
/// # Errors
///
/// 同 [`prepare`]。
pub fn routes(resources: &ResourceRoot) -> Result<Arc<dyn Models>, String> {
    let routes = prepare(resources, None)?;
    let places = Places {
        snapshot: resources.path().join("models"),
        cache: None,
    };
    routes
        .data
        .loaded(catalog::load(&places), Observed::default());
    Ok(Arc::new(routes))
}

/// 写了 `ready` 以后：在阻塞线程里读目录（快照和缓存挑新的）、用出来的、供应商的列表，读完放行；再照 `settings` 在后台
/// 更新目录。缓存目录 `cache`（`<缓存目录>/models`）算不出来的只读快照、不拉。
pub fn start(
    data: Arc<ModelData>,
    snapshot: PathBuf,
    cache: Option<PathBuf>,
    state: Option<PathBuf>,
    settings: watch::Receiver<Schedule>,
) {
    let places = Places {
        snapshot,
        cache: cache.clone(),
    };
    tokio::spawn(async move {
        let reading = Arc::clone(&data);
        let read = tokio::task::spawn_blocking(move || {
            let observed = state.as_deref().map(read_observed).unwrap_or_default();
            reading.loaded(catalog::load(&places), observed);
        })
        .await;
        if let Err(error) = read {
            tracing::error!(target: TARGET, error = %error, "catalog read panicked");
            data.loaded(None, Observed::default());
        }
        // 缓存目录算不出来的不拉：算的时候已经记过一行 `WARN catalog cache unavailable`（[`cache`]）。
        let Some(cache) = cache else {
            return;
        };
        // 和拉供应商的列表用同一个 GET 的客户端（[`prepare`] 造的）。
        if let Some(client) = data.fetcher().cloned() {
            Refresher {
                data,
                client,
                cache,
            }
            .run(settings)
            .await;
        }
    });
}

/// 缓存目录里放目录的地方：`<缓存目录>/models`（`store.md` 第 3 条，整台机器共用）。算不出来的记一行，交回空的。
pub fn cache(env: &miyu_store::env::Env) -> Option<PathBuf> {
    match miyu_store::root::cache_root(env) {
        Ok(root) => Some(root.join("models")),
        Err(error) => {
            tracing::warn!(target: TARGET, reason = crate::sandbox::why(&error), "catalog cache unavailable");
            None
        }
    }
}

/// 配置里 `[models.catalog]` 那几项：配置换了当场跟着换（当场生效）。地址是环境变量的引用的照核心的环境取（施工 8-8）。
pub fn catalog_settings(
    mut config: watch::Receiver<Arc<miyu_endpoint::config::Config>>,
) -> watch::Receiver<Schedule> {
    let of = |config: &miyu_endpoint::config::Config| {
        let settings = CatalogSettings::from(&config.resolved().values());
        Schedule::of(settings, &|name| {
            let secret = config.secret(&Reference::Env(name.to_string()))?;
            Some(secret.expose().to_string())
        })
    };
    let (sender, receiver) = watch::channel(of(&config.borrow_and_update()));
    tokio::spawn(async move {
        while config.changed().await.is_ok() {
            let now = of(&config.borrow_and_update());
            sender.send_if_modified(|old| {
                let changed = *old != now;
                *old = now;
                changed
            });
        }
    });
    receiver
}

/// 认原厂的表：TOML 的字先变成 JSON，再照 `miyu-models` 的样子读。
///
/// # Errors
///
/// TOML 写法不对、形状不对：原因写明是这张表。
pub fn vendors(text: &str) -> Result<Vendors, String> {
    let document = toml_edit::Document::parse(text).map_err(|error| {
        format!(
            "models/vendors.toml not readable: {}",
            error.message().trim()
        )
    })?;
    Vendors::parse(&table(document.as_table())?)
}

/// 读档案：TOML 的字先变成 JSON，再照 `miyu-models` 的样子读。
///
/// # Errors
///
/// TOML 写法不对、有 JSON 写不下的值（日期时间）、形状不对：原因写明是档案。
pub fn profiles(text: &str) -> Result<Profiles, String> {
    let document = toml_edit::Document::parse(text).map_err(|error| {
        format!(
            "models/profiles.toml not readable: {}",
            error.message().trim()
        )
    })?;
    let json = table(document.as_table())?;
    Profiles::parse(&json)
}

/// 一张 TOML 的表（有表头的、行内的都行）写成 JSON 的对象。
fn table(table: &dyn toml_edit::TableLike) -> Result<serde_json::Value, String> {
    let mut object = serde_json::Map::new();
    for (key, node) in table.iter() {
        let value = match node {
            toml_edit::Item::Value(value) => value_json(value)?,
            toml_edit::Item::Table(inner) => self::table(inner)?,
            toml_edit::Item::ArrayOfTables(array) => serde_json::Value::Array(
                array
                    .iter()
                    .map(|inner| self::table(inner))
                    .collect::<Result<_, _>>()?,
            ),
            toml_edit::Item::None => continue,
        };
        object.insert(key.to_string(), value);
    }
    Ok(serde_json::Value::Object(object))
}

/// 一个 TOML 的值写成 JSON。
fn value_json(value: &toml_edit::Value) -> Result<serde_json::Value, String> {
    Ok(match value {
        toml_edit::Value::String(text) => serde_json::Value::from(text.value().as_str()),
        toml_edit::Value::Integer(number) => serde_json::Value::from(*number.value()),
        toml_edit::Value::Float(number) => serde_json::Value::from(*number.value()),
        toml_edit::Value::Boolean(on) => serde_json::Value::from(*on.value()),
        toml_edit::Value::Array(array) => {
            serde_json::Value::Array(array.iter().map(value_json).collect::<Result<_, _>>()?)
        }
        toml_edit::Value::InlineTable(inner) => table(inner)?,
        toml_edit::Value::Datetime(_) => {
            return Err("models/profiles.toml not readable: a date is not expected".to_string());
        }
    })
}

#[cfg(test)]
mod tests;
