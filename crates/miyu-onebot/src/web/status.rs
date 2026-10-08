//! `/status`（`onebot.md` 第二条「对外的样子」，施工 O-16）：只读，要登录令牌（`super::login`）。
//!
//! 回 `{"napcat": {...}, "listen": 端口, "web": 端口, "token": "set" | "none" | "missing"}`：
//!
//! - `napcat`：连着的号里最小的那一个，`connected`、`self_id`，问到了是哪个实现的再带 `implementation`、`version`；没连着的
//!   只有 `connected: false`。
//! - `listen`、`web`：实际听的两个端口（`/apply` 换过的照换过的）。
//! - `token`：每次重读一次配置（`crate::current`，桥手里的令牌跟着换上），照桥手里的说：取到了、没写引用、写了引用取不到
//!   （补二：去掉了 `restart_needed`，改了当场生效，第二条「施工时定的」第 6 条）。

use std::sync::atomic::Ordering;

use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};
use serde_json::{Map, Value, json};

use miyu_webserve::respond::{Body, empty};

use super::{Web, login, reply};
use crate::settings::Token;

/// `GET /status`。
pub(super) async fn get(request: &Request<Incoming>, web: &Web) -> Response<Body> {
    if request.method() != Method::GET {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }
    if let Some(refused) = login::refused(request, web).await {
        return refused;
    }
    web.current.reload().await;
    let token = match web.current.token() {
        Token::Set(_) => "set",
        Token::Unset => "none",
        Token::Missing => "missing",
    };
    let status = json!({
        "napcat": napcat(web),
        "listen": web.listen.load(Ordering::Relaxed),
        "web": web.port.load(Ordering::Relaxed),
        "token": token,
    });
    reply(StatusCode::OK, &status)
}

/// NapCat 连没连上、是哪个号、哪个实现。
fn napcat(web: &Web) -> Value {
    let Some((bot, link)) = web.bots.first() else {
        return json!({"connected": false});
    };
    let mut napcat = Map::new();
    napcat.insert("connected".into(), json!(true));
    napcat.insert("self_id".into(), json!(bot.to_string()));
    if let Some(peer) = link.peer.get() {
        napcat.insert("implementation".into(), json!(peer.implementation));
        napcat.insert("version".into(), json!(peer.version));
    }
    Value::Object(napcat)
}
