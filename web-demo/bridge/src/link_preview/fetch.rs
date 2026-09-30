//! 出站抓取（照旧版 `link_preview/fetch.rs`）：抓一页的 `<head>`，抓卡片的图和图标。
//!
//! 为什么由桥抓：页面只跟本机说话（`img-src 'self'`、`connect-src 'self'`）；放开等于让模型写的任意链接在你的浏览器上
//! 留一次带 IP 的请求，远程像素追踪就是这么做的。所以页面、图都由桥代劳，图经 `/link-image` 从本机给。
//!
//! - 每一跳先过 `guard.rs` 的闸，解析好的地址钉进这一跳的客户端（`resolve_to_addrs`），不走代理（`no_proxy`）；
//! - 重定向不交给 reqwest（`Policy::none()`）：自己跟，最多几跳照 `link_preview.json`，每跳重新过闸。自动跟就等于中途
//!   某一跳可以指向内网而没人再看一眼；
//! - 页面读到 `</head>` 或 `<body` 就停，字节上限只是兜底（旧版 256 KB 的死上限读不到 YouTube 的 og 标签：它们在第
//!   70 万字节上，`<head>` 里塞满了内联脚本，旧版 09-09 实测）；
//! - 图只收认得出的五种（按开头的魔数认，不看对面说的类型）：PNG、JPEG、GIF、WebP、ICO。**不收 SVG**：它能带脚本，
//!   而这些字节最后是从本机同源发出去的。
//! - 客户端按「主机 + 钉住的地址 + 预算」复用一会儿：每跳新建一个要重新装证书、从零握手（旧版实测每跳约 0.2 秒）；
//!   地址变了就是另一个键、另一个客户端，闸照旧每跳都过。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, LOCATION, USER_AGENT};
use reqwest::redirect::Policy;
use reqwest::{Client, Response, Url};

use super::Miss;
use super::guard::{self, Pinned};

/// 抓取的规矩（`resources/link_preview.json`）。
pub(super) struct Rules {
    /// 抓页面：每一跳的预算。
    pub(super) page_timeout: Duration,
    /// 抓页面：最多读多少字节（读到 `</head>` 就停，这是兜底）。
    pub(super) page_bytes: usize,
    /// 抓页面的 `Accept`。
    pub(super) page_accept: String,
    /// 抓图：每一跳的预算。
    pub(super) image_timeout: Duration,
    /// 抓图：超过这么多字节的不要。
    pub(super) image_bytes: usize,
    /// 抓图的 `Accept`。
    pub(super) image_accept: String,
    /// 最多跟几跳重定向。
    pub(super) redirects: usize,
    /// `User-Agent`：装成 Chrome，有的站对不认识的客户端不给 og 标签。
    pub(super) user_agent: String,
    /// `Accept-Language`。
    pub(super) accept_language: String,
    /// 复用的客户端留多久。
    pub(super) client_ttl: Duration,
    /// 最多留几个客户端，满了整个清空。
    pub(super) client_keep: usize,
}

/// 照规矩抓，记着几个能复用的客户端。
pub(super) struct Fetcher {
    rules: Rules,
    clients: Mutex<HashMap<String, (Instant, Client)>>,
}

impl Fetcher {
    /// 照 `rules` 抓。
    pub(super) fn new(rules: Rules) -> Fetcher {
        Fetcher { rules, clients: Mutex::new(HashMap::new()) }
    }

    /// 抓一页：交回最后落到的地址和读到 `</head>` 为止的那一截（照 UTF-8 读，读不了的字换成替换符）。
    ///
    /// # Errors
    ///
    /// 过不了闸、重定向太多、对面不是网页：[`Miss::NoPreview`]；超时、连不上、对面回 4xx/5xx：[`Miss::Transient`]。
    pub(super) async fn page(&self, url: &Url) -> Result<(Url, String), Miss> {
        let (response, page) = self.get(url, &self.rules.page_accept, self.rules.page_timeout).await?;
        let kind = response.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or_default().to_ascii_lowercase();
        if !kind.contains("html") {
            return Err(Miss::NoPreview);
        }
        let head = read_head(response, self.rules.page_bytes).await?;
        Ok((page, String::from_utf8_lossy(&head).into_owned()))
    }

