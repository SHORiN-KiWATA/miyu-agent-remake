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
    let handle = core.sessions.get(core, &session).await?.handle;
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
        let (upto, current) =
            subscribe_after(core, subscriptions, &handle, &session, after, out).await?;
        reply["upto"] = json!(upto);
        now(core, &mut reply, current.as_ref());
        return Ok((reply, Some(session)));
    }
    let current = match subscriptions.has(&session) {
        // 还是那一个（第 2 条）：不另起订阅，另要一份这一刻的。
        true => handle.current().await.ok(),
        false => {
            let Ok(subscription) = handle.subscribe().await else {
                core.sessions.forget(&session).await;
                return Err(Refusal::STOPPED);
            };
            let current = subscription.current().cloned();
            subscriptions.add(session, subscription, Vec::new(), out.clone());
            current
        }
    };
    now(core, &mut reply, current.as_ref());
    Ok((reply, None))
}

/// 回应里「当前的」三格（施工 9-6 上）：这个会话累计的用量和计数（`usage`，写法同 `usage.query` 的一行），人设的权限
/// （`permission`），还在跑的后台命令和子代理（`jobs`，照 `job.started` 的写法）。和订阅在会话 actor 的同一步里拿；会话停了、
/// 拿不到的不写。
fn now(core: &Core, reply: &mut Value, current: Option<&miyu_session::Current>) {
    let Some(current) = current else {
        return;
    };
    reply["usage"] = crate::usage::tallied(core, &current.tally);
    reply["permission"] = json!(current.permission);
    reply["jobs"] = json!(current.jobs);
    // 会话在哪个目录干活（施工 9-7 上）：之后照推过来的 `session.workspace_changed` 换。
    reply["workspace"] = json!({"cwd": current.cwd, "dirs": current.dirs});
}

/// 带 `after` 订阅（施工 3-8 六补）：总是换一个新的。原来有一个的，先等它把交给它的推送、回应都放完、拿回它的订阅，补的
/// 就不和它的交错；新的拿到了才放下旧的，这个头一直算看着（施工 7-9）。补发的那一截和新的订阅在会话 actor 的同一步里拿，
/// 在这里读完（会话照常跑，推送攒在新的订阅里），交给新的转发任务先写。交回补到哪一条，和同一步里拿的「当前的」几样。
async fn subscribe_after(
    core: &Arc<Core>,
    subscriptions: &mut Subscriptions,
    handle: &Handle,
    session: &SessionId,
    after: u64,
    out: &mpsc::Sender<String>,
) -> Result<(u64, Option<miyu_session::Current>), Refusal> {
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
    let current = subscription.current().cloned();
    subscriptions.add(session.clone(), subscription, backlog, out.clone());
    Ok((upto, current))
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
