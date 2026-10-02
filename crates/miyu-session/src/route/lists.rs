//! 拉供应商的模型列表（`docs/blueprint/models.md`「怎么走」第二条第 10 条，施工 8-7）：照驱动的 `models_path()` GET，带这
//! 家的第一个取得到值的 key，整个 30 秒；读出来的存 `state/models/providers/<编号>.json`。拉不到的记一行
//! `WARN provider list failed`，照旧用上一份。
//!
//! 谁来拉：`model.list` 带 `refresh` 的拉完再答，发现某家没有、旧过 24 小时的在后台拉（协议端点）。GET 一家、读出模型的那一段
//! （[`list_models`]）`provider.test` 列模型、`provider.detect` 探本机也用（施工 8-11，`route/probe.rs`、`route/local.rs`）。

use std::sync::Arc;
use std::time::Duration;

use miyu_config::Values;
use miyu_config::secret::{Reference, Secret};
use miyu_drivers::{Driver, DriverTextSources, DriverTexts, OpenAiChat};
use miyu_http::{Client, Failed, Get, Got, get_full};
use miyu_models::observed::{ListedModel, ProviderList};
use miyu_models::provider::{self, NoModel};

use crate::TARGET;
use crate::blocking::blocking;
use crate::clock::wall_now;
use crate::route::shared::ModelData;

/// 整个最多多久。
pub(crate) const TIMEOUT: Duration = Duration::from_secs(30);

/// 响应体最多多少字节：几千个模型的列表也就几 MB。
const LIMIT: usize = 16 * 1024 * 1024;

/// 列表旧过多久，`model.list` 在后台重拉。
pub const STALE: Duration = Duration::from_secs(24 * 60 * 60);

/// 拉编号 `id` 这一家的列表，照这一刻的最终值 `values`，key 照 `secret` 取。拉到了换上、写盘。
///
/// # Errors
///
/// 这一家用不了（原话照 [`NoModel`]）、key 一个都取不到、没有客户端、拉不到、读不了：英文的一句，已经记过一行 `WARN`。
pub async fn refresh_list(
    data: &Arc<ModelData>,
    values: &Values,
    secret: &(dyn Fn(&Reference) -> Option<Secret> + Sync),
    id: &str,
) -> Result<(), String> {
    let fetched = fetch(data, values, secret, id).await;
    match fetched {
        Ok(models) => {
            let list = ProviderList {
                fetched: wall_now(),
                models,
            };
            let (data, id) = (Arc::clone(data), id.to_string());
            blocking(move || data.set_list(&id, list)).await;
            Ok(())
        }
        Err(error) => {
            tracing::warn!(target: TARGET, provider = id, error = %error, "provider list failed");
            Err(error)
        }
    }
}

/// GET、读。
async fn fetch(
    data: &ModelData,
    values: &Values,
    secret: &(dyn Fn(&Reference) -> Option<Secret> + Sync),
    id: &str,
) -> Result<Vec<ListedModel>, String> {
    let provider = data
        .with(|knowledge| provider::provider(values, knowledge, id))
        .map_err(|NoModel(why)| why)?;
    let driver = match provider.driver {
        provider::Driver::OpenAiChat => OpenAiChat::new(provider.compat.clone(), listing_texts()?),
    };
    let headers = if provider.keys.is_empty() {
        Vec::new()
    } else {
        let key = provider
            .keys
            .iter()
            .find_map(secret)
            .ok_or_else(|| format!("provider {id:?} has no usable key"))?;
        driver.auth(key.expose())
    };
    // 地址也可能是环境变量的引用（施工 8-6b），照同一个 `secret` 取。
    let base_url = provider::resolve_base_url(&provider, secret).map_err(|NoModel(why)| why)?;
    // 地址落在本机的不走代理，和探本机的服务一样（施工 8-11 补）。
    let client = data
        .fetcher_for(&base_url)
        .ok_or("no client to fetch with")?;
    list_models(client, &driver, &base_url, &headers, TIMEOUT)
        .await
        .map_err(|failed| failed.message)
}

/// GET 地址 `base_url` 后面接驱动的 `models_path()`，带 `headers`（认证头），整个最多 `timeout`，读成模型列表。
///
/// # Errors
///
/// 拿不到（原样的 [`Failed`]）；回的读不出：只有原话的一份。
pub(crate) async fn list_models(
    client: &Client,
    driver: &dyn Driver,
    base_url: &str,
    headers: &[(String, String)],
    timeout: Duration,
) -> Result<Vec<ListedModel>, Failed> {
    let said = |message: String| Failed {
        message,
        status: None,
        headers: Vec::new(),
        body: Vec::new(),
    };
    let url = format!("{}{}", base_url.trim_end_matches('/'), driver.models_path());
    let got = get_full(Get {
        client,
        url: &url,
        headers,
        etag: None,
        timeout,
        limit: LIMIT,
    })
    .await?;
    let Got::Body { bytes, .. } = got else {
        return Err(said("not modified without asking".to_string()));
    };
    let listed = driver.parse_models(&bytes).map_err(said)?;
    Ok(listed
        .into_iter()
        .map(|listed| ListedModel {
            id: listed.id,
            window: listed.window,
        })
        .collect())
}

/// 列模型、写认证头用不着占位的字（只有编码用它）：几句都是空的。`provider.test` 发的那一句也用不着（没有图、文件）。
pub(crate) fn listing_texts() -> Result<DriverTexts, String> {
    DriverTexts::new(DriverTextSources {
        image_omitted: "",
        file_omitted: "",
        no_output: "",
        tool_attachments: "",
        tool_attachments_only: "",
        text_file: None,
        image_name: None,
        image_description: None,
    })
    .map_err(|error| error.to_string())
}
