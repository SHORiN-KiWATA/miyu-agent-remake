//! 一个连接（`docs/designs/04-核心协议.md` 第三节「一次连接的全过程」）：一行一条消息，第一条必须是
//! `hello`，之后一条条照方法办，回应写回去。对方关了，或者握手没过，就断开。

use std::sync::Arc;

use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use crate::Core;
use crate::hello::{Peer, hello};
use crate::methods;
use crate::refusal::{Locale, Refusal};
use crate::wire::{self, Incoming, Read};

/// 照协议和这个连接说话，直到对方关了、握手没过，或者读写出错。字节流从哪来不管：本机套接字、命名
/// 管道、以后的 WebSocket、测试里的内存管道都行。
pub async fn serve<S>(stream: S, core: Arc<Core>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (read, mut write) = tokio::io::split(stream);
    let mut reader = BufReader::new(read);
    let mut peer: Option<Peer> = None;
    loop {
        let locale = peer.map_or(Locale::En, |peer| peer.locale);
        let line = match wire::read_line(&mut reader).await {
            Ok(Read::Line(line)) => line,
            Ok(Read::TooLong) => {
                // 超长的读不完，行界也找不回来了：回一句读不懂，断开。
                if reply(
                    &mut write,
                    &wire::error(Value::Null, Refusal::PARSE, locale),
                )
                .await
                .is_ok()
                {
                    tracing::warn!(target: "miyu::endpoint", "line too long, closed");
                }
                break;
            }
            Ok(Read::Closed) | Err(_) => break,
        };
        let request = match wire::parse(&line) {
            Incoming::Request(request) => request,
            Incoming::Notification => continue,
            Incoming::Bad(id, refusal) => {
                if reply(&mut write, &wire::error(id, refusal, locale))
                    .await
                    .is_err()
                {
                    break;
                }
                continue;
            }
        };
        tracing::debug!(target: "miyu::endpoint", method = request.method.as_str(), "request");
        let (answer, close) = if request.method == "hello" {
            match hello(&core, request.params.clone()) {
                Ok((shaken, result)) => {
                    peer = Some(shaken);
                    (wire::result(&request.id, result), false)
                }
                Err((refusal, close)) => (
                    wire::error(
                        Value::String(request.id.as_str().to_string()),
                        refusal,
                        locale,
                    ),
                    close,
                ),
            }
        } else if let Some(peer) = peer {
            match methods::call(&core, peer, &request).await {
                Ok(result) => (wire::result(&request.id, result), false),
                Err(refusal) => (
                    wire::error(
                        Value::String(request.id.as_str().to_string()),
                        refusal,
                        locale,
                    ),
                    false,
                ),
            }
        } else {
            let id = Value::String(request.id.as_str().to_string());
            (wire::error(id, Refusal::HELLO_FIRST, locale), false)
        };
        if reply(&mut write, &answer).await.is_err() || close {
            break;
        }
    }
    if peer.is_some() {
        tracing::info!(target: "miyu::endpoint", "disconnected");
    }
}

/// 写一行回去。
async fn reply<W: AsyncWrite + Unpin>(write: &mut W, line: &str) -> std::io::Result<()> {
    write.write_all(line.as_bytes()).await?;
    write.write_all(b"\n").await?;
    write.flush().await
}
