//! 模型（`docs/blueprint/models.md`、`core.md`「模型」，施工 8-6）：起来时读资源目录里的供应商档案
//! （`models/profiles.toml`）和模型资料（`models/models-dev.json`），造每个会话的路由（`miyu_session::Routes`）。用哪家
//! 供应商、哪个模型、哪个 key，全照配置（`[providers.*]`、`models.chat`），每一轮开始时冻结的那一份；核心不再读开发用的
//! 环境变量。没配的，每次请求都当场说完：`no_model`。
//!
//! 档案是 TOML，这里读成 JSON 交给 `miyu-models`（那一层只用白名单里的 `serde_json`）。

use std::sync::Arc;

use miyu_http::{Proxy, client};
use miyu_models::ModelTable;
use miyu_models::profile::Profiles;
use miyu_session::{IDLE, Models, Routes};
use miyu_store::resources::ResourceRoot;

use crate::TARGET;

/// 照资源目录造给会话请求模型的路由。
///
/// # Errors
///
/// 档案、模型资料读不出来、写法不对（安装坏了）；HTTP 客户端造不出来（系统的证书读不了之类）。交回原因。
pub fn routes(resources: &ResourceRoot) -> Result<Arc<dyn Models>, String> {
    let table = resources
        .models()
        .map_err(|error| error.to_string())
        .and_then(|text| ModelTable::parse(&text))?;
    let profiles = resources
        .profiles()
        .map_err(|error| error.to_string())
        .and_then(|text| profiles(&text))?;
    tracing::info!(
        target: TARGET,
        profiles = profiles.providers.len(),
        "model profiles loaded"
    );
    let client = client(Proxy::FromEnvironment).map_err(|error| error.to_string())?;
    Ok(Arc::new(Routes {
        client,
        profiles: Arc::new(profiles),
        table: Arc::new(table),
        idle: IDLE,
    }))
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
