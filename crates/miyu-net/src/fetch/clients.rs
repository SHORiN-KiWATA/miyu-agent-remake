//! 复用的客户端（`net.md`「怎么走」第 10 条，照桥）：每一跳新造一个要重新装证书、从零握手（旧版实测一跳约 0.2 秒），
//! 所以照「怎么连」留一会儿。怎么连变了（钉的地址换了、换了代理）就是另一个客户端，闸照旧每一跳都过。

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use reqwest::Client;
use reqwest::redirect::Policy;

use crate::proxy::Via;

/// 这一跳怎么连。
#[derive(Debug, Clone)]
pub(crate) enum Way {
    /// 主机写的就是 IP、不走代理：照它连。
    Direct,
    /// 不走代理：钉住本机解析好的地址（主机名，地址），中间没有第二次解析。
    Pinned(String, Vec<SocketAddr>),
    /// 交给代理连，不钉地址（代理那头自己解析）。
    Proxy(Via),
}

impl Way {
    /// 复用的键：怎么连一样的才共用一个客户端。
    fn key(&self) -> String {
        match self {
            Way::Direct => "direct".to_string(),
            Way::Pinned(host, addresses) => format!("pinned|{host}|{addresses:?}"),
            Way::Proxy(via) => format!("proxy|{}", via.uri),
        }
    }
}

/// 留着的几个客户端：照键，记着什么时候造的。
pub(crate) struct Clients {
    ttl: Duration,
    keep: usize,
    made: Mutex<HashMap<String, (Instant, Client)>>,
}

impl Clients {
    /// 一个留 `ttl`，最多留 `keep` 个，满了整个清空。
    pub(crate) fn new(ttl: Duration, keep: usize) -> Clients {
        Clients {
            ttl,
            keep,
            made: Mutex::new(HashMap::new()),
        }
    }

    /// 照 `way` 连的客户端：留着的还没过期就用它，不然新造一个留下。
    ///
    /// # Errors
    ///
    /// 造不出来：TLS 初始化失败、reqwest 读不懂代理的地址。
    pub(crate) fn get(&self, way: &Way) -> reqwest::Result<Client> {
        let key = way.key();
        let now = Instant::now();
        let mut made = self.made.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((at, client)) = made.get(&key)
            && now.duration_since(*at) < self.ttl
        {
            return Ok(client.clone());
        }
        let client = build(way)?;
        made.retain(|_, (at, _)| now.duration_since(*at) < self.ttl);
        if made.len() >= self.keep {
            made.clear();
        }
        made.insert(key, (now, client.clone()));
        Ok(client)
    }
}

/// 造一个：TLS 用 rustls；跳转一个都不跟（自己跟，每一跳过闸）；照 `way` 钉地址、不走代理或者只走那一个代理。
fn build(way: &Way) -> reqwest::Result<Client> {
    let builder = Client::builder().use_rustls_tls().redirect(Policy::none());
    let builder = match way {
        Way::Direct => builder.no_proxy(),
        Way::Pinned(host, addresses) => builder.no_proxy().resolve_to_addrs(host, addresses),
        // 设了一个代理，reqwest 就不再自己照环境变量找代理：这个客户端发的只经它。
        Way::Proxy(via) => builder.proxy(via.proxy()?),
    };
    builder.build()
}