    /// 抓一张图：交回字节和照魔数认出来的媒体类型；抓不到、太大、认不出的都是 `None`（卡片照样成立，只是没图）。
    pub(super) async fn image(&self, url: &Url) -> Option<(Vec<u8>, &'static str)> {
        let (response, _) = self.get(url, &self.rules.image_accept, self.rules.image_timeout).await.ok()?;
        let max = self.rules.image_bytes;
        if response.content_length().is_some_and(|length| length > u64::try_from(max).unwrap_or(u64::MAX)) {
            return None;
        }
        // 多读一个字节：读满了说明超了，整张不要（截半截的图画出来是坏的）
        let bytes = read_prefix(response, max.saturating_add(1)).await.ok()?;
        if bytes.len() > max {
            return None;
        }
        let kind = sniff_image(&bytes)?;
        Some((bytes, kind))
    }

    /// GET 一个地址，自己跟重定向：每一跳过闸、钉住地址。交回最后那一跳的回应和它的地址。
    async fn get(&self, url: &Url, accept: &str, budget: Duration) -> Result<(Response, Url), Miss> {
        let mut current = url.clone();
        for _ in 0..=self.rules.redirects {
            let pinned = guard::resolve(&current, budget).await?;
            let response = self
                .client(pinned.as_ref(), budget)?
                .get(current.clone())
                .header(USER_AGENT, &self.rules.user_agent)
                .header(ACCEPT, accept)
                .header(ACCEPT_LANGUAGE, &self.rules.accept_language)
                .send()
                .await
                .map_err(|_| Miss::Transient)?;
            if response.status().is_redirection() {
                let location = response.headers().get(LOCATION).and_then(|v| v.to_str().ok()).ok_or(Miss::Transient)?;
                current = current.join(location).map_err(|_| Miss::Transient)?;
                continue;
            }
            let response = response.error_for_status().map_err(|_| Miss::Transient)?;
            return Ok((response, current));
        }
        // 重定向太多：不会自己变好
        Err(Miss::NoPreview)
    }

    /// 这一跳的客户端：钉住 `pinned` 里的地址（主机写的就是 IP 的不用钉），不跟重定向，不走代理。
    fn client(&self, pinned: Option<&Pinned>, budget: Duration) -> Result<Client, Miss> {
        let key = match pinned {
            Some((host, addresses)) => format!("{host}|{addresses:?}|{}", budget.as_millis()),
            None => format!("|{}", budget.as_millis()),
        };
        let now = Instant::now();
        let ttl = self.rules.client_ttl;
        if let Ok(clients) = self.clients.lock()
            && let Some((at, client)) = clients.get(&key)
            && now.duration_since(*at) < ttl
        {
            return Ok(client.clone());
        }
        let mut builder = Client::builder().timeout(budget).redirect(Policy::none()).no_proxy();
        if let Some((host, addresses)) = pinned {
            builder = builder.resolve_to_addrs(host, addresses);
        }
        let client = builder.build().map_err(|_| Miss::Transient)?;
        if let Ok(mut clients) = self.clients.lock() {
            clients.retain(|_, (at, _)| now.duration_since(*at) < ttl);
            if clients.len() >= self.rules.client_keep {
                clients.clear();
            }
            clients.insert(key, (now, client.clone()));
        }
        Ok(client)
    }
}

