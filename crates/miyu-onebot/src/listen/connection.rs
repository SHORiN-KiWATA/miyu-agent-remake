//! 一条 NapCat 的连接（`onebot.md` 第一条「怎么走」第 2 到 6 条）：一帧一条 JSON，回应交给在等的调用，消息（私聊、群）、
//! 撤回（施工 O-22）和禁言、解禁（施工 O-25 中）交给跟核心的那一头，别的事件记一行调试日志就丢。号认出来了（`X-Self-ID`、
//! 第一条事件）也告诉跟核心的那一头（`Event::Connected`，施工 O-25 中：排着的照先后发），和消息走同一条队，先后一致。连上就
//! 调一次 `get_version_info`，把实现的名字和版本记进运行日志（第 3 条），也记在这条连接上，WebUI 的 `/status` 照它说（施工
//! O-16）。号认出来、断开、问到实现，都叫一声状态文件（施工 O-18）。
//!
//! 往 NapCat 写的都经一个写的任务（回话、`get_version_info`、被顶掉时的关闭帧）。断开时：在等的调用都算失败，号还是这一条
//! 的拿掉，说一行；桥不退，等 NapCat 自己重连（第 11 条）。

use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::{Notify, mpsc};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::Role;

use super::Gate;
use super::bots::{Link, Peer};
use crate::TARGET;
use crate::onebot::{self, Calls, Event, Frame};
use crate::serve::Notice;

/// 跑一条升级好的连接，直到断开。`bot` 是 `X-Self-ID` 报的号。
pub(super) async fn run<S>(io: S, gate: &Gate, mut bot: Option<i64>)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let ws = WebSocketStream::from_raw_socket(io, Role::Server, None).await;
    let (mut sink, mut frames) = ws.split();
    let (out, mut outgoing) = mpsc::channel::<Message>(gate.tuning.write_queue);
    // 写的任务：发的一头都放下了（连接断了、被顶掉了），或者写了关闭帧，就停。
    tokio::spawn(async move {
        while let Some(message) = outgoing.recv().await {
            let closing = matches!(message, Message::Close(_));
            if sink.send(message).await.is_err() || closing {
                break;
            }
        }
        if let Err(error) = sink.close().await {
            tracing::debug!(target: TARGET, error = %error, "websocket not closed cleanly");
        }
    });
    let calls = Arc::new(Calls::new(gate.tuning.call_timeout()));
    let link = Link {
        serial: gate.serial.fetch_add(1, Ordering::Relaxed),
        out: out.clone(),
        calls: Arc::clone(&calls),
        peer: Arc::new(OnceLock::new()),
    };
    // 跟核心的那一头不收了（桥在停）：这条连接不再读。
    let mut reading = match bot {
        Some(bot) => register(gate, bot, &link).await,
        None => true,
    };
    tracing::info!(target: TARGET, bot, "napcat connected");
    (gate.tell)(Notice::Connected { bot });
    let probe = version(
        Arc::clone(&calls),
        out.clone(),
        Arc::clone(&link.peer),
        Arc::clone(&gate.changed),
    );
    tokio::pin!(probe);
    let mut probed = false;
    while reading {
        tokio::select! {
            frame = frames.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    reading = frame_in(gate, &text, &mut bot, &link).await;
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => reading = false,
                Some(Ok(_)) => {}
            },
            () = &mut probe, if !probed => probed = true,
        }
    }
    calls.close();
    if let Some(bot) = bot {
        gate.bots.remove(bot, link.serial);
        gate.changed.notify_one();
    }
    tracing::info!(target: TARGET, bot, "napcat disconnected");
    (gate.tell)(Notice::Disconnected { bot });
}

