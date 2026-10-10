//! `/status`（`onebot.md` 第二条「对外的样子」，施工 O-16）：只读，要登录令牌（`super::login`）。
//!
//! 回 `{"napcat": {...}, "listen": 端口, "web": 端口, "token": "set" | "none", "platform": "qq"}`：照桥手里最新的配置答，
//! 不读盘（施工 O-20）。
//!
//! - `napcat`：连着的号里最小的那一个，`connected`、`self_id`，问到了是哪个实现的再带 `implementation`、`version`；没连着的
//!   只有 `connected: false`。
//! - `listen`、`web`：实际听的两个端口（换过的照换过的：推送来的、`/apply` 的）。
//! - `token`：桥手里有令牌的 `set`，没有的 `none`（`crate::current`）。没写引用、引用取不到，核心都不交，分不出（第一条
//!   「施工时定的」第 41 条：原来的 `missing` 没了）。
//! - `platform`：桥的平台名（[`crate::onebot::PLATFORM`]）。「终端管理员与白名单成员」页照它拼 `qq:<号>`，平台的名字还是只写一处
//!   （O-17，第二条「施工时定的」第 26 条）。
//!
//! 除了 `web`，这几格和后台页的 `status` 方法是同一份（[`shared`]，施工 O-28 上，第一条「后台页」第 2 条）。

use std::sync::atomic::Ordering;

use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};
use serde_json::{Map, Value, json};

use miyu_webserve::respond::{Body, empty};

use super::{Web, login, reply};
use crate::onebot::PLATFORM;

/// `GET /status`。
pub(super) async fn get(request: &Request<Incoming>, web: &Web) -> Response<Body> {
    if request.method() != Method::GET {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }
    if let Some(refused) = login::refused(request, web).await {
        return refused;
    }
    let mut status = shared(web);
    status.insert("web".into(), json!(web.port.load(Ordering::Relaxed)));
    reply(StatusCode::OK, &Value::Object(status))
}

/// `/status` 和后台页的 `status` 方法共用的几格（施工 O-28 上）：`napcat`、`listen`、`token`、`platform`，照桥手里最新的。
pub(crate) fn shared(web: &Web) -> Map<String, Value> {
    let token = match web.current.token() {
        Some(_) => "set",
        None => "none",
    };
    let mut status = Map::new();
    status.insert("napcat".into(), web.bots.napcat());
    status.insert("listen".into(), json!(web.listen.load(Ordering::Relaxed)));
    status.insert("token".into(), json!(token));
    status.insert("platform".into(), json!(PLATFORM));
    status
}
