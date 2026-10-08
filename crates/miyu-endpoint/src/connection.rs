//! 一个连接（`docs/designs/04-核心协议.md` 第三节「一次连接的全过程」）：一行一条消息，第一条必须是
//! `hello`，之后一条条照方法办。
//!
//! 读和写分开：读的一头一条条办请求；写的一头从一个有上限的队列里取出一行行写出去。订阅的推送由各自的
//! 转发任务放进同一个队列（施工 3-8 中）。头读得慢，队列满了，转发任务就不再从会话那里拿，会话那边的
//! 队列满了就掉队：核心和会话都不等这个头（第七节）。
//!
//! 分块上传跟着连接走（施工 W-5）：这个连接的上传表（[`crate::uploads::Uploads`]）住在读的一头的循环里，
//! 和请求一条条办；连接断了，循环结束前把它开着的上传全部作废、删暂存文件。
//!
//! 身份（施工 W-8，`web-module.md`「怎么走」第一条）：握手时定这个连接是怎么认出来的（[`Via`]）。用一次性码连上的只能调
//! `hello`、`human.get`、`account.setup`，别的回 `setup_first`；设好了换成登录令牌的连接。用登录令牌、密码连上的收作废的
//! 广播，作废了它靠的那个令牌就断开。
//!
//! 登记成在后台答的查询（`link.preview`，施工 W-7，`net.md`「怎么走」第 11 条）是一条条办的例外：交给这个连接自己的
//! 一组后台任务，接着读下一行；办完了回应照 `id` 对上，直接放进写队列。连接断了，这组任务一起停：不然连接走了
//! 还在抓，核心一直不算空闲。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;

use miyu_kernel::id::SessionId;
use miyu_session::Handle;

use crate::hello::{Shaken, hello};
use crate::login::{self, Revoked, Via};
use crate::methods;
use crate::queries::Handler;
use crate::refusal::{Locale, Refusal};
use crate::subscriptions::{Subscriptions, Target};
use crate::uploads::Uploads;
use crate::wire::{self, Incoming, Read, Request};
use crate::{Connected, Core};

/// 写队列能攒多少行：满了，转发任务就等着，会话那边掉队（`04-核心协议.md` 第七节）。
const QUEUE: usize = 256;

/// 照协议和这个连接说话，直到对方关了、握手没过，或者读写出错。字节流从哪来不管：本机套接字、命名
/// 管道、以后的 WebSocket、测试里的内存管道都行。
pub async fn serve<S>(stream: S, core: Arc<Core>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    serve_from(stream, core, None).await;
}

/// 同 [`serve`]，对面是核心亲手拉起的扩展（施工 9-4 上）：握手不看凭据，握成了往 `shook` 说一声。
pub(crate) async fn serve_spawned<S>(stream: S, core: Arc<Core>, shook: oneshot::Sender<()>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    serve_from(stream, core, Some(shook)).await;
}

/// 两种连接共用的：`spawned` 有的是核心亲手拉起的扩展。
async fn serve_from<S>(stream: S, core: Arc<Core>, spawned: Option<oneshot::Sender<()>>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let _connected = Connected::new(Arc::clone(&core));
    let (read, write) = tokio::io::split(stream);
    let (out, lines) = mpsc::channel(QUEUE);
    tokio::join!(read_all(read, core, out, spawned), write_all(write, lines));
}

/// 写的一头：一行行写出去，写不出去就停。
async fn write_all<W: AsyncWrite + Unpin>(mut write: W, mut lines: mpsc::Receiver<String>) {
    while let Some(line) = lines.recv().await {
        let written = async {
            write.write_all(line.as_bytes()).await?;
            write.write_all(b"\n").await?;
            write.flush().await
        };
        if written.await.is_err() {
            return;
        }
    }
}

