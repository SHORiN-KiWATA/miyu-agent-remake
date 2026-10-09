//! 一次 GET（`docs/blueprint/models.md`「在哪」、`http.md`「一次 GET」，施工 8-7）：拉 models.dev 的目录、拉供应商的模型
//! 列表。和发请求（[`crate::send()`]）分开：不是流，整个读完再交回，有总时限、有大小上限，可以带上次的 `ETag` 问「变了
//! 没有」。
//!
//! 出错的原话是英文的一句，进运行日志：不带地址（路径、参数里可能有 key），reqwest 的错先去掉地址再接起来。
//!
//! [`get_full`]（施工 8-11）：出错时另交回状态码、响应头、最多 64 KiB 的响应体，`provider.test` 列模型失败时照驱动分类
//! （「和 `model.called` 的一样」）；[`get`] 只交原话，拉目录、拉列表照旧用它。
//!
//! [`download`]（施工 R-5 中）：边下边交，响应体一块一块交给收的一方、不攒成一整块：本机 embedding 的模型文件几十 MB。

use std::error::Error;
use std::io;
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

/// 出错的响应体最多读多少：分类用不着那么多（和发请求的一样）。
const ERROR_BODY_LIMIT: usize = 64 * 1024;

/// 没拿到的：原话，回的不是 2xx 也不是 304 的另有状态码、响应头、响应体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failed {
    /// 英文的一句原话，和 [`get`] 交回的一样。
    pub message: String,
    /// HTTP 状态码；连不上、超时这类没有。
    pub status: Option<u16>,
    /// 响应头，名字是小写的；没有响应的是空的。
    pub headers: Vec<(String, String)>,
    /// 响应体，最多 64 KiB；没有响应的是空的。
    pub body: Vec<u8>,
}

impl Failed {
    /// 只有原话的：连不上、超时、太大这类。
    pub(crate) fn said(message: String) -> Failed {
        Failed {
            message,
            status: None,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }
}

/// GET 一次。
///
/// # Errors
///
/// 头写不进去、连不上、超时、回的不是 2xx 也不是 304、响应体超过上限、读到一半断了：英文的一句原话。
pub async fn get(get: Get<'_>) -> Result<Got, String> {
    get_full(get).await.map_err(|failed| failed.message)
}

/// 同 [`get`]，出错时交回 [`Failed`]：回的不是 2xx 也不是 304 的，带着状态码、响应头、最多 64 KiB 的响应体（施工 8-11）。
///
/// # Errors
///
/// 同 [`get`]。
pub async fn get_full(get: Get<'_>) -> Result<Got, Failed> {
    let mut bytes = Vec::new();
    let mut keep = |chunk: &[u8]| {
        bytes.extend_from_slice(chunk);
        Ok(())
    };
    match fetch(get, &mut keep).await? {
        Fetched::Body { etag } => Ok(Got::Body { bytes, etag }),
        Fetched::NotModified => Ok(Got::NotModified),
    }
}

/// 边下边交（施工 R-5 中）：2xx 的响应体一块一块交给 `each`，交回一共多少字节。`etag` 带上了照样带，回 304 的算出错
/// （要下的一方没东西可用）。
///
/// # Errors
///
/// 同 [`get`]；`each` 出错的照它的原话，当场停。
pub async fn download(
    get: Get<'_>,
    each: &mut (dyn FnMut(&[u8]) -> io::Result<()> + Send),
) -> Result<u64, String> {
    let mut total = 0_u64;
    let mut count = |chunk: &[u8]| {
        each(chunk).map_err(|error| Failed::said(error.to_string()))?;
        total += chunk.len() as u64;
        Ok(())
    };
    match fetch(get, &mut count).await {
        Ok(Fetched::Body { .. }) => Ok(total),
        Ok(Fetched::NotModified) => Err("HTTP 304".to_string()),
        Err(failed) => Err(failed.message),
    }
}

/// 一块响应体交给谁：出错的当场停。
type Each<'e> = dyn FnMut(&[u8]) -> Result<(), Failed> + Send + 'e;

/// 发完一次：响应体已经一块一块交出去了，这里只剩它的 `ETag`。
enum Fetched {
    /// 2xx，回的 `ETag`（没有的没有）。
    Body { etag: Option<String> },
    /// 304。
    NotModified,
}

/// 带上头、限着时发一次，2xx 的响应体一块一块交给 `each`。
async fn fetch(get: Get<'_>, each: &mut Each<'_>) -> Result<Fetched, Failed> {
    let mut headers = HeaderMap::new();
    for (name, value) in get.headers {
        let header = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| Failed::said(format!("header {name} is not valid")))?;
        // 值可能是密钥，不进原话。
        let value = HeaderValue::from_str(value)
            .map_err(|_| Failed::said(format!("value of header {name} is not valid")))?;
        headers.append(header, value);
    }
    if let Some(etag) = get.etag
        && let Ok(etag) = HeaderValue::from_str(etag)
    {
        headers.insert(IF_NONE_MATCH, etag);
    }
    match timeout(get.timeout, exchange(get, headers, each)).await {
        Ok(got) => got,
        Err(_) => Err(Failed::said(format!(
            "timed out after {} seconds",
            get.timeout.as_secs_f64()
        ))),
    }
}

/// 发、一块一块读完，超过上限停。
async fn exchange(
    get: Get<'_>,
    headers: HeaderMap,
    each: &mut Each<'_>,
) -> Result<Fetched, Failed> {
    let broken = |error: reqwest::Error| Failed::said(chain(&error.without_url()));
    let mut response = get
        .client
        .get(get.url)
        .headers(headers)
        .send()
        .await
        .map_err(broken)?;
    let status = response.status();
    if status == StatusCode::NOT_MODIFIED {
        return Ok(Fetched::NotModified);
    }
    if !status.is_success() {
        return Err(failure(status, response).await);
    }
    let etag = response
        .headers()
        .get(ETAG)
        .and_then(|etag| etag.to_str().ok())
        .map(str::to_string);
    let mut read = 0;
    while let Some(chunk) = response.chunk().await.map_err(broken)? {
        read += chunk.len();
        if read > get.limit {
            return Err(Failed::said(format!("body over {} bytes", get.limit)));
        }
        each(&chunk)?;
    }
    Ok(Fetched::Body { etag })
}

/// 回的不是 2xx 也不是 304：状态码、响应头、最多 64 KiB 的响应体（读到一半断了的照读到的）。一次 POST 也照它说（`post.rs`）。
pub(crate) async fn failure(status: StatusCode, mut response: reqwest::Response) -> Failed {
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_string(),
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            )
        })
        .collect();
    let mut body = Vec::new();
    while body.len() < ERROR_BODY_LIMIT {
        match response.chunk().await {
            Ok(Some(chunk)) => body.extend_from_slice(&chunk),
            Ok(None) | Err(_) => break,
        }
    }
    body.truncate(ERROR_BODY_LIMIT);
    Failed {
        message: format!("HTTP {}", status.as_u16()),
        status: Some(status.as_u16()),
        headers,
        body,
    }
}

/// 一个错误连同它的来由，一层一层接起来。
pub(crate) fn chain(error: &(dyn Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(inner) = source {
        text.push_str(": ");
        text.push_str(&inner.to_string());
        source = inner.source();
    }
    text
}
