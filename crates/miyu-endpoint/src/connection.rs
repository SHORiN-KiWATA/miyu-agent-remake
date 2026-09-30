//! 一个连接（`docs/designs/04-核心协议.md` 第三节「一次连接的全过程」）：一行一条消息，第一条必须是
//! `hello`，之后一条条照方法办。
//!
//! 读和写分开：读的一头一条条办请求；写的一头从一个有上限的队列里取出一行行写出去。订阅的推送由各自的
//! 转发任务放进同一个队列（施工 3-8 中）。头读得慢，队列满了，转发任务就不再从会话那里拿，会话那边的
//! 队列满了就掉队：核心和会话都不等这个头（第七节）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use miyu_kernel::id::SessionId;
use miyu_session::Handle;

use crate::hello::{Shaken, hello};
use crate::methods;
use crate::refusal::{Locale, Refusal};
use crate::subscriptions::{Subscriptions, Target};
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
    let _connected = Connected::new(Arc::clone(&core));
    let (read, write) = tokio::io::split(stream);
    let (out, lines) = mpsc::channel(QUEUE);
    tokio::join!(read_all(read, core, out), write_all(write, lines));
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

/// 读的一头：一条条办请求，回应放进写队列（订阅了的会话，经它的转发任务）。
async fn read_all<R: AsyncRead + Unpin>(read: R, core: Arc<Core>, out: mpsc::Sender<String>) {
    let mut reader = BufReader::new(read);
    let mut shaken: Option<Shaken> = None;
    let mut subscriptions = Subscriptions::default();
    // 握手的期限（施工 4-9 再补三上）：连上以后这么久还没握手成的，断开。
    let deadline = tokio::time::Instant::now() + core.hello_wait;
    loop {
        let read = match shaken {
            Some(_) => wire::read_line(&mut reader).await,
            None => match tokio::time::timeout_at(deadline, wire::read_line(&mut reader)).await {
                Ok(read) => read,
                Err(_) => {
                    tracing::info!(target: "miyu::endpoint", "no hello, closed");
                    break;
                }
            },
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
        let id = || Value::String(request.id.as_str().to_string());
        let (answer, target, close) = match (request.method.as_str(), peer) {
            ("hello", _) => match hello(&core, request.params.clone()) {
                Ok((shook, result)) => {
                    shaken = Some(shook);
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
            ("unsubscribe", Some(_)) => {
                let result = stream_of(&request).map(|stream| {
                    match stream {
                        Stream::Events(session) => subscriptions.remove(&session),
                        Stream::Config => subscriptions.remove_config(),
                    }
                    json!({})
                });
                (answer(&request, result, locale), None, false)
            }
            (_, Some(peer)) => {
                let result = methods::call(&core, peer, &request).await;
                (answer(&request, result, locale), target(&request), false)
            }
        };
        if !subscriptions.reply(target.as_ref(), answer, &out).await || close {
            break;
        }
    }
    if shaken.is_some() {
        tracing::info!(target: "miyu::endpoint", "disconnected");
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
}

/// 订阅会话的事件流：没在跑的照样先载入；已经订阅着的，还是那一个。回应带会话的限额（施工 6-3 补）：订阅着的也从会话表
/// 拿，在跑的直接用，不多载入。写了 `after` 的，先补之前的事件（[`subscribe_after`]）。交回回应，和回应经哪个订阅写出去：
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
    let limits = handle.limits();
    if let Some(after) = after {
        let upto = subscribe_after(core, subscriptions, &handle, &session, after, out).await?;
        return Ok((json!({"limits": limits, "upto": upto}), Some(session)));
    }
    if !subscriptions.has(&session) {
        let Ok(subscription) = handle.subscribe().await else {
            core.sessions.forget(&session).await;
            return Err(Refusal::STOPPED);
        };
        subscriptions.add(session, subscription, Vec::new(), out.clone());
    }
    Ok((json!({"limits": limits}), None))
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

/// 订阅的参数：`events` 带会话编号；`config` 不带会话、不带 `after`，带了是参数不对（施工 8-4）。
fn stream_of(request: &Request) -> Result<Stream, Refusal> {
    let params: StreamParams =
        serde_json::from_value(request.params.clone()).map_err(|_| Refusal::BAD_PARAMS)?;
    match (params.stream.as_str(), params.session) {
        ("events", Some(session)) => SessionId::parse(&session)
            .map(Stream::Events)
            .map_err(|_| Refusal::BAD_PARAMS),
        ("config", None) if request.params.get("after").is_none() => Ok(Stream::Config),
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
