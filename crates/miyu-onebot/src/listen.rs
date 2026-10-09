//! NapCat 反连进来的那一下（`onebot.md` 第一条「怎么走」第 2 条）：一个 TCP 连接先当 HTTP 读，路径、令牌、升级都对了回
//! 101，升级好以后在同一个任务里接着当 WebSocket 用（`connection`）。
//!
//! - 只认 `bridge.json` 的 `paths`（出厂是 `/onebot/v11/ws`、`/ws`），别的 404。
//! - 令牌照 `Authorization: Bearer <令牌>`、`Authorization: Token <令牌>`、查询参数 `access_token` 的先后取，和桥手里最新的
//!   （`crate::current`：握手交来的，核心推来新的就换上，施工 O-20）按常数时间比（长短不一样直接不对，一样长的每个字节都比，
//!   照核心比本机令牌的规矩，`protocol.md`「握手」第 3 条）：令牌刚在 WebUI 或命令行里设、换、删的，不用重启就照新的。对不上、
//!   桥手里没有、没出示的 401。查询参数照原样比，不做百分号解码：NapCat 用头出示。
//! - 不是 WebSocket 的升级请求：400。算出来的 `Sec-WebSocket-Accept` 放不进回应的头：也是 400，不升级。
//! - 机器人的号照 `X-Self-ID` 头取，没有的等第一条事件的 `self_id`。
//!
//! 升级好的连接不另起任务：在接这个 TCP 连接的任务里跑（hyper 的连接在交出升级以后就结束了）。接连接的任务都在
//! `serve` 的一组里，桥停下时一起停。

pub(crate) mod bots;
mod connection;
#[cfg(test)]
mod tests;

use std::convert::Infallible;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, PoisonError};

use http_body_util::Empty;
use hyper::body::{Bytes, Incoming};
use hyper::header::{self, HeaderValue};
use hyper::upgrade::OnUpgrade;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tokio::sync::{Notify, mpsc};
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;

use crate::TARGET;
use crate::current::Current;
use crate::onebot::{Event, number};
use crate::serve::Notice;
use crate::tuning::Tuning;
use bots::Bots;

/// 各个连接共用的：桥手里的令牌、桥自己的数、连着的号、读出来的消息和撤回交给谁、说给人听的。
pub(crate) struct Gate {
    /// NapCat 要出示的令牌在这里（和 WebUI 共用，核心推来新的就换上，施工 O-20）。
    pub(crate) current: Arc<Current>,
    /// 认哪几个路径、调用等多久、写队列多长（`bridge.json`）。
    pub(crate) tuning: Tuning,
    /// 连着的号。
    pub(crate) bots: Arc<Bots>,
    /// 读出来的消息、撤回（施工 O-22 起群的也算）、禁言和解禁、认出来的号（施工 O-25 中）交给跟核心的那一头（`core/route.rs`）。
    pub(crate) inbound: mpsc::Sender<Event>,
    /// 连上、断开各说一行（「样子」）。
    pub(crate) tell: Arc<dyn Fn(Notice) + Send + Sync>,
    /// 下一条连接的序号（`bots::Link::serial`）。
    pub(crate) serial: AtomicU64,
    /// NapCat 连上、断开、认出号、问到是哪个实现时叫一声：状态文件照这一刻再写（`crate::status_file`，施工 O-18）。
    pub(crate) changed: Arc<Notify>,
}

/// 升级了的一个：hyper 交出连接的那一头、`X-Self-ID` 报的号。
type Upgrade = (OnUpgrade, Option<i64>);

/// 接一个 TCP 连接：当 HTTP 读，升级成了的接着当 NapCat 的连接跑，直到断开。
pub(crate) async fn accept(stream: TcpStream, gate: Arc<Gate>) {
    let slot: Arc<Mutex<Option<Upgrade>>> = Arc::default();
    let service = {
        let slot = Arc::clone(&slot);
        let gate = Arc::clone(&gate);
        hyper::service::service_fn(move |request| {
            let (gate, slot) = (Arc::clone(&gate), Arc::clone(&slot));
            async move { Ok::<_, Infallible>(handle(request, &gate, &slot).await) }
        })
    };
    let served = hyper::server::conn::http1::Builder::new()
        .serve_connection(TokioIo::new(stream), service)
        .with_upgrades()
        .await;
    if let Err(error) = served {
        tracing::debug!(target: TARGET, error = %error, "connection ended");
    }
    let upgrade = slot.lock().unwrap_or_else(PoisonError::into_inner).take();
    let Some((upgrade, bot)) = upgrade else {
        return;
    };
    match upgrade.await {
        Ok(upgraded) => connection::run(TokioIo::new(upgraded), &gate, bot).await,
        Err(error) => tracing::debug!(target: TARGET, error = %error, "not upgraded"),
    }
}

