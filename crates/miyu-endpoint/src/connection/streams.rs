//! 订阅哪一个流、订阅会话的事件流（`docs/blueprint/protocol.md`「`subscribe`、`unsubscribe`」「补发」）：施工 9-4 补从
//! `connection.rs` 挪出来（那一份到了 500 行）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use miyu_kernel::id::SessionId;
use miyu_session::Handle;

use crate::Core;
use crate::refusal::Refusal;
use crate::subscriptions::Subscriptions;
use crate::wire::Request;

/// `subscribe`、`unsubscribe` 的参数。
#[derive(Debug, Deserialize)]
struct StreamParams {
    #[serde(default)]
    session: Option<String>,
    stream: String,
}

/// 订阅哪一个流。
pub(super) enum Stream {
    /// 一个会话的事件流。
    Events(SessionId),
    /// 配置的推送（施工 8-4）。
    Config,
    /// 会话列表的推送（施工 9-5）。
    Sessions,
    /// 扩展的状态的推送（施工 9-4 补）。
    Extensions,
}

/// 会话 `session` 的 `session.created`：日志第一条，在阻塞线程里读一行。读不了、第一条不是它的是没有。
async fn created_of(
    core: &Core,
    session: &SessionId,
) -> Option<miyu_kernel::event::SessionCreated> {
    let dir = core.root.session_dir(&core.admin, session);
    let first = tokio::task::spawn_blocking(move || miyu_store::log::first_event(&dir))
        .await
        .ok()?
        .ok()?;
    match first.body {
        miyu_kernel::event::Body::SessionCreated(created) => Some(created),
        _ => None,
    }
}

/// 订阅会话的事件流：没在跑的照样先载入；已经订阅着的，还是那一个。回应带会话的限额（施工 6-3 补）、会话接下来请求的模型
/// （施工 8-10，一个都没有的不写）：订阅着的也从会话表拿，在跑的直接用，不多载入。写了 `after` 的，先补之前的事件（[`subscribe_after`]）。交回回应，和回应经哪个订阅写出去：
/// 补了的经新的订阅，排在补的后面；别的直接写。
pub(super) async fn subscribe(
    core: &Arc<Core>,
    subscriptions: &mut Subscriptions,
    request: &Request,
    session: SessionId,
    out: &mpsc::Sender<String>,
) -> Result<(Value, Option<SessionId>), Refusal> {
    let after = after_of(request)?;
    let handle = core.sessions.get(core, &session, None, None).await?.handle;
    let mut reply = json!({"limits": handle.limits()});
    if let Some(model) = crate::models::next(&handle.next()) {
        reply["model"] = model;
    }
    // 会话用哪个人格（施工 P-1 下）、哪个预设（施工 P-2 上）：照日志第一条 `session.created` 读，以前的日志没有的不写。
    if let Some(created) = created_of(core, &session).await {
        if let Some(persona) = created.persona {
            reply["persona"] = json!(persona);
        }
        if let Some(preset) = created.preset {
            reply["preset"] = json!(preset);
        }
    }
    // 当前的待办（施工 D-3）：没有的不写；之后变了照推送的 `todos.changed`。
    let todos = handle.todos();
    if !todos.is_empty() {
        reply["todos"] = json!(todos);
    }
    if let Some(after) = after {
        let upto = subscribe_after(core, subscriptions, &handle, &session, after, out).await?;
        reply["upto"] = json!(upto);
        return Ok((reply, Some(session)));
    }
    if !subscriptions.has(&session) {
        let Ok(subscription) = handle.subscribe().await else {
            core.sessions.forget(&session).await;
            return Err(Refusal::STOPPED);
        };
        subscriptions.add(session, subscription, Vec::new(), out.clone());
    }
    Ok((reply, None))
}

/// 带 `after` 订阅（施工 3-8 六补）：总是换一个新的。原来有一个的，先等它把交给它的推送、回应都放完、拿回它的订阅，补的
/// 就不和它的交错；新的拿到了才放下旧的，这个头一直算看着（施工 7-9）。补发的那一截和新的订阅在会话 actor 的同一步里拿，
/// 在这里读完（会话照常跑，推送攒在新的订阅里），交给新的转发任务先写。交回补到哪一条。
async fn subscribe_after(
    core: &Arc<Core>,
    subscriptions: &mut Subscriptions,
    handle: &Handle,
    session: &SessionId,
    after: u64,
    out: &mpsc::Sender<String>,
) -> Result<u64, Refusal> {
    let old = subscriptions.take(session).await;
    let Ok((subscription, backlog)) = handle.subscribe_after(after).await else {
        core.sessions.forget(session).await;
        return Err(Refusal::STOPPED);
    };
    drop(old);
    let upto = backlog.upto();
    let backlog = backlog.read().await.map_err(|error| {
        tracing::warn!(target: "miyu::endpoint", session = session.as_str(), error = %error, "replay not read");
        Refusal::BROKEN
    })?;
    subscriptions.add(session.clone(), subscription, backlog, out.clone());
    Ok(upto)
}

/// `subscribe` 的 `after`（施工 3-8 六补）：可以不写，写 `null` 等于没写；写了要是非负整数，别的 `bad_params`。
fn after_of(request: &Request) -> Result<Option<u64>, Refusal> {
    match request.params.get("after") {
        None | Some(Value::Null) => Ok(None),
        Some(after) => after.as_u64().map(Some).ok_or(Refusal::BAD_PARAMS),
    }
}

/// 订阅的参数：`events` 带会话编号；`config`、`sessions`、`extensions` 不带会话、不带 `after`，带了是参数不对（施工 8-4、9-5、
/// 9-4 补）。
pub(super) fn stream_of(request: &Request) -> Result<Stream, Refusal> {
    let params: StreamParams =
        serde_json::from_value(request.params.clone()).map_err(|_| Refusal::BAD_PARAMS)?;
    match (params.stream.as_str(), params.session) {
        ("events", Some(session)) => SessionId::parse(&session)
            .map(Stream::Events)
            .map_err(|_| Refusal::BAD_PARAMS),
        ("config", None) if request.params.get("after").is_none() => Ok(Stream::Config),
        ("sessions", None) if request.params.get("after").is_none() => Ok(Stream::Sessions),
        ("extensions", None) if request.params.get("after").is_none() => Ok(Stream::Extensions),
        _ => Err(Refusal::BAD_PARAMS),
    }
}
