//! 一次 POST，整个读完（施工 R-5 补：远程的 embedding，`docs/blueprint/recall.md` 第四条第 1 款）：带着头和 JSON 的请求体
//! 发出去，2xx 的交回整个响应体，有总时限、有大小上限。出错的说法同一次 GET（[`crate::get_full`]）：原话不带地址，回的不是
//! 2xx 的另有状态码、响应头、最多 64 KiB 的响应体。

use std::time::Duration;

use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use tokio::time::timeout;

use crate::get::{Failed, chain, failure};

/// 一次 POST 要的。
#[derive(Clone, Copy)]
pub struct Post<'a> {
    /// HTTP 客户端：连接的时限在它上面（[`crate::fetcher`]）。
    pub client: &'a reqwest::Client,
    /// 完整的地址。
    pub url: &'a str,
    /// 另带的头，照先后：认证头这些。值写不进头的，这一次出错。
    pub headers: &'a [(String, String)],
    /// 请求体：JSON（`Content-Type: application/json`）。
    pub body: &'a [u8],
    /// 整个最多多久：从发出到读完。
    pub timeout: Duration,
    /// 响应体最多多少字节。
    pub limit: usize,
}

/// POST 一次，交回 2xx 的响应体。
///
/// # Errors
///
/// 头写不进去、连不上、超时、回的不是 2xx、响应体超过上限、读到一半断了：英文的一句原话，回了的带着状态码和响应体。
pub async fn post(post: Post<'_>) -> Result<Vec<u8>, Failed> {
    let mut headers = HeaderMap::new();
    for (name, value) in post.headers {
        let header = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| Failed::said(format!("header {name} is not valid")))?;
        // 值可能是密钥，不进原话。
        let value = HeaderValue::from_str(value)
            .map_err(|_| Failed::said(format!("value of header {name} is not valid")))?;
        headers.append(header, value);
    }
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    match timeout(post.timeout, exchange(post, headers)).await {
        Ok(answered) => answered,
        Err(_) => Err(Failed::said(format!(
            "timed out after {} seconds",
            post.timeout.as_secs_f64()
        ))),
    }
}

/// 发、读完，超过上限停。
async fn exchange(post: Post<'_>, headers: HeaderMap) -> Result<Vec<u8>, Failed> {
    let broken = |error: reqwest::Error| Failed::said(chain(&error.without_url()));
    let mut response = post
        .client
        .post(post.url)
        .headers(headers)
        .body(post.body.to_vec())
        .send()
        .await
        .map_err(broken)?;
    let status = response.status();
    if !status.is_success() {
        return Err(failure(status, response).await);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(broken)? {
        if bytes.len() + chunk.len() > post.limit {
            return Err(Failed::said(format!("body over {} bytes", post.limit)));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
