//! 并着发的调用口（施工 O-23 下，`onebot.md` 第一条「群里怎么叫她」第 12 条，「施工时定的」第 86 条）：问判官的任务经它调
//! `venue.records`、`model.call`，不等跟核心的那一头（[`super::Core`]）手上的事。同一条连接、同一个写的一头（写一行时锁着，
//! 两头的行不会搅在一起）；回应由读的一头照编号分给等它的那一个。
//!
//! - 编号是跟核心的那一头的前缀加 `-side-` 和序号：读的一头认得出哪些回应是这里的，不交给跟核心的那一头。
//! - 等的一方不等了（超时、任务被放下）：编号从等着的表里拿掉；回应晚来了，读的一头照编号认出是这里的、没人等，丢掉。
//! - 读的一头停了（核心关了管道）：等着的表清掉、关上，等着的和再来的都交回 [`Gone`]。
//!
//! 核心的 `model.call` 在后台答（施工 8-20 补，`protocol.md` 的 `model.call` 第 2 条）：判官一次几十秒，这段时间这条连接上别的
//! 请求不等它。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::oneshot;

use super::Gone;

/// 写的一头：两头共用，写一行时锁着。
pub(super) type Writer = Arc<tokio::sync::Mutex<Box<dyn AsyncWrite + Send + Unpin>>>;

/// 等着回应的：编号 → 交回应的一头。读的一头停了是空的。
type Table = Option<HashMap<String, oneshot::Sender<Value>>>;

/// 读的一头要的：认哪些回应、交给谁。
#[derive(Clone)]
pub(super) struct Waiting {
    /// 这里编的编号的前缀。
    prefix: String,
    /// 等着的。
    table: Arc<Mutex<Table>>,
}

impl Waiting {
    /// 前缀是 `prefix` 的一张空表。
    pub(super) fn new(prefix: String) -> Waiting {
        Waiting {
            prefix,
            table: Arc::new(Mutex::new(Some(HashMap::new()))),
        }
    }

    /// 读进来的一条 `message`：是这里编的编号的回应的，交给等它的那一个（没人等了的丢掉），交回空的；别的原样交回。
    pub(super) fn sort(&self, message: Value) -> Option<Value> {
        let ours = message.get("method").is_none()
            && message["id"]
                .as_str()
                .is_some_and(|id| id.starts_with(&self.prefix));
        if !ours {
            return Some(message);
        }
        let id = message["id"].as_str().unwrap_or_default().to_string();
        let waiter = self.lock().as_mut().and_then(|table| table.remove(&id));
        if let Some(waiter) = waiter
            && waiter.send(message).is_err()
        {
            // 等的一方刚好不等了：同没人等，丢掉。
        }
        None
    }

    /// 读的一头停了：等着的都交回断开，再来的也是。
    pub(super) fn close(&self) {
        *self.lock() = None;
    }

    /// 锁上表。锁坏了（拿着锁的地方 panic 了）照样用：表里只有编号和交回应的一头，坏不了。
    fn lock(&self) -> std::sync::MutexGuard<'_, Table> {
        self.table.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// 并着发的调用口：可以复制，问判官的任务各拿一份。
#[derive(Clone)]
pub(crate) struct Caller {
    /// 写的一头。
    writer: Writer,
    /// 等着回应的。
    waiting: Waiting,
    /// 下一个序号。
    next: Arc<AtomicU64>,
}

impl Caller {
    /// 往 `writer` 写、回应照 `waiting` 分的一个调用口。
    pub(super) fn new(writer: Writer, waiting: Waiting) -> Caller {
        Caller {
            writer,
            waiting,
            next: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 发一条请求，编号自己编，等到它的回应（接受的、拒绝的都交回原样）。不等了（放下这个 future）的，编号从表里拿掉，回应
    /// 晚来了丢掉。
    ///
    /// # Errors
    ///
    /// 写不出去、读的一头停了（核心断开）。
    pub(crate) async fn call(&self, method: &str, params: Value) -> Result<Value, Gone> {
        let n = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let id = format!("{}{n}", self.waiting.prefix);
        let (sender, answer) = oneshot::channel();
        match self.waiting.lock().as_mut() {
            Some(table) => table.insert(id.clone(), sender),
            None => return Err(Gone),
        };
        let _forget = Forget {
            waiting: &self.waiting,
            id: &id,
        };
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        write_line(&self.writer, &request).await?;
        answer.await.map_err(|_| Gone)
    }
}

/// 写一行：锁着写的一头，写完、刷出去。
///
/// # Errors
///
/// 写不出去：核心关了管道。
pub(super) async fn write_line(writer: &Writer, message: &Value) -> Result<(), Gone> {
    let mut writer = writer.lock().await;
    let written = async {
        writer.write_all(format!("{message}\n").as_bytes()).await?;
        writer.flush().await
    };
    written.await.map_err(|_| Gone)
}

/// 放下时把编号从等着的表里拿掉：等的一方不等了（超时、被放下），回应晚来了没人收。
struct Forget<'a> {
    waiting: &'a Waiting,
    id: &'a str,
}

impl Drop for Forget<'_> {
    fn drop(&mut self) {
        if let Some(table) = self.waiting.lock().as_mut() {
            table.remove(self.id);
        }
    }
}

#[cfg(test)]
mod tests;
