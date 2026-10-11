//! 各项怎么量（施工 V-1，`docs/blueprint/perf.md`）：每一项一个模块，这里放它们共用的两步：造一个会话并订阅它，
//! 在会话里说一句。

pub mod append;
pub mod large;
pub mod sessions;
pub mod size;
pub mod startup;

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::fake::Fake;
use crate::rpc::Rpc;
use crate::stats::ms;

/// 核心起来、会话说完以后，量内存之前歇多久：让刚才的活干完、刚放开的内存落定。
pub const SETTLE: Duration = Duration::from_secs(2);

/// 说了一句：这一句 `message.user` 的序号，说出去到模型收到请求、落了盘到模型收到请求、说出去到这一轮结束各用了多久（毫秒）。
#[derive(Debug, Clone, Copy)]
pub struct Said {
    /// 这一句的序号：说之前会话里有几条事件。
    pub seq: u64,
    /// 发出 `session.send` 到假模型读全请求头：含 `message.user` 落盘（同步）。
    pub request: f64,
    /// 收到 `session.send` 的回应（这一句落了盘才回）到假模型读全请求头：组装、编码、连上假模型，不含落盘（施工 V-2 中）。
    /// 核心回了就去组装，回应在路上的那一点也算在里面；请求先到的记 0。
    pub projection: f64,
    /// 发出 `session.send` 到推来 `turn.ended`。
    pub turn: f64,
}

/// 在 `cwd` 里造一个会话，订阅它，交回编号。
///
/// # Errors
///
/// 核心拒了、回应里没有编号。
pub async fn open(rpc: &mut Rpc, cwd: &Path) -> Result<String, String> {
    let created = rpc
        .call("session.create", json!({"cwd": cwd.to_string_lossy()}))
        .await?;
    let session = created["session"]
        .as_str()
        .ok_or("session.create 的回应里没有编号")?
        .to_string();
    subscribe(rpc, &session).await?;
    Ok(session)
}

/// 订阅一个会话的事件流：没在跑的核心先载入它。
///
/// # Errors
///
/// 核心拒了。
pub async fn subscribe(rpc: &mut Rpc, session: &str) -> Result<(), String> {
    rpc.call("subscribe", json!({"session": session, "stream": "events"}))
        .await
        .map(|_| ())
}

/// 在订阅着的会话里说一句带着 `marker` 的话，等这一轮结束。`marker` 每一句都不一样：假模型照它认出这一轮的请求。
///
/// # Errors
///
/// 核心拒了、模型一直没收到请求、这一轮一直不结束。
pub async fn say(
    rpc: &mut Rpc,
    fake: &mut Fake,
    session: &str,
    marker: &str,
) -> Result<Said, String> {
    let began = Instant::now();
    let sent = rpc
        .call(
            "session.send",
            json!({"session": session, "text": format!("{marker}: keep going.")}),
        )
        .await?;
    let persisted = Instant::now();
    let seq = sent["events"][0]
        .as_u64()
        .ok_or("session.send 的回应里没有序号")?;
    let arrival = fake.request_with(marker).await?;
    rpc.turn_ended(session).await?;
    Ok(Said {
        seq,
        request: ms(arrival.at.saturating_duration_since(began)),
        projection: ms(arrival.at.saturating_duration_since(persisted)),
        turn: ms(began.elapsed()),
    })
}