/// 读的一头：一条条办请求，回应放进写队列（订阅了的会话，经它的转发任务）。`spawned` 见 [`serve_from`]。
async fn read_all<R: AsyncRead + Unpin>(
    read: R,
    core: Arc<Core>,
    out: mpsc::Sender<String>,
    mut spawned: Option<oneshot::Sender<()>>,
) {
    let from_core = spawned.is_some();
    let mut reader = BufReader::new(read);
    let mut shaken: Option<Shaken> = None;
    // 这个连接是怎么认出来的（施工 W-8）：用登录令牌、密码连上的另收作废的广播。
    let mut via: Option<Via> = None;
    let mut revoked: Option<tokio::sync::broadcast::Receiver<Revoked>> = None;
    let mut subscriptions = Subscriptions::default();
    // 这个连接上的分块上传（施工 W-5）：和请求一条条办，不用锁。
    let mut uploads = Uploads::new(core.upload_idle);
    // 这个连接在后台答的请求（施工 W-7）：连接断了，丢掉它就一起停了。
    let mut background = JoinSet::new();
    // 握手的期限（施工 4-9 再补三上）：连上以后这么久还没握手成的，断开。
    let deadline = tokio::time::Instant::now() + core.hello_wait;
    loop {
        let read = match (shaken, revoked.as_mut()) {
            (Some(_), Some(revoked)) => {
                let mine = via
                    .as_ref()
                    .and_then(Via::login)
                    .unwrap_or_default()
                    .to_string();
                tokio::select! {
                    read = wire::read_line(&mut reader) => read,
                    () = until_revoked(revoked, &mine) => {
                        tracing::info!(target: "miyu::endpoint", "login revoked, closed");
                        break;
                    }
                }
            }
            (Some(_), None) => wire::read_line(&mut reader).await,
            (None, _) => {
                match tokio::time::timeout_at(deadline, wire::read_line(&mut reader)).await {
                    Ok(read) => read,
                    Err(_) => {
                        tracing::info!(target: "miyu::endpoint", "no hello, closed");
                        break;
                    }
                }
            }
        };
        let line = match read {
            Ok(Read::Line(line)) => line,
            Ok(Read::TooLong) => {
                let locale = shaken.map_or(Locale::En, |shaken| shaken.now(&core).locale);
                // 超长的读不完，行界也找不回来了：回一句读不懂，断开。
                tracing::warn!(target: "miyu::endpoint", "line too long, closed");
                send(&out, wire::error(Value::Null, Refusal::PARSE, locale)).await;
                break;
            }
            Ok(Read::Closed) | Err(_) => break,
        };
        // 每说一句都照这时的 `ui.language` 重算这个连接的语言（施工 8-4）。
        let peer = shaken.map(|shaken| shaken.now(&core));
        let locale = peer.map_or(Locale::En, |peer| peer.locale);
        let request = match wire::parse(&line) {
            Incoming::Request(request) => request,
            Incoming::Notification => continue,
            Incoming::Bad(id, refusal) => {
                if !send(&out, wire::error(id, refusal, locale)).await {
                    break;
                }
                continue;
            }
        };
        tracing::debug!(target: "miyu::endpoint", method = request.method.as_str(), "request");
        reap(&mut background);
        // 用一次性码连上的，先设密码（施工 W-8）。
        if via == Some(Via::Code)
            && !matches!(
                request.method.as_str(),
                "hello" | "human.get" | "account.setup"
            )
        {
            let refused = wire::error(
                Value::String(request.id.as_str().to_string()),
                Refusal::SETUP_FIRST,
                locale,
            );
            if !send(&out, refused).await {
                break;
            }
            continue;
        }
        // 扩展不能开关、重启扩展（施工 9-4 上）：那是人的事。
        if via == Some(Via::Spawned) && request.method.starts_with("extension.") {
            let refused = wire::error(
                Value::String(request.id.as_str().to_string()),
                Refusal::LOCAL_ONLY,
                locale,
            );
            if !send(&out, refused).await {
                break;
            }
            continue;
        }
        if peer.is_some()
            && let Some(handler) = core.queries.background(&request.method)
        {
            background.spawn(answer_later(
                handler,
                Arc::clone(&core),
                request,
                locale,
                out.clone(),
            ));
            continue;
        }
        let id = || Value::String(request.id.as_str().to_string());
        let (answer, target, close) = match (request.method.as_str(), peer) {
            ("hello", _) => match hello(&core, request.params.clone(), from_core).await {
                Ok((shook, by, result)) => {
                    if let Some(ready) = spawned.take()
                        && ready.send(()).is_err()
                    {
                        // 看管它的那一头不等了：不用说。
                    }
                    shaken = Some(shook);
                    revoked = by.login().map(|_| core.identity.revoked());
                    via = Some(by);
                    (wire::result(&request.id, result), None, false)
                }
                // 被拒的，话照这一次报的语言说（施工 4-9 再补三上）：第一次握手也不是一律英文。
                Err((refusal, close)) => {
                    let asked = asked_locale(&request.params).unwrap_or(locale);
                    (wire::error(id(), refusal, asked), None, close)
                }
            },
            (_, None) => (wire::error(id(), Refusal::HELLO_FIRST, locale), None, false),
            ("subscribe", Some(peer)) => {
                let (result, target) = match stream_of(&request) {
                    Ok(Stream::Config) => {
                        let system = shaken.map_or("en", Shaken::system);
                        subscriptions.add_config(&core, system, &out);
                        (Ok(json!({})), None)
                    }
                    Ok(Stream::Sessions) => match subscriptions.add_sessions(&core, &out).await {
                        Ok(result) => (Ok(result), Some(Target::Sessions)),
                        Err(refusal) => (Err(refusal), None),
                    },
                    Ok(Stream::Events(session)) => {
                        match subscribe(&core, &mut subscriptions, &request, session, &out).await {
                            Ok((result, target)) => (Ok(result), target.map(Target::Session)),
                            Err(refusal) => (Err(refusal), None),
                        }
                    }
                    Err(refusal) => (Err(refusal), None),
                };
                (answer(&request, result, peer.locale), target, false)
            }
            ("account.setup_code", Some(_)) => {
                let result = login::setup_code(&core, via.as_ref().unwrap_or(&Via::Token));
                (answer(&request, result, locale), None, false)
            }
            ("account.setup", Some(_)) => {
                let now = via.clone().unwrap_or(Via::Token);
                match login::setup(&core, &now, request.params.clone()).await {
                    Ok((result, by)) => {
                        revoked = Some(core.identity.revoked());
                        via = Some(by);
                        (wire::result(&request.id, result), None, false)
                    }
                    Err(refusal) => (answer(&request, Err(refusal), locale), None, false),
                }
            }
            ("account.logout", Some(_)) => {
                let now = via.clone().unwrap_or(Via::Token);
                match login::logout(&core, &now, request.params.clone()).await {
                    Ok((result, close)) => (wire::result(&request.id, result), None, close),
                    Err(refusal) => (answer(&request, Err(refusal), locale), None, false),
                }
            }
            ("unsubscribe", Some(_)) => {
                let result = stream_of(&request).map(|stream| {
                    match stream {
                        Stream::Events(session) => subscriptions.remove(&session),
                        Stream::Config => subscriptions.remove_config(),
                        Stream::Sessions => subscriptions.remove_sessions(),
                    }
                    json!({})
                });
                (answer(&request, result, locale), None, false)
            }
            (_, Some(peer)) => {
                let result = methods::call(&core, peer, &request, &mut uploads).await;
                (answer(&request, result, locale), target(&request), false)
            }
        };
        if !subscriptions.reply(target.as_ref(), answer, &out).await || close {
            break;
        }
    }
    // 连接断了：在后台答的一起停（施工 W-7）；它开着的上传全部作废，删暂存文件（`web-module.md`「怎么走」第六条
    // 第 4 款）。
    background.abort_all();
    uploads.discard_all(&core).await;
    if shaken.is_some() {
        tracing::info!(target: "miyu::endpoint", "disconnected");
    }
}

