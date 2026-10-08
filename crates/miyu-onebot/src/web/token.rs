//! `/token`（`onebot.md` 第二条「对外的样子」「施工时定的」第 17 条，施工 O-16 补二）：要登录令牌（`super::login`）。回桥手里
//! 最新的令牌 `{"token": "<值>"}`，没有的 `{"token": null}`；不读盘（施工 O-20：核心推来的就是最新的）。
//!
//! 令牌是桥自己的凭据，桥手里本来就有：由桥交给登录了的管理员看、复制，不经核心协议交出去（`config.md` 第九条管的是核心
//! 协议）。运行日志只记取过，不记值；回应不让缓存。

use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};
use serde_json::json;

use miyu_config::secret::Secret;
use miyu_webserve::respond::{Body, empty};

use super::{Web, login, reply};
use crate::TARGET;

/// `GET /token`。
pub(super) async fn get(request: &Request<Incoming>, web: &Web) -> Response<Body> {
    if request.method() != Method::GET {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }
    if let Some(refused) = login::refused(request, web).await {
        return refused;
    }
    let token = web.current.token();
    tracing::info!(target: TARGET, "web token read");
    reply(
        StatusCode::OK,
        &json!({"token": token.as_ref().map(Secret::expose)}),
    )
}
