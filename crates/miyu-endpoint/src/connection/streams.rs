//! 订阅哪一个流、订阅会话的事件流（`docs/blueprint/protocol.md`「`subscribe`、`unsubscribe`」「补发」）：施工 9-4 补从
//! `connection.rs` 挪出来（那一份到了 500 行）。会话的视图流（施工 9-8 下）也在这里订。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use miyu_kernel::id::SessionId;
use miyu_session::Handle;

use crate::Core;
use crate::hello::Shaken;
use crate::refusal::Refusal;
use crate::subscriptions::{MemoryAt, Subscriptions, Target, View};
use crate::view::status::Fixed;
use crate::wire::Request;

/// `subscribe`、`unsubscribe` 的参数。
#[derive(Debug, Deserialize)]
struct StreamParams {
    #[serde(default)]
    session: Option<String>,
    /// 记忆日志的流照人格找那一间（施工 R-12 上）。
    #[serde(default)]
    persona: Option<String>,
    stream: String,
}

/// 订阅哪一个流。
pub(super) enum Stream {
    /// 一个会话的事件流。
    Events(SessionId),
    /// 一个会话的视图流（施工 9-8 下）。
    View(SessionId),
    /// 配置的推送（施工 8-4）。
    Config,
    /// 会话列表的推送（施工 9-5）。
    Sessions,
    /// 扩展的状态的推送（施工 9-4 补）。
    Extensions,
    /// 一间的记忆日志（施工 R-12 上，`memory.md`「协议」）。
    Memory(MemoryAt),
}

/// 会话 `session` 的 `session.created`：日志第一条，在阻塞线程里读一行。读不了、第一条不是它的是没有。
async fn created_of(
    core: &Core,
    session: &SessionId,
) -> Option<miyu_kernel::event::SessionCreated> {
    let owner = core.sessions.owner(core, session).await?;
    let dir = core.root.session_dir(&owner, session);
    let first = tokio::task::spawn_blocking(move || miyu_store::log::first_event(&dir))
        .await
        .ok()?
        .ok()?;
    match first.body {
        miyu_kernel::event::Body::SessionCreated(created) => Some(created),
        _ => None,
    }
}

/// `subscribe`：照 `stream` 订阅哪一个流，交回回应和它经哪个订阅写出去。`shaken` 是握手时记下的：配置、扩展的状态、视图流
/// 照它挑语言。
pub(super) async fn subscribe(
    core: &Arc<Core>,
    subscriptions: &mut Subscriptions,
    request: &Request,
    shaken: Option<Shaken>,
    out: &mpsc::Sender<String>,
) -> (Result<Value, Refusal>, Option<Target>) {
    let session_reply = |done: Result<(Value, Option<SessionId>), Refusal>| match done {
        Ok((result, target)) => (Ok(result), target.map(Target::Session)),
        Err(refusal) => (Err(refusal), None),
    };
    match (stream_of(request), shaken) {
        (Ok(Stream::Config), _) => {
            let system = shaken.map_or("en", Shaken::system);
            subscriptions.add_config(core, system, out);
            (Ok(json!({})), None)
        }
        (Ok(Stream::Sessions), _) => match subscriptions.add_sessions(core, out).await {
            Ok(result) => (Ok(result), Some(Target::Sessions)),
            Err(refusal) => (Err(refusal), None),
        },
        (Ok(Stream::Extensions), Some(shook)) => {
            let listed = subscriptions.add_extensions(core, shook, out);
            (Ok(listed), Some(Target::Extensions))
        }
        (Ok(Stream::Events(session)), _) => {
            session_reply(events(core, subscriptions, request, session, out).await)
        }
        (Ok(Stream::View(session)), Some(shook)) => {
            session_reply(view(core, subscriptions, session, shook, out).await)
        }
        (Ok(Stream::Memory(at)), _) => match memory(core, subscriptions, request, at, out).await {
            Ok((result, at)) => (Ok(result), Some(Target::Memory(at))),
            Err(refusal) => (Err(refusal), None),
        },
        (Ok(Stream::Extensions | Stream::View(_)), None) => (Err(Refusal::HELLO_FIRST), None),
        (Err(refusal), _) => (Err(refusal), None),
    }
}

