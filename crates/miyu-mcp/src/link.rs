//! 一条连接（施工 X-1）：写的一头一个任务、读的一头一个任务，请求照编号等回应。读到头、读到太长的一行、写不进去，这条连接就
//! 断了：在等的都收到 [`Failed::Closed`]，之后发的当场收到。
//!
//! 服务反过来的请求在读的那个任务里当场回：`ping` 回空的，别的回「没有这个方法」（`-32601`）。工具变了的通知数一次。

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot, watch};

use crate::TARGET;
use crate::error::Failed;
use crate::wire::{self, Incoming, Read};

/// 「没有这个方法」。
const METHOD_NOT_FOUND: i64 = -32601;

/// 一条连接。
pub(crate) struct Link {
    out: mpsc::UnboundedSender<Vec<u8>>,
    shared: Arc<Shared>,
    next: AtomicU64,
}

/// 读写两个任务和发请求的一方共用的。
struct Shared {
    waiting: Mutex<Waiting>,
    /// 服务说工具变了几次。
    changes: watch::Sender<u64>,
    /// 断了没有。
    closed: watch::Sender<bool>,
}

/// 在等回应的请求。
#[derive(Default)]
struct Waiting {
    open: bool,
    asked: BTreeMap<u64, oneshot::Sender<Result<Value, Value>>>,
}

impl Link {
    /// 照服务的标准输出 `reader`、标准输入 `writer` 起读写两个任务。
    pub(crate) fn start<R, W>(reader: R, writer: W) -> Link
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (out, lines) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared {
            waiting: Mutex::new(Waiting {
                open: true,
                asked: BTreeMap::new(),
            }),
            changes: watch::channel(0).0,
            closed: watch::channel(false).0,
        });
        tokio::spawn(write(writer, lines, Arc::clone(&shared)));
        tokio::spawn(read(reader, out.clone(), Arc::clone(&shared)));
        Link {
            out,
            shared,
            next: AtomicU64::new(1),
        }
    }

    /// 发请求 `method`，等回应：`wait` 是最多等多久，没写的一直等。`cancel` 的，没等到回应就不等了（超时、这个 future 被丢掉）
    /// 时发 `notifications/cancelled`；握手那几个不发（MCP 规范：`initialize` 不许取消）。
    pub(crate) async fn ask(
        &self,
        method: &str,
        params: Value,
        wait: Option<Duration>,
        cancel: bool,
    ) -> Result<Value, Failed> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = oneshot::channel();
        {
            let mut waiting = self.shared.lock();
            if !waiting.open {
                return Err(Failed::Closed);
            }
            waiting.asked.insert(id, reply);
        }
        let mut asking = Asking {
            link: self,
            id,
            cancel,
            done: false,
        };
        if self.out.send(wire::request(id, method, params)).is_err() {
            return Err(Failed::Closed);
        }
        let answered = match wait {
            Some(wait) => tokio::time::timeout(wait, answer)
                .await
                .map_err(|_| Failed::TimedOut)?,
            None => answer.await,
        };
        asking.done = true;
        match answered {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(error)) => Err(rpc(error)),
            Err(_) => Err(Failed::Closed),
        }
    }

    /// 发通知 `method`。断了的不发。
    pub(crate) fn tell(&self, method: &str, params: Value) {
        if self.out.send(wire::notice(method, params)).is_err() {
            tracing::debug!(target: TARGET, method, "notice not sent: closed");
        }
    }

    /// 服务说工具变了几次。
    pub(crate) fn changes(&self) -> watch::Receiver<u64> {
        self.shared.changes.subscribe()
    }

    /// 等到这条连接断了。
    pub(crate) async fn closed(&self) {
        let mut closed = self.shared.closed.subscribe();
        if closed.wait_for(|closed| *closed).await.is_err() {
            tracing::debug!(target: TARGET, "close flag dropped");
        }
    }
}

