//! `/apply`（`onebot.md` 第二条「对外的样子」「施工时定的」第 19 条，施工 O-16 补二，O-20 改）：照桥手里最新的配置换端口，不读
//! 盘。要登录令牌（`super::login`），只认 `POST`。核心推来的端口变化（第一条第 1 条）也照这里的 [`latest`] 换。
//!
//! 1. 两个端口照桥手里最新的配置（`crate::current`），和上一次照的（[`super::Web::applied`]）比，变了的开新的
//!    （[`crate::serve::bind`]）。都开上了才交给 `serve` 换掉旧的，回 `{"listen": 端口, "web": 端口}`（实际听的）；有一个开不
//!    了，回 409 `{"in_use": 端口}`，开好的放掉，两个都不换，旧的照旧开着，桥不会落到哪个端口都不听。
//! 2. 推送来了已经换好的，`/apply` 再照一次什么都不换，回实际听的；推送来时被占、没换成的，`/apply` 再试一次（第一条「施工时
//!    定的」第 39 条）。
//! 3. 已经接进来的连接（NapCat 的那一条、页面正在用的这一条）不断：换掉的只是监听。
//!
//! 同时来的几个（`/apply`、推送）锁着 `applied` 一个一个办，每个照这时手里最新的。

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
    match latest(web).await {
        Ok((listen, port)) => reply(StatusCode::OK, &json!({"listen": listen, "web": port})),
        Err(Failure::PortInUse(port) | Failure::WebPortInUse(port)) => {
            reply(StatusCode::CONFLICT, &json!({"in_use": port}))
        }
        Err(_) => empty(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// 照桥手里最新的配置换端口（模块的说明）：交回实际听的两个端口（NapCat 的、WebUI 的）。都记一行运行日志。
///
/// # Errors
///
/// 新端口被占（[`Failure::PortInUse`]、[`Failure::WebPortInUse`]）、听不了：两个都不换。
pub(crate) async fn latest(web: &Web) -> Result<(u16, u16), Failure> {
    let mut applied = web.applied.lock().await;
    let now = web.current.settings();
    let napcat = rebind(now.port, applied.0, Failure::PortInUse).await;
    let pages = rebind(now.web, applied.1, Failure::WebPortInUse).await;
    let (napcat, pages) = match (napcat, pages) {
        (Ok(napcat), Ok(pages)) => (napcat, pages),
        (Err(failure), _) | (_, Err(failure)) => {
            refused(&failure);
            return Err(failure);
        }
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
    Ok((listen, port))
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

/// 新端口开不了：记一行运行日志，被占的说是哪个端口。
fn refused(failure: &Failure) {
    match failure {
        Failure::PortInUse(port) | Failure::WebPortInUse(port) => {
            tracing::warn!(target: TARGET, port, "apply port in use");
        }
        other => tracing::warn!(target: TARGET, failure = ?other, "apply failed"),
    }
}

/// 把新监听交给 `serve` 换上。`serve` 不收了（桥在停）：新的随之放掉，没有别处可交。
fn hand(web: &Web, swap: Swap) {
    if web.swap.send(swap).is_err() {
        tracing::debug!(target: TARGET, "bridge stopping, listener dropped");
    }
}