/// 一个请求：路径、令牌、升级都对了的回 101，把升级交进 `slot`；回别的不交。
async fn handle(
    mut request: Request<Incoming>,
    gate: &Gate,
    slot: &Mutex<Option<Upgrade>>,
) -> Response<Empty<Bytes>> {
    let path = request.uri().path();
    if !gate.tuning.paths.iter().any(|known| known == path) {
        return empty(StatusCode::NOT_FOUND);
    }
    let presented = presented(&request).map(str::to_string);
    if !admitted(gate, presented.as_deref()) {
        tracing::warn!(target: TARGET, path = request.uri().path(), "token refused");
        return empty(StatusCode::UNAUTHORIZED);
    }
    let headers = request.headers();
    let upgrade = headers
        .get(header::UPGRADE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"));
    let version = headers
        .get(header::SEC_WEBSOCKET_VERSION)
        .is_some_and(|value| value.as_bytes() == b"13");
    let Some(key) = headers
        .get(header::SEC_WEBSOCKET_KEY)
        .filter(|_| upgrade && version)
    else {
        return empty(StatusCode::BAD_REQUEST);
    };
    let bot = headers
        .get("x-self-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| number(&serde_json::Value::from(value)));
    let response = switching(&derive_accept_key(key.as_bytes()));
    if response.status() == StatusCode::SWITCHING_PROTOCOLS {
        *slot.lock().unwrap_or_else(PoisonError::into_inner) =
            Some((hyper::upgrade::on(&mut request), bot));
    }
    response
}

/// 升级的回应：101，带上算好的 `Sec-WebSocket-Accept`。放不进头的（算出来的是 base64，照说不会）回 400、不升级，记一行
/// 运行日志：回了 101 却不带它，NapCat 那头握手失败，这一头却当升级成了。
fn switching(accept: &str) -> Response<Empty<Bytes>> {
    let Ok(accept) = HeaderValue::from_str(accept) else {
        tracing::warn!(target: TARGET, "websocket accept key not a header value");
        return empty(StatusCode::BAD_REQUEST);
    };
    let mut response = empty(StatusCode::SWITCHING_PROTOCOLS);
    let headers = response.headers_mut();
    headers.insert(header::UPGRADE, HeaderValue::from_static("websocket"));
    headers.insert(header::CONNECTION, HeaderValue::from_static("Upgrade"));
    headers.insert(header::SEC_WEBSOCKET_ACCEPT, accept);
    response
}

/// 出示的 `presented` 对得上桥手里最新的令牌。手里没有、没出示的对不上。
fn admitted(gate: &Gate, presented: Option<&str>) -> bool {
    match (presented, gate.current.token()) {
        (Some(presented), Some(token)) => same(presented.as_bytes(), token.expose().as_bytes()),
        _ => false,
    }
}

/// 出示的令牌：`Authorization` 头（`Bearer`、`Token`，不分大小写）在前，查询参数 `access_token` 在后。都没有的是空的。
fn presented(request: &Request<Incoming>) -> Option<&str> {
    let header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .filter(|(scheme, _)| {
            scheme.eq_ignore_ascii_case("Bearer") || scheme.eq_ignore_ascii_case("Token")
        })
        .map(|(_, token)| token.trim());
    header.or_else(|| {
        request
            .uri()
            .query()?
            .split('&')
            .find_map(|pair| pair.strip_prefix("access_token="))
    })
}

/// 两串字节一样不一样，按常数时间比：长短不一样直接不一样；一样长的每个字节都比，比到哪一个不一样都用一样长的时间。
fn same(presented: &[u8], token: &[u8]) -> bool {
    presented.len() == token.len()
        && presented
            .iter()
            .zip(token)
            .fold(0u8, |differs, (a, b)| differs | (a ^ b))
            == 0
}

/// 没有内容的回应。
fn empty(status: StatusCode) -> Response<Empty<Bytes>> {
    let mut response = Response::new(Empty::new());
    *response.status_mut() = status;
    response
}
