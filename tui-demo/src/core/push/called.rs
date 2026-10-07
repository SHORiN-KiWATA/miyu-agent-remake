//! 一次请求记了什么（`model.called`）：用量、金额、发出去了没有、速度、出错。

use serde_json::Value;

use super::{CallError, Compaction, Push, Usage};
use crate::core::Cost;

/// 读一条 `model.called`，照先后放进 `out`。
pub(super) fn read(body: &Value, out: &mut Vec<Push>) {
    let usage = &body["usage"];
    let n = |k: &str| usage[k].as_u64().unwrap_or_default();
    let spent = usage.is_object().then(|| Usage {
        uncached: n("uncached"),
        cache_read: n("cache_read"),
        cache_write: n("cache_write"),
        output: n("output"),
        aux: 0,
    });
    // 辅助请求（回顾）：只算用量，不动上下文、缓存、速度，出错也不算这一轮的错。
    if body["purpose"].as_str().is_some_and(|p| !p.is_empty()) {
        out.extend(spent.map(Push::AuxUsage));
        out.extend(spent.map(|_| Push::Billed(Cost::read(&body["cost"]))));
        return;
    }
    out.extend(spent.map(Push::Usage));
    out.extend(spent.map(|_| Push::Billed(Cost::read(&body["cost"]))));
    let summary = !body["compaction"].is_null();
    if !body["request"].is_null() {
        out.push(Push::Sent {
            seen: body["seen"].as_u64().unwrap_or_default(),
            changed: body["first_difference"].is_object(),
            summary,
        });
    }
    let output = usage["output"].as_u64().unwrap_or_default();
    let (duration, first) = (
        body["duration_ms"].as_u64(),
        body["first_token_ms"].as_u64(),
    );
    if let (Some(duration), Some(first)) = (duration, first)
        && output > 0
        && duration > first
    {
        out.push(Push::Speed {
            output,
            ms: duration - first,
        });
    }
    // 摘要请求出错说压缩失败，不算这一轮的出错（蓝图「正文」第 9 条）。
    let error = || CallError::read(&body["error"]);
    match (body["result"].as_str(), summary) {
        (Some("error"), true) => out.push(Push::Compaction(Compaction::Failed(error()))),
        (Some("error"), false) => {
            // 试的是哪个端点、模型：换端点那一行的「原来的」照它写（核心 8-9）。
            if let (Some(endpoint), Some(model)) =
                (body["endpoint"].as_str(), body["model"].as_str())
            {
                out.push(Push::Tried {
                    endpoint: endpoint.to_string(),
                    model: model.to_string(),
                });
            }
            out.push(Push::CallFailed(error()));
        }
        (Some("ok"), false) => out.push(Push::CallOk),
        _ => {}
    }
}
