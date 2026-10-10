//! 桥当提供者（施工 O-26，`onebot.md` 第一条「提供者和不说话」第 1、2 条，`providers.md`）：起来时经 `provide` 登记桥的工具，
//! 答核心反向发来的请求。
//!
//! - 登记（[`provide`]）：经并着发的调用口发，写出去就交回等回应、记运行日志的那一段（「施工时定的」第 139 条）：核心一个连接的
//!   请求照先后一条条办，先写出去的先登记；回应不等，核心那一头慢了、不答，桥照样起来。被拒了只记一行 `ERROR`，话照说。
//! - 答请求（[`heard`]）：读的一头读到一行就问它（`core.rs` 的 `read_lines`），不交给跟核心的那一头：那一头手上可能正等着别的
//!   回应，核心那边的回合在等这一次调用（第 140 条）。回应另起一个小任务写，读的一头接着读。后台页调的方法（`method.call`，
//!   施工 O-28 上）也从这里交给 `super::methods` 答。
//! - 撤回、禁言、戳一戳（施工 O-31，「平台工具（一）」第 2 条）不在这里答：要投影、群成员的缓存、机器人号的连接，原样交给跟核心
//!   的那一头（`route/acting.rs`），答完了照 [`Answerer`] 写回去。

use serde_json::{Value, json};

use super::caller::{Caller, Writer, write_line};
use crate::TARGET;
use crate::core::methods::Methods;
use crate::core::{Gone, reason};
use crate::rules::Tools;

/// 核心说的一行是哪一种。
#[derive(Debug)]
pub(super) enum Heard {
    /// 核心发来的请求（带 `id`、`method`）：交回给它的回应。
    Asked(Value),
    /// `tool.cancel`（通知）：记了一行调试日志，没有别的要做。
    Cancelled,
    /// 别的（推送、对桥的请求的回应）：原样交回，照旧走。
    Other(Value),
}

/// 核心说的一行 `message` 分成哪一种；请求照 `tools` 答好（「提供者和不说话」第 2 条），`method.call` 照 `methods` 答（施工
/// O-28 上，「后台页」第 2 条）。平台工具（一）的 `tool.call` 原样交回（施工 O-31）。
pub(super) fn heard(tools: &Tools, methods: &Methods, message: Value) -> Heard {
    let Some(method) = message["method"].as_str() else {
        return Heard::Other(message);
    };
    if message.get("id").is_none() {
        if method != "tool.cancel" {
            return Heard::Other(message);
        }
        let params = &message["params"];
        tracing::debug!(target: TARGET, session = %params["session"], call = %params["call_id"], "tool cancel");
        return Heard::Cancelled;
    }
    let id = message["id"].clone();
    if method == "method.call" {
        return Heard::Asked(methods.answer(id, &message["params"]));
    }
    if method != "tool.call" {
        tracing::warn!(target: TARGET, method, "core request not understood");
        let error = json!({"code": -32601, "message": "unknown_method", "data": {"reason": "unknown_method"}});
        return Heard::Asked(json!({"jsonrpc": "2.0", "id": id, "error": error}));
    }
    let params = &message["params"];
    let tool = params["tool"].as_str().unwrap_or_default();
    if tools.routed(tool) {
        return Heard::Other(message);
    }
    let result = tools.call(tool);
    if result["error"] == true {
        tracing::warn!(target: TARGET, tool, "unknown tool called");
    } else {
        tracing::debug!(target: TARGET, tool, session = %params["session"], "tool called");
    }
    Heard::Asked(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

/// 往核心写 `tool.call` 回应的一头（施工 O-31）：交给跟核心的那一头答的平台工具，任务办完了照它写，写法同 [`heard`] 答的。
#[derive(Clone)]
pub(crate) struct Answerer(Writer);

impl Answerer {
    /// 照写的一头 `writer`。
    pub(super) fn new(writer: &Writer) -> Answerer {
        Answerer(std::sync::Arc::clone(writer))
    }

    /// 答编号是 `id` 的那一次调用：结果是 `result`（`{blocks, error}`）。另起小任务写，同 [`reply`]。
    pub(crate) fn answer(&self, id: &Value, result: Value) {
        reply(
            &self.0,
            json!({"jsonrpc": "2.0", "id": id, "result": result}),
        );
    }
}

/// 写一条回应 `reply`：另起一个小任务，读的一头不等它。写不出去的是核心关了管道：读的一头接着读到头，桥照「怎么走」第 11 条停下。
pub(super) fn reply(writer: &Writer, reply: Value) {
    let writer = std::sync::Arc::clone(writer);
    tokio::spawn(async move {
        if write_line(&writer, &reply).await.is_err() {
            tracing::debug!(target: TARGET, "answer not written, core closed");
        }
    });
}

/// 登记 `tools`（「提供者和不说话」第 1 条）：经 `caller` 发 `provide`，写出去就交回等回应、记运行日志的那一段（交给 `serve`
/// 的任务组）。写不出去的（核心关了管道）交回空的：跟核心的那一头接着读到头，照「怎么走」第 11 条停下。
pub(crate) async fn provide(
    caller: &Caller,
    tools: &Tools,
) -> Option<impl Future<Output = ()> + use<>> {
    let answer = caller.send("provide", tools.provided()).await.ok()?;
    Some(async move {
        match answer.wait().await {
            Ok(reply) => match reason(&reply) {
                None => {
                    let count = reply["result"]["tools"].as_u64().unwrap_or_default();
                    tracing::info!(target: TARGET, count, "tools provided");
                }
                Some(reason) => {
                    let detail = &reply["error"]["data"];
                    tracing::error!(target: TARGET, reason, detail = %detail, "tools not provided");
                }
            },
            Err(Gone) => {
                // 核心关了管道：跟核心的那一头接着读到头，桥停下。
            }
        }
    })
}

#[cfg(test)]
mod tests;
