//! 一跳一跳地抓（`net.md`「怎么走」第 3 到 8 条，照桥的 `fetch.rs`）：抓一页的 `<head>`，抓卡片的图和图标。
//!
//! - 每一跳先过闸（`guard.rs`），再照 `proxy.rs` 挑怎么连：不走代理的钉住解析好的地址，本机解析不出来的连不上；
//!   走代理的交给代理，本机解析不出来的照样交（第 5 条）。
//! - 跳转不交给 reqwest（`Policy::none()`）：自己跟，最多几跳照 `link_preview.json`，每一跳重新过闸。自动跟就等于
//!   中途某一跳可以指向内网而没人再看一眼。
//! - 一跳的时限从过闸算起：解析、连上、发出、读完都在里面（「起草时定的」第 5 条）。

mod clients;

use std::time::Duration;

use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, LOCATION, USER_AGENT};
use reqwest::{Response, Url};
use tokio::time::Instant;

use crate::Why;
use crate::body;
use crate::guard::{Guard, Resolved};
use crate::proxy::Proxies;
use crate::rules::{Fetching, Rules};
use clients::{Clients, Way};

/// 照规矩抓：闸、代理、复用的客户端、请求头。
pub(crate) struct Fetcher {
    guard: Guard,
    proxies: Proxies,
    clients: Clients,
    redirects: usize,
    user_agent: String,
    accept_language: String,
    page: Fetching,
    image: Fetching,
}

impl Fetcher {
    /// 照 `rules` 抓，过 `guard` 这道闸，代理照 `proxies`。
    pub(crate) fn new(rules: &Rules, guard: Guard, proxies: Proxies) -> Fetcher {
        Fetcher {
            guard,
            proxies,
            clients: Clients::new(rules.client_ttl, rules.client_keep),
            redirects: rules.redirects,
            user_agent: rules.user_agent.clone(),
            accept_language: rules.accept_language.clone(),
            page: rules.page.clone(),
            image: rules.image.clone(),
        }
    }

    /// 抓一页：交回最后落到的地址和读到 `</head>` 为止的那一截（`<head>` 里什么都没挖到时接着往下读，见
    /// `body::read_head`，W-7 补）；照 UTF-8 读，读不了的字换成替换符。
    ///
    /// # Errors
    ///
    /// 过不了闸、跳转太多、不是 HTML：[`Why::NoPreview`]；超时、连不上、4xx、5xx、读到一半断了：
    /// [`Why::Unreachable`]。
    pub(crate) async fn page(&self, url: &Url) -> Result<(Url, String), Why> {
        let (response, page) = self.get(url, &self.page).await?;
        let kind = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !kind.contains("html") {
            return Err(Why::NoPreview);
        }
        let head = body::read_head(response, self.page.max_bytes)
            .await
            .map_err(|_| Why::Unreachable)?;
        Ok((page, String::from_utf8_lossy(&head).into_owned()))
    }

    /// 抓一张图：交回字节和照魔数认出来的媒体类型；抓不到、太大、认不出的都是 `None`（卡片照样成立，只是没图）。
    pub(crate) async fn image(&self, url: &Url) -> Option<(Vec<u8>, &'static str)> {
        let (response, _) = self.get(url, &self.image).await.ok()?;
        let max = self.image.max_bytes;
        if response
            .content_length()
            .is_some_and(|length| length > u64::try_from(max).unwrap_or(u64::MAX))
        {
            return None;
        }
        // 多读一个字节：读满了说明超了，整张不要（截半截的图画出来是坏的）
        let bytes = body::read_prefix(response, max.saturating_add(1))
            .await
            .ok()?;
        if bytes.len() > max {
            return None;
        }
        let kind = body::sniff_image(&bytes)?;
        Some((bytes, kind))
    }

    /// GET 一个地址，自己跟跳转：每一跳过闸、挑怎么连。交回最后那一跳的回应和它的地址。
    async fn get(&self, url: &Url, fetching: &Fetching) -> Result<(Response, Url), Why> {
        let mut current = url.clone();
        for _ in 0..=self.redirects {
            let response = self.hop(&current, fetching).await?;
            let status = response.status();
            if status.is_redirection() {
                let location = response
                    .headers()
                    .get(LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or(Why::Unreachable)?;
                current = current.join(location).map_err(|_| Why::Unreachable)?;
                continue;
            }
            if !status.is_success() {
                return Err(Why::Unreachable);
            }
            return Ok((response, current));
        }
        // 跳转太多：不会自己变好
        Err(Why::NoPreview)
    }

    /// 一跳：过闸，挑怎么连，发出去，交回回应（还没读身子）。
    async fn hop(&self, url: &Url, fetching: &Fetching) -> Result<Response, Why> {
        let deadline = Instant::now() + fetching.timeout;
        let resolved = self
            .guard
            .pass(url, fetching.timeout)
            .await
            .map_err(|_| Why::NoPreview)?;
        let way = match (self.proxies.via(url), resolved) {
            (Some(via), _) => Way::Proxy(via),
            (None, Resolved::Addresses(addresses)) => {
                let host = url.host_str().unwrap_or_default().to_string();
                Way::Pinned(host, addresses)
            }
            (None, Resolved::Literal) => Way::Direct,
            (None, Resolved::Nowhere) => return Err(Why::Unreachable),
        };
        let client = self.clients.get(&way).map_err(|_| Why::Unreachable)?;
        let left = deadline.saturating_duration_since(Instant::now());
        if left == Duration::ZERO {
            return Err(Why::Unreachable);
        }
        client
            .get(url.clone())
            .timeout(left)
            .header(USER_AGENT, &self.user_agent)
            .header(ACCEPT, &fetching.accept)
            .header(ACCEPT_LANGUAGE, &self.accept_language)
            .send()
            .await
            .map_err(|_| Why::Unreachable)
    }
}