/// 一帧文字。跟核心的那一头不收了（桥在停）交回假。
async fn frame_in(gate: &Gate, text: &str, bot: &mut Option<i64>, link: &Link) -> bool {
    let frame: Value = match serde_json::from_str(text) {
        Ok(frame) => frame,
        Err(error) => {
            tracing::debug!(target: TARGET, error = %error, "frame not understood");
            return true;
        }
    };
    if bot.is_none()
        && let Some(id) = onebot::self_id(&frame)
    {
        *bot = Some(id);
        if !register(gate, id, link).await {
            return false;
        }
    }
    match onebot::read(frame) {
        Frame::Reply(reply) => {
            if !link.calls.answer(reply) {
                tracing::debug!(target: TARGET, "reply nobody waits for");
            }
        }
        Frame::Event(event) => {
            noted(&event);
            return gate.inbound.send(event).await.is_ok();
        }
        Frame::Other(kind) => tracing::debug!(target: TARGET, kind, "event ignored"),
    }
    true
}

/// 读出来的一件事记一行调试日志：谁、哪一条、几个字，不记原文。
fn noted(event: &Event) {
    match event {
        Event::Private(posted) => tracing::debug!(
            target: TARGET,
            user = posted.user,
            message = posted.message_id,
            chars = posted.text.chars().count(),
            "private message"
        ),
        Event::Group { group, posted } => tracing::debug!(
            target: TARGET,
            group,
            user = posted.user,
            message = posted.message_id,
            chars = posted.text.chars().count(),
            "group message"
        ),
        Event::Recalled(recall) => tracing::debug!(
            target: TARGET,
            group = recall.group,
            user = recall.user,
            message = recall.message_id,
            "recall"
        ),
        Event::Muted { group, seconds, .. } => {
            tracing::debug!(target: TARGET, group, seconds, "muted");
        }
        Event::Unmuted { group, .. } => tracing::debug!(target: TARGET, group, "unmuted"),
        Event::Connected { bot } => tracing::debug!(target: TARGET, bot, "bot known"),
    }
}

/// 号 `bot` 现在用 `link`：顶掉的那一条在等的调用都算失败，给它发一帧关闭；告诉跟核心的那一头这个号连上了（施工 O-25 中，
/// 「施工时定的」第 126 条）。跟核心的那一头不收了（桥在停）交回假。
async fn register(gate: &Gate, bot: i64, link: &Link) -> bool {
    if let Some(old) = gate.bots.insert(bot, link.clone()) {
        old.calls.close();
        if old.out.try_send(Message::Close(None)).is_err() {
            // 旧的那一条已经断了，或者写队列满了：它的写的一头随连接一起放下。
            tracing::debug!(target: TARGET, bot, "old connection not told to close");
        }
        tracing::info!(target: TARGET, bot, "napcat connection replaced");
    }
    gate.changed.notify_one();
    gate.inbound.send(Event::Connected { bot }).await.is_ok()
}

/// 问对端是谁（第 3 条）：实现的名字、版本、协议版本记进运行日志；名字和版本记进 `peer`，叫一声 `changed`。
async fn version(
    calls: Arc<Calls>,
    out: mpsc::Sender<Message>,
    peer: Arc<OnceLock<Peer>>,
    changed: Arc<Notify>,
) {
    match calls.call(&out, "get_version_info", json!({})).await {
        Ok(reply) => {
            let data = &reply["data"];
            let known = Peer {
                implementation: data["app_name"].as_str().unwrap_or_default().to_string(),
                version: data["app_version"].as_str().unwrap_or_default().to_string(),
            };
            if peer.set(known).is_err() {
                // 一条连接只问一次：不会已经有了。
            }
            changed.notify_one();
            tracing::info!(
                target: TARGET,
                app_name = data["app_name"].as_str().unwrap_or_default(),
                app_version = data["app_version"].as_str().unwrap_or_default(),
                protocol_version = data["protocol_version"].as_str().unwrap_or_default(),
                "napcat version"
            );
        }
        Err(error) => tracing::warn!(target: TARGET, error = ?error, "napcat version unknown"),
    }
}
