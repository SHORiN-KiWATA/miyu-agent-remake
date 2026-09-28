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

use crate::hello::{Peer, hello};
use crate::methods;
use crate::refusal::{Locale, Refusal};
use crate::subscriptions::Subscriptions;
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
    let mut peer: Option<Peer> = None;
    let mut subscriptions = Subscriptions::default();
    // 握手的期限（施工 4-9 再补三上）：连上以后这么久还没握手成的，断开。
    let deadline = tokio::time::Instant::now() + core.hello_wait;
    loop {
        let locale = peer.map_or(Locale::En, |peer| peer.locale);
        let read = match peer {
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
                // 超长的读不完，行界也找不回来了：回一句读不懂，断开。
                tracing::warn!(target: "miyu::endpoint", "line too long, closed");
                send(&out, wire::error(Value::Null, Refusal::PARSE, locale)).await;
                break;
            }
            Ok(Read::Closed) | Err(_) => break,
        };
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
                Ok((shaken, result)) => {
                    peer = Some(shaken);
                    (wire::result(&request.id, result), None, false)
                }
                // 被拒的，话照这一次报的语言说（施工 4-9 再补三上）：第一次握手也不是一律英文。
                Err((refusal, close)) => {
                    let asked = asked_locale(&request.params).unwrap_or(locale);
                    (wire::error(id(), refusal, asked), None, close)
                }
            },
            (_, None) => (wire::error(id(), Refusal::HELLO_FIRST, locale), None, false),
            ("subscribe", Some(_)) => {
                let result = subscribe(&core, &mut subscriptions, &request, &out).await;
                (answer(&request, result, locale), None, false)
            }
            ("unsubscribe", Some(_)) => {
                let result = stream_of(&request).map(|session| {
                    subscriptions.remove(&session);
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
    if peer.is_some() {
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
    session: String,
    stream: String,
}

/// 订阅会话的事件流：没在跑的照样先载入；已经订阅着的，还是那一个。
async fn subscribe(
    core: &Core,
    subscriptions: &mut Subscriptions,
    request: &Request,
    out: &mpsc::Sender<String>,
) -> Result<Value, Refusal> {
    let session = stream_of(request)?;
    if !subscriptions.has(&session) {
        let handle = core.sessions.get(core, &session, None).await?.handle;
        let Ok(subscription) = handle.subscribe().await else {
            core.sessions.forget(&session).await;
            return Err(Refusal::STOPPED);
        };
        subscriptions.add(session, subscription, out.clone());
    }
    Ok(json!({}))
}

/// 订阅的参数：会话编号，流现在只有 `events`。
fn stream_of(request: &Request) -> Result<SessionId, Refusal> {
    let params: StreamParams =
        serde_json::from_value(request.params.clone()).map_err(|_| Refusal::BAD_PARAMS)?;
    if params.stream != "events" {
        return Err(Refusal::BAD_PARAMS);
    }
    SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)
}

/// 命令是给哪个会话的：它的回应经这个会话的订阅写出去。
fn target(request: &Request) -> Option<SessionId> {
    request
        .params
        .get("session")
        .and_then(Value::as_str)
        .and_then(|session| SessionId::parse(session).ok())
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
