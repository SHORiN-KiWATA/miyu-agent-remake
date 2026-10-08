//! `/apply`（`onebot.md` 第二条「对外的样子」「施工时定的」第 19 条，施工 O-16 补二）：页面存好配置以后叫桥当场照新的，不用
//! 重启。要登录令牌（`super::login`），只认 `POST`。
//!
//! 1. 重读配置：令牌照读到的换上（`crate::current`）。读不出来（端口坏了，走不到）回 500。
//! 2. 两个端口照配置里写的，和上一次照的（[`super::Web::applied`]）比，变了的开新的（[`crate::serve::bind`]）。都开上了才
//!    交给 `serve` 换掉旧的，回 `{"listen": 端口, "web": 端口}`（实际听的）；有一个开不了，回 409 `{"in_use": 端口}`，开好的
//!    放掉，两个都不换，旧的照旧开着，桥不会落到哪个端口都不听。令牌照样换上了。
//! 3. 已经接进来的连接（NapCat 的那一条、页面正在用的这一条）不断：换掉的只是监听。
//!
//! 同时来的几个 `/apply` 锁着 `applied` 一个一个办。

use std::sync::atomic::Ordering;

use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};
use serde_json::json;
use tokio::net::TcpListener;

use miyu_webserve::respond::{Body, empty};

use super::{Web, login, reply};
use crate::TARGET;
use crate::serve::{Failure, Swap, bind};

/// `POST /apply`。
pub(super) async fn post(request: &Request<Incoming>, web: &Web) -> Response<Body> {
    if request.method() != Method::POST {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }
    if let Some(refused) = login::refused(request, web).await {
        return refused;
    }
    let mut applied = web.applied.lock().await;
    let Some(now) = web.current.reload().await else {
        return empty(StatusCode::INTERNAL_SERVER_ERROR);
    };
    let napcat = rebind(now.port, applied.0, Failure::PortInUse).await;
    let pages = rebind(now.web, applied.1, Failure::WebPortInUse).await;
    let (napcat, pages) = match (napcat, pages) {
        (Ok(napcat), Ok(pages)) => (napcat, pages),
        (Err(failure), _) | (_, Err(failure)) => return refused(&failure),
    };
    if let Some((listener, port)) = napcat {
        web.listen.store(port, Ordering::Relaxed);
        hand(web, Swap::Napcat(listener));
    }
    if let Some((listener, port)) = pages {
        web.port.store(port, Ordering::Relaxed);
        hand(web, Swap::Web(listener));
    }
    *applied = (now.port, now.web);
    let (listen, port) = (
        web.listen.load(Ordering::Relaxed),
        web.port.load(Ordering::Relaxed),
    );
    tracing::info!(target: TARGET, listen, web = port, "applied");
    reply(StatusCode::OK, &json!({"listen": listen, "web": port}))
}

/// 配置里写的 `wanted` 和上一次照的 `before` 不一样的，开 `wanted`：交回新的监听和实际的端口；一样的交回空的。
async fn rebind(
    wanted: u16,
    before: u16,
    in_use: fn(u16) -> Failure,
) -> Result<Option<(TcpListener, u16)>, Failure> {
    if wanted == before {
        return Ok(None);
    }
    bind(wanted, in_use).await.map(Some)
}

/// 新端口开不了：被占的回 409 和是哪个端口，别的回 500。都记一行运行日志。
fn refused(failure: &Failure) -> Response<Body> {
    match failure {
        Failure::PortInUse(port) | Failure::WebPortInUse(port) => {
            tracing::warn!(target: TARGET, port, "apply port in use");
            reply(StatusCode::CONFLICT, &json!({"in_use": port}))
        }
        other => {
            tracing::warn!(target: TARGET, failure = ?other, "apply failed");
            empty(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 把新监听交给 `serve` 换上。`serve` 不收了（桥在停）：新的随之放掉，没有别处可交。
fn hand(web: &Web, swap: Swap) {
    if web.swap.send(swap).is_err() {
        tracing::debug!(target: TARGET, "bridge stopping, listener dropped");
    }
}