/// 读回应的前 `max` 个字节。
async fn read_prefix(mut response: Response, max: usize) -> Result<Vec<u8>, Miss> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Miss::Transient)? {
        let room = max.saturating_sub(body.len());
        if chunk.len() >= room {
            body.extend_from_slice(&chunk[..room]);
            break;
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// 读到 `</head>` 或 `<body` 就停，最多 `max` 个字节：要的全在 `<head>` 里，正文可以有几 MB。
async fn read_head(mut response: Response, max: usize) -> Result<Vec<u8>, Miss> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Miss::Transient)? {
        // 记号可能跨在两块的接缝上：从上一块的尾巴开始找
        let scan_from = body.len().saturating_sub(MARKER_OVERLAP);
        let room = max.saturating_sub(body.len());
        if chunk.len() >= room {
            body.extend_from_slice(&chunk[..room]);
            break;
        }
        body.extend_from_slice(&chunk);
        if let Some(end) = find_head_end(&body[scan_from..]) {
            body.truncate(scan_from + end);
            break;
        }
    }
    Ok(body)
}

const HEAD_END: &[u8] = b"</head>";
const BODY_START: &[u8] = b"<body";
/// 接缝上往回看几个字节：长的那个记号的长度就够。
const MARKER_OVERLAP: usize = if HEAD_END.len() > BODY_START.len() { HEAD_END.len() } else { BODY_START.len() };

/// `<head>` 在哪儿结束：`</head>` 之后，或者 `<body` 之前，哪个先到算哪个（不分大小写）。
fn find_head_end(haystack: &[u8]) -> Option<usize> {
    let lower = haystack.to_ascii_lowercase();
    let head = lower.windows(HEAD_END.len()).position(|w| w == HEAD_END).map(|at| at + HEAD_END.len());
    let body = lower.windows(BODY_START.len()).position(|w| w == BODY_START);
    match (head, body) {
        (Some(head), Some(body)) => Some(head.min(body)),
        (head, body) => head.or(body),
    }
}

/// 照开头的魔数认图，认出来的交回媒体类型；认不出的一律不要（分不清是什么就别当图发出去）。
///
/// 收 ICO 是因为一半的站 `rel=icon` 还是 `.ico`，不收就只能画首字母。
fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x00\x00\x01\x00") {
        Some("image/x-icon")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_image_bytes_are_kept() {
        for (bytes, kind) in [
            (&b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR"[..], Some("image/png")),
            (b"\xff\xd8\xff\xe0\x00\x10JFIF", Some("image/jpeg")),
            (b"GIF87a\x01\x00", Some("image/gif")),
            (b"GIF89a\x01\x00", Some("image/gif")),
            (b"RIFF\x24\x00\x00\x00WEBPVP8 ", Some("image/webp")),
            (b"\x00\x00\x01\x00\x01\x00\x10\x10", Some("image/x-icon")),
            // SVG 能带脚本，这些字节最后从本机同源发出去：永远不收
            (b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>", None),
            (b"<?xml version=\"1.0\"?><svg/>", None),
            (b"<html><body>gotcha</body></html>", None),
            (b"<!DOCTYPE html>", None),
            (b"RIFF\x24\x00\x00\x00WAVEfmt ", None),
            (b"\x89PN", None),
            (b"", None),
        ] {
            assert_eq!(sniff_image(bytes), kind, "{}", String::from_utf8_lossy(bytes));
        }
    }

    #[test]
    fn a_head_is_cut_at_its_end() {
        let html = b"<html><head><title>x</title></head><body>aaaaaaaa</body></html>";
        assert_eq!(find_head_end(html).map(|end| &html[..end]), Some(&b"<html><head><title>x</title></head>"[..]));
        // 没有 </head> 的退到 <body 前面；大小写不管
        let no_close = b"<html><head><title>x</title><BODY>tail";
        assert_eq!(find_head_end(no_close).map(|end| &no_close[..end]), Some(&b"<html><head><title>x</title>"[..]));
        assert_eq!(find_head_end(b"<html><head><title>x</title>"), None);
    }
}
