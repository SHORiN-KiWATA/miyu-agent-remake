//! 一次 GET（`docs/blueprint/models.md`「在哪」、`http.md`「一次 GET」，施工 8-7）：拉 models.dev 的目录、拉供应商的模型
//! 列表。和发请求（[`crate::send()`]）分开：不是流，整个读完再交回，有总时限、有大小上限，可以带上次的 `ETag` 问「变了
//! 没有」。
//!
//! 出错的原话是英文的一句，进运行日志：不带地址（路径、参数里可能有 key），reqwest 的错先去掉地址再接起来。

use std::error::Error;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::{ETAG, HeaderMap, HeaderName, HeaderValue, IF_NONE_MATCH};
use tokio::time::timeout;

/// 一次 GET 要的。
#[derive(Clone, Copy)]
pub struct Get<'a> {
    /// HTTP 客户端：连接的时限在它上面（[`crate::fetcher`]）。
    pub client: &'a reqwest::Client,
    /// 完整的地址。
    pub url: &'a str,
    /// 另带的头，照先后：认证头这些。值写不进头的，这一次出错。
    pub headers: &'a [(String, String)],
    /// 上次的 `ETag`：带上了，没变的回 304。
    pub etag: Option<&'a str>,
    /// 整个最多多久：从发出到读完。
    pub timeout: Duration,
    /// 响应体最多多少字节。
    pub limit: usize,
}

/// 拿到了什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Got {
    /// 2xx：响应体，和它的 `ETag`（没有的没有）。
    Body {
        /// 响应体。
        bytes: Vec<u8>,
        /// 回的 `ETag`。
        etag: Option<String>,
    },
    /// 304：和上次的一样。
    NotModified,
}

/// GET 一次。
///
/// # Errors
///
/// 头写不进去、连不上、超时、回的不是 2xx 也不是 304、响应体超过上限、读到一半断了：英文的一句原话。
pub async fn get(get: Get<'_>) -> Result<Got, String> {
    let mut headers = HeaderMap::new();
    for (name, value) in get.headers {
        let header = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| format!("header {name} is not valid"))?;
        // 值可能是密钥，不进原话。
        let value = HeaderValue::from_str(value)
            .map_err(|_| format!("value of header {name} is not valid"))?;
        headers.append(header, value);
    }
    if let Some(etag) = get.etag
        && let Ok(etag) = HeaderValue::from_str(etag)
    {
        headers.insert(IF_NONE_MATCH, etag);
    }
    match timeout(get.timeout, exchange(get, headers)).await {
        Ok(got) => got,
        Err(_) => Err(format!(
            "timed out after {} seconds",
            get.timeout.as_secs_f64()
        )),
    }
}

/// 发、读完。
async fn exchange(get: Get<'_>, headers: HeaderMap) -> Result<Got, String> {
    let mut response = get
        .client
        .get(get.url)
        .headers(headers)
        .send()
        .await
        .map_err(|error| chain(&error.without_url()))?;
    let status = response.status();
    if status == StatusCode::NOT_MODIFIED {
        return Ok(Got::NotModified);
    }
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }
    let etag = response
        .headers()
        .get(ETAG)
        .and_then(|etag| etag.to_str().ok())
        .map(str::to_string);
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| chain(&error.without_url()))?
    {
        if bytes.len() + chunk.len() > get.limit {
            return Err(format!("body over {} bytes", get.limit));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Got::Body { bytes, etag })
}

/// 一个错误连同它的来由，一层一层接起来。
fn chain(error: &(dyn Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(inner) = source {
        text.push_str(": ");
        text.push_str(&inner.to_string());
        source = inner.source();
    }
    text
}