/// 等到作废了这个连接靠的登录令牌 `mine`（施工 W-8）：全部作废、作废的正是它；收慢了丢了几条的也当作废了（页面照登录令牌
/// 重连，还认得的照常进来）。
async fn until_revoked(revoked: &mut tokio::sync::broadcast::Receiver<Revoked>, mine: &str) {
    loop {
        match revoked.recv().await {
            Ok(Revoked::One(hash)) if hash != mine => {}
            Ok(_) | Err(_) => return,
        }
    }
}

/// 在后台答一条（施工 W-7）：办完了把回应放进写队列。连接已经断了的，放不进去也不要紧，没人收了。
async fn answer_later(
    handler: Handler,
    core: Arc<Core>,
    request: Request,
    locale: Locale,
    out: mpsc::Sender<String>,
) {
    let result = handler(core, request.params.clone())
        .await
        .map_err(Refusal::from);
    send(&out, answer(&request, result, locale)).await;
}

/// 收掉办完了的后台任务，不让这一组越攒越多；崩了的记一行（它的回应永远不会来了）。
fn reap(background: &mut JoinSet<()>) {
    while let Some(joined) = background.try_join_next() {
        if let Err(error) = joined
            && error.is_panic()
        {
            tracing::error!(target: "miyu::endpoint", error = %error, "background request panicked");
        }
    }
}