/// 一个在等回应的请求：没等到就不等了的，拿掉它，要发取消的发。
struct Asking<'a> {
    link: &'a Link,
    id: u64,
    cancel: bool,
    done: bool,
}

impl Drop for Asking<'_> {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        let was_waiting = self.link.shared.lock().asked.remove(&self.id).is_some();
        if was_waiting && self.cancel {
            self.link.tell(
                "notifications/cancelled",
                json!({"requestId": self.id, "reason": "the caller stopped waiting"}),
            );
        }
    }
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, Waiting> {
        self.waiting.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 断了：在等的都放下（它们收到 [`Failed::Closed`]），之后发的当场收到。
    fn close(&self) {
        let asked = {
            let mut waiting = self.lock();
            waiting.open = false;
            std::mem::take(&mut waiting.asked)
        };
        drop(asked);
        self.closed.send_replace(true);
    }
}

/// 写的一头：一行行写进服务的标准输入。写不进去就断了。
async fn write<W: AsyncWrite + Unpin>(
    mut writer: W,
    mut lines: mpsc::UnboundedReceiver<Vec<u8>>,
    shared: Arc<Shared>,
) {
    while let Some(line) = lines.recv().await {
        let written = async {
            writer.write_all(&line).await?;
            writer.flush().await
        };
        if let Err(error) = written.await {
            tracing::debug!(target: TARGET, error = %error, "MCP server input closed");
            shared.close();
            return;
        }
    }
}

/// 读的一头：一行行认，回应交给在等的，服务的请求当场回，通知照种类办。读到头、太长、读不了就断了。
async fn read<R: AsyncRead + Unpin>(
    reader: R,
    out: mpsc::UnboundedSender<Vec<u8>>,
    shared: Arc<Shared>,
) {
    let mut reader = BufReader::new(reader);
    loop {
        let line = match wire::read_line(&mut reader).await {
            Ok(Read::Line(line)) => line,
            Ok(Read::Closed) => break,
            Ok(Read::TooLong) => {
                tracing::warn!(target: TARGET, limit = wire::LINE_LIMIT, "MCP line too long, closing");
                break;
            }
            Err(error) => {
                tracing::debug!(target: TARGET, error = %error, "MCP server output unreadable");
                break;
            }
        };
        match wire::parse(&line) {
            Incoming::Answer(id, outcome) => {
                let reply = shared.lock().asked.remove(&id);
                if let Some(reply) = reply
                    && reply.send(outcome).is_err()
                {
                    tracing::debug!(target: TARGET, id, "answer arrived after the caller left");
                }
            }
            Incoming::Request(id, method) => {
                let outcome = match method.as_str() {
                    "ping" => Ok(json!({})),
                    _ => {
                        tracing::info!(target: TARGET, method = method.as_str(), "MCP server request refused");
                        Err((METHOD_NOT_FOUND, "Method not found"))
                    }
                };
                if out.send(wire::answer(&id, outcome)).is_err() {
                    break;
                }
            }
            Incoming::Notice(method, _) => match method.as_str() {
                "notifications/tools/list_changed" => {
                    shared.changes.send_modify(|count| *count += 1);
                }
                _ => tracing::trace!(target: TARGET, method = method.as_str(), "MCP notice"),
            },
            Incoming::Bad(problem) => {
                tracing::warn!(target: TARGET, problem = problem.as_str(), "MCP line not understood");
            }
        }
    }
    shared.close();
}

/// 服务回的错：错误码、原话、附带的；写法不对的照写法不对报。
fn rpc(error: Value) -> Failed {
    let code = error.get("code").and_then(Value::as_i64);
    let message = error.get("message").and_then(Value::as_str);
    match (code, message) {
        (Some(code), Some(message)) => Failed::Rpc {
            code,
            message: message.to_string(),
            data: error.get("data").cloned(),
        },
        _ => Failed::Malformed(format!("an error without code or message: {error}")),
    }
}
