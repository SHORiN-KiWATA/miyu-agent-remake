//! 在连上了的连接上说话的几样（施工 3-9 下写在 `ask/talk.rs` 里，施工 4-7 下挪出来，`miyu undo` 也用）：握手、发一条
//! 请求等回应、找最新的那个一次性会话。出了问题的，照界面语言说清楚，交回退出码（`22-命令行.md` 第二节）。

use std::io::Write;

use serde_json::{Value, json};

use crate::exit;
use crate::language::Language;
use crate::rpc::Rpc;
use crate::shown::say;

/// 握手：协议的版本、是哪个头、界面语言（核心的拒绝照它说）、有没有人能当场回答、本机令牌。交回核心的回应：里面有这台
/// 机器的沙盒能不能用（施工 5-4 下）。
///
/// # Errors
///
/// 被拒绝、核心断开：说清楚，交回退出码。
pub(crate) async fn hello(
    rpc: &mut Rpc,
    token: &str,
    language: &Language,
    input: bool,
    err: &mut dyn Write,
) -> Result<Value, u8> {
    let hello = json!({
        "protocol": [1, 1],
        "head": {"kind": "cli", "version": env!("CARGO_PKG_VERSION")},
        "locale": language.locale(),
        "caps": {"input": input},
        "token": token,
    });
    request(rpc, "hello", hello, language, err).await
}

/// 握手以后说哪种话（施工 8-2）：照回应的 `language`；老的核心没回的，还是 `asked`（握手以前的那一种）。
pub(crate) fn spoken(hello: &Value, asked: Language) -> Language {
    hello["language"]
        .as_str()
        .and_then(Language::from_code)
        .unwrap_or(asked)
}

/// 握手的回应说配置里有几处错误（施工 8-2）：没有的是 0。
pub(crate) fn config_errors(hello: &Value) -> u64 {
    hello["config_errors"].as_u64().unwrap_or(0)
}

/// 握手的回应说沙盒用不了：交回原因（协议上的写法）；能用的、没说的（老的核心）是空的。
pub(crate) fn unsandboxed(hello: &Value) -> Option<String> {
    let sandbox = &hello["sandbox"];
    (sandbox["usable"] == json!(false))
        .then(|| sandbox["reason"].as_str().unwrap_or_default().to_string())
}

/// 最新的那个一次性会话（`miyu ask --continue` 接的就是它）。一个都没有的，说「还没有 miyu ask 开过的会话」。
///
/// # Errors
///
/// 一个都没有、被拒绝、核心断开：说清楚，交回退出码。
pub(crate) async fn latest_oneshot(
    rpc: &mut Rpc,
    language: &Language,
    err: &mut dyn Write,
) -> Result<String, u8> {
    let params = json!({"oneshot": true, "limit": 1});
    let result = request(rpc, "session.list", params, language, err).await?;
    match result["sessions"][0]["session"].as_str() {
        Some(session) => Ok(session.to_string()),
        None => {
            say(err, &language.no_oneshot());
            Err(exit::ERROR)
        }
    }
}

/// 发一条请求，等回应，交回 `result`。被拒绝的、核心断开的，说清楚，交回退出码。
///
/// # Errors
///
/// 被拒绝、核心断开、写不出去。
pub(crate) async fn request(
    rpc: &mut Rpc,
    method: &str,
    params: Value,
    language: &Language,
    err: &mut dyn Write,
) -> Result<Value, u8> {
    request_saying(rpc, method, params, language, err, |error| {
        language.refused(error["message"].as_str().unwrap_or_default())
    })
    .await
}

/// 同 [`request`]，只是被拒绝时说的那一句由 `refused` 照核心回的 `error` 写：原话在 `message`，原因码和多带的几格在 `data`
/// （施工 3-9 三补：附件传不上，先说是哪个文件；施工 R-3 再补：记忆太长的照 `data` 说几个字、上限几个）。
///
/// # Errors
///
/// 被拒绝、核心断开、写不出去。
pub(crate) async fn request_saying(
    rpc: &mut Rpc,
    method: &str,
    params: Value,
    language: &Language,
    err: &mut dyn Write,
    refused: impl FnOnce(&Value) -> String,
) -> Result<Value, u8> {
    match rpc.call(method, params).await {
        Ok(Some(reply)) => match reply.get("error") {
            None => Ok(reply["result"].clone()),
            Some(error) => {
                say(err, &refused(error));
                Err(exit::ERROR)
            }
        },
        Ok(None) => {
            say(err, &language.disconnected());
            Err(exit::ERROR)
        }
        Err(error) => {
            say(err, &error.to_string());
            Err(exit::ERROR)
        }
    }
}

#[cfg(test)]
mod tests;