/// `hello` 里报的语言，读得出来的话（施工 4-9 再补三上）：握手被拒时照它说。
fn asked_locale(params: &Value) -> Option<Locale> {
    params
        .get("locale")
        .and_then(Value::as_str)
        .map(|locale| Locale::of(Some(locale)))
}

/// `subscribe`、`unsubscribe` 的参数。
#[derive(Debug, Deserialize)]
struct StreamParams {
    #[serde(default)]
    session: Option<String>,
    stream: String,
}

/// 订阅哪一个流。
enum Stream {
    /// 一个会话的事件流。
    Events(SessionId),
    /// 配置的推送（施工 8-4）。
    Config,
    /// 会话列表的推送（施工 9-5）。
    Sessions,
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
async fn subscribe(
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

/// 订阅的参数：`events` 带会话编号；`config`、`sessions` 不带会话、不带 `after`，带了是参数不对（施工 8-4、9-5）。
fn stream_of(request: &Request) -> Result<Stream, Refusal> {
    let params: StreamParams =
        serde_json::from_value(request.params.clone()).map_err(|_| Refusal::BAD_PARAMS)?;
    match (params.stream.as_str(), params.session) {
        ("events", Some(session)) => SessionId::parse(&session)
            .map(Stream::Events)
            .map_err(|_| Refusal::BAD_PARAMS),
        ("config", None) if request.params.get("after").is_none() => Ok(Stream::Config),
        ("sessions", None) if request.params.get("after").is_none() => Ok(Stream::Sessions),
        _ => Err(Refusal::BAD_PARAMS),
    }
}

/// 命令的回应经哪个订阅写出去：给会话的经这个会话的订阅，`config.set` 经配置的订阅（施工 8-4）。
fn target(request: &Request) -> Option<Target> {
    if request.method == "config.set" {
        return Some(Target::Config);
    }
    request
        .params
        .get("session")
        .and_then(Value::as_str)
        .and_then(|session| SessionId::parse(session).ok())
        .map(Target::Session)
}

/// 回应写成一行：接受的 `result`，拒绝的 `error`。
fn answer(request: &Request, result: Result<Value, Refusal>, locale: Locale) -> String {
    match result {
        Ok(result) => wire::result(&request.id, result),
        Err(refusal) => wire::error(
            Value::String(request.id.as_str().to_string()),
            refusal,
            locale,
        ),
    }
}

/// 放进写队列；写队列关了（连接断了），交回 `false`。
async fn send(out: &mpsc::Sender<String>, line: String) -> bool {
    out.send(line).await.is_ok()
}
