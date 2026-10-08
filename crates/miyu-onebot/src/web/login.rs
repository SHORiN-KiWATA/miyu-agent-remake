//! 核对登录令牌（`onebot.md` 第二条「对外的样子」，施工 O-16；补二从 `status.rs` 挪出来，`/status`、`/token`、`/apply` 共用）：
//! 要 `Authorization: Bearer <登录令牌>`。桥拿这个令牌去和核心握手，核心认了才放行，不认 401；验过的记一阵
//! （[`super::Checked`]，只记哈希），这一阵里不再握手。连不上核心 502。

use std::time::Instant;

use hyper::body::Incoming;
use hyper::header;
use hyper::{Request, Response, StatusCode};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use miyu_webserve::respond::{Body, empty};

use super::Web;
use crate::TARGET;

/// 核对 `request` 带的登录令牌：放行的交回空的；不放行的交回该回的（401、502）。
pub(super) async fn refused(request: &Request<Incoming>, web: &Web) -> Option<Response<Body>> {
    let Some(login) = bearer(request) else {
        return Some(empty(StatusCode::UNAUTHORIZED));
    };
    if web.checked.fresh(&login, Instant::now()) {
        return None;
    }
    match verify(web, &login).await {
        Ok(true) => {
            tracing::info!(target: TARGET, "web login accepted");
            web.checked.remember(&login, Instant::now());
            None
        }
        Ok(false) => {
            tracing::warn!(target: TARGET, "web login refused");
            Some(empty(StatusCode::UNAUTHORIZED))
        }
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "core unreachable");
            Some(empty(StatusCode::BAD_GATEWAY))
        }
    }
}

/// `Authorization: Bearer <登录令牌>`：方案不分大小写，令牌去掉前后空白不能是空的。
fn bearer(request: &Request<Incoming>) -> Option<String> {
    let value = request
        .headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then(|| token.to_string())
}

/// 拿登录令牌 `login` 和核心握手：认了交回真，拒了交回假。连不上、没回、断了：原因。照 `bridge.json` 的
/// `call_timeout_seconds` 等回应。握完就关，不留连接。
async fn verify(web: &Web, login: &str) -> Result<bool, String> {
    let connection = miyu_ipc::connect_or_start_bare(&web.root, || (web.core)())
        .await
        .map_err(|error| error.to_string())?;
    let (read, mut write) = tokio::io::split(connection);
    let mut lines = BufReader::new(read);
    let hello = json!({
        "jsonrpc": "2.0",
        "id": "status",
        "method": "hello",
        "params": {
            "protocol": [1, 1],
            "head": {"kind": "onebot", "version": env!("CARGO_PKG_VERSION")},
            "login": login,
        },
    });
    let shaken = async {
        write.write_all(format!("{hello}\n").as_bytes()).await?;
        write.flush().await?;
        let mut line = String::new();
        lines.read_line(&mut line).await?;
        Ok::<_, std::io::Error>(line)
    };
    let line = tokio::time::timeout(web.tuning.call_timeout(), shaken)
        .await
        .map_err(|_| "no answer to hello".to_string())?
        .map_err(|error| error.to_string())?;
    let reply: Value = serde_json::from_str(&line).map_err(|_| "core disconnected".to_string())?;
    Ok(reply.get("result").is_some())
}