/// 订阅会话的事件流：没在跑的照样先载入；已经订阅着的，还是那一个。回应带会话的限额（施工 6-3 补）、会话接下来请求的模型
/// （施工 8-10，一个都没有的不写）：订阅着的也从会话表拿，在跑的直接用，不多载入。写了 `after` 的，先补之前的事件（[`subscribe_after`]）。交回回应，和回应经哪个订阅写出去：
/// 补了的经新的订阅，排在补的后面；别的直接写。
async fn events(
    core: &Arc<Core>,
    subscriptions: &mut Subscriptions,
    request: &Request,
    session: SessionId,
    out: &mpsc::Sender<String>,
) -> Result<(Value, Option<SessionId>), Refusal> {
    let after = after_of(request)?;
    let handle = core.sessions.get(core, &session).await?.handle;
    let mut reply = head(core, &handle, &session).await;
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
            // 原来是视图流的，先拿掉它（施工 9-8 下）：等它放完交给它的回应。
            drop(subscriptions.take(&session).await);
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

/// 订阅会话的视图流（施工 9-8 下，`view.md`「视图流」）：总是换一个新的。和事件流的回应一样带限额、模型、人格、预设、待办、
/// 「当前的」几格，另带最新一页的条目（同 `view.page {view: true}`，字照这个连接的语言）。那一页和新的订阅在会话 actor 的
/// 同一步里拿，之后的推送接着喂同一台投影，不重不漏。回应经新的订阅写出去，排在所有推送前面。
async fn view(
    core: &Arc<Core>,
    subscriptions: &mut Subscriptions,
    session: SessionId,
    shaken: Shaken,
    out: &mpsc::Sender<String>,
) -> Result<(Value, Option<SessionId>), Refusal> {
    let handle = core.sessions.get(core, &session).await?.handle;
    let mut reply = head(core, &handle, &session).await;
    let owner = core
        .sessions
        .owner(core, &session)
        .await
        .ok_or(Refusal::NOT_FOUND)?;
    let old = subscriptions.take(&session).await;
    let Ok((subscription, backlog)) = handle.subscribe_after(0).await else {
        core.sessions.forget(&session).await;
        return Err(Refusal::STOPPED);
    };
    drop(old);
    let events = backlog.read().await.map_err(|error| {
        tracing::warn!(target: "miyu::endpoint", session = session.as_str(), error = %error, "view not read");
        Refusal::BROKEN
    })?;
    let current = subscription.current().cloned();
    let language = shaken.now(core).language;
    let resources = core.resources.clone();
    let blobs = miyu_store::blob::Blobs::new(core.root.blobs(&owner));
    let (page, projector) = tokio::task::spawn_blocking(move || {
        crate::view::texts(&resources, language)
            .map(|texts| crate::view::newest(&events, texts, blobs))
    })
    .await
    .map_err(|_| Refusal::INTERNAL)??;
    // 那一页的 `jobs`（切点前派出、这一页报完的）条目里已经写了；回应的 `jobs` 是「当前的」在跑的任务（[`now`]）。
    if let (Some(reply), Value::Object(mut page)) = (reply.as_object_mut(), page) {
        page.remove("jobs");
        reply.extend(page);
    }
    now(core, &mut reply, current.as_ref());
    // 会话状态（施工 9-8 补上）：人格、预设照回应里已经读出来的。
    let fixed = Fixed {
        persona: reply.get("persona").cloned(),
        preset: reply.get("preset").cloned(),
    };
    let view = View::start(
        Arc::clone(core),
        shaken,
        language,
        projector,
        handle.clone(),
        fixed,
        current,
    )
    .await;
    reply["status"] = view.status().clone();
    subscriptions.add_view(session.clone(), subscription, view, out.clone());
    Ok((reply, Some(session)))
}

/// 两种流的回应都带的：会话的限额、接下来请求的模型、人格、预设、待办。
async fn head(core: &Core, handle: &Handle, session: &SessionId) -> Value {
    let mut reply = json!({"limits": handle.limits()});
    if let Some(model) = crate::models::next(&handle.next()) {
        reply["model"] = model;
    }
    // 会话用哪个人格（施工 P-1 下）、哪个预设（施工 P-2 上）：照日志第一条 `session.created` 读，以前的日志没有的不写。
    if let Some(created) = created_of(core, session).await {
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
    reply
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

/// 订阅一间的记忆日志（施工 R-12 上）：找哪一间、补什么、之后的照 `crate::memory::follow`；回应 `{"upto"}` 交给新的转发任务，
/// 排在补的后面。
async fn memory(
    core: &Arc<Core>,
    subscriptions: &mut Subscriptions,
    request: &Request,
    at: MemoryAt,
    out: &mpsc::Sender<String>,
) -> Result<(Value, MemoryAt), Refusal> {
    let after = after_of(request)?;
    let following = crate::memory::follow(core, &at, after).await?;
    let upto = following.upto;
    subscriptions.add_memory(at.clone(), following, out);
    Ok((json!({ "upto": upto }), at))
}

/// `subscribe` 的 `after`（施工 3-8 六补）：可以不写，写 `null` 等于没写；写了要是非负整数，别的 `bad_params`。
fn after_of(request: &Request) -> Result<Option<u64>, Refusal> {
    match request.params.get("after") {
        None | Some(Value::Null) => Ok(None),
        Some(after) => after.as_u64().map(Some).ok_or(Refusal::BAD_PARAMS),
    }
}

/// 订阅的参数：`events` 带会话编号；`config`、`sessions`、`extensions` 不带会话、不带 `after`，带了是参数不对（施工 8-4、9-5、
/// 9-4 补）；`memory` 带 `persona` 或 `session`、可以带 `after`，原样交给找那一间的一方（施工 R-12 上）。
pub(super) fn stream_of(request: &Request) -> Result<Stream, Refusal> {
    let params: StreamParams =
        serde_json::from_value(request.params.clone()).map_err(|_| Refusal::BAD_PARAMS)?;
    if params.stream == "memory" {
        // 找哪一间同 `memory.*`：两样都写的、写错的由那边判（施工 R-12 上）。
        return Ok(Stream::Memory(MemoryAt {
            persona: params.persona,
            session: params.session,
        }));
    }
    match (params.stream.as_str(), params.session) {
        ("events", Some(session)) => SessionId::parse(&session)
            .map(Stream::Events)
            .map_err(|_| Refusal::BAD_PARAMS),
        // 视图流总是从最新一页起（施工 9-8 下）：不带 `after`。
        ("view", Some(session)) if request.params.get("after").is_none() => {
            SessionId::parse(&session)
                .map(Stream::View)
                .map_err(|_| Refusal::BAD_PARAMS)
        }
        ("config", None) if request.params.get("after").is_none() => Ok(Stream::Config),
        ("sessions", None) if request.params.get("after").is_none() => Ok(Stream::Sessions),
        ("extensions", None) if request.params.get("after").is_none() => Ok(Stream::Extensions),
        _ => Err(Refusal::BAD_PARAMS),
    }
}
