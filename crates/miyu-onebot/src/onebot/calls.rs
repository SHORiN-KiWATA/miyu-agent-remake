//! 发出去的动作和回应照 `echo` 配对（`onebot.md` 第一条「怎么走」第 4 条）：`echo` 由桥自己编，同一条连接里不重；等带同样
//! `echo` 的回应，等多久照 `bridge.json` 的 `call_timeout_seconds`，等不到算失败；连接断了，在等的都算失败，之后再调的也是。
//!
//! 一条连接一个 [`Calls`]。分两步：[`Calls::begin`] 把这一帧放进连接的写队列（照调的先后），[`Pending::wait`] 等回应；
//! 发回话的一方先在自己的循环里 `begin`、再把等的那一步交给别的任务，几句回话就照她说的先后到 QQ（`core/route/sending.rs`）。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

/// 一次调用没成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    /// 等了 [`Calls::new`] 给的那么久，没等到回应。
    Timeout,
    /// 连接断了：发不出去，或者等的时候断了。
    Closed,
    /// 回了，`status` 不是 `ok`：交回回应原文（施工 O-25 中：出站队列取它的 `message` 记进 `failed`，`onebot.md`「施工时定的」
    /// 第 120 条）。
    Failed(Value),
}

/// NapCat 回失败时说的为什么（施工 O-31 起平台工具、O-33 取东西照它答）：`message` 去掉首尾空白，空的换 `wording`，再空的写
/// `retcode`；截到 [`DETAIL`] 个字符。
pub fn said(reply: &Value) -> String {
    let said = ["message", "wording"]
        .iter()
        .filter_map(|key| reply[*key].as_str())
        .map(str::trim)
        .find(|said| !said.is_empty())
        .map_or_else(|| reply["retcode"].to_string(), str::to_string);
    said.chars().take(DETAIL).collect()
}

/// 回失败时说的为什么最多留几个字符：出站队列 `failed` 的 `detail`（`onebot.md`「出站队列」第 4 条）、平台工具答的原话（施工
/// O-31）、取东西答的原话（施工 O-33）都照它。
pub const DETAIL: usize = 200;

/// 一条连接上在等回应的调用。
#[derive(Debug)]
pub struct Calls {
    /// 等一个回应最多多久。
    timeout: Duration,
    /// 下一个 `echo` 的序号。
    next: AtomicU64,
    /// 在等的：`echo` 到交回应的那一头。连接断了换成 `None`，之后再调的直接失败。
    pending: Mutex<Option<HashMap<String, oneshot::Sender<Value>>>>,
}

/// 放进了写队列、在等回应的一次调用。
#[derive(Debug)]
pub struct Pending {
    /// 等多久。
    timeout: Duration,
    /// 收回应的那一头：连接断了、过了时，交的那一头被拿掉，这里收到的是关了。
    answer: oneshot::Receiver<Value>,
}

impl Calls {
    /// 一个新的：还没调过，连接还在；一个回应最多等 `timeout`。
    pub fn new(timeout: Duration) -> Calls {
        Calls {
            timeout,
            next: AtomicU64::new(0),
            pending: Mutex::new(Some(HashMap::new())),
        }
    }

    /// 调一个动作：放进写队列 `out`，等回应。
    ///
    /// # Errors
    ///
    /// 连接断了（之前、等的时候）；等了 [`Calls::new`] 给的那么久没回；回的 `status` 不是 `ok`。
    pub async fn call(
        &self,
        out: &mpsc::Sender<Message>,
        action: &str,
        params: Value,
    ) -> Result<Value, CallError> {
        let pending = self.begin(out, action, params).await?;
        pending.wait().await
    }

    /// 编一个 `echo`，登记好，把这一帧放进写队列 `out`（写队列满了等着）。
    ///
    /// # Errors
    ///
    /// 连接已经断了，或者写的一头已经走了：[`CallError::Closed`]。
    pub async fn begin(
        &self,
        out: &mpsc::Sender<Message>,
        action: &str,
        params: Value,
    ) -> Result<Pending, CallError> {
        let echo = format!("miyu-{}", self.next.fetch_add(1, Ordering::Relaxed));
        let (sender, answer) = oneshot::channel();
        match self.lock().as_mut() {
            Some(pending) => {
                // 过了时的那几个，等的一头已经放下了：顺手拿掉，表不会越攒越多。
                pending.retain(|_, waiting| !waiting.is_closed());
                pending.insert(echo.clone(), sender);
            }
            None => return Err(CallError::Closed),
        }
        let frame = json!({"action": action, "params": params, "echo": echo});
        if out.send(Message::text(frame.to_string())).await.is_err() {
            self.forget(&echo);
            return Err(CallError::Closed);
        }
        Ok(Pending {
            timeout: self.timeout,
            answer,
        })
    }

    /// 来了一个回应：交给照 `echo` 在等的那一个。没人等的（过了时、编错了的）交回假。
    pub fn answer(&self, reply: Value) -> bool {
        let Some(echo) = reply["echo"].as_str() else {
            return false;
        };
        let sender = self
            .lock()
            .as_mut()
            .and_then(|pending| pending.remove(echo));
        match sender {
            Some(sender) => sender.send(reply).is_ok(),
            None => false,
        }
    }

    /// 连接断了：在等的都算失败，之后再调的也是。
    pub fn close(&self) {
        *self.lock() = None;
    }

    /// 不等了：过了时、发不出去的拿掉，回应晚到了也不认。
    fn forget(&self, echo: &str) {
        if let Some(pending) = self.lock().as_mut() {
            pending.remove(echo);
        }
    }

    /// 在等的那张表。锁里不 `await`，拿锁的一方不会在里面崩；真崩了，表照样能用。
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<HashMap<String, oneshot::Sender<Value>>>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Pending {
    /// 等回应，最多 [`Calls::new`] 给的那么久。
    ///
    /// # Errors
    ///
    /// 等的时候连接断了；过了时；回的 `status` 不是 `ok`。
    pub async fn wait(self) -> Result<Value, CallError> {
        let timeout = self.timeout;
        self.wait_for(timeout).await
    }

    /// 等回应，最多 `timeout`（施工 O-33：`get_image`、`get_file` 要 NapCat 先把东西下下来，等得比别的动作久）。
    ///
    /// # Errors
    ///
    /// 同 [`Pending::wait`]。
    pub async fn wait_for(self, timeout: Duration) -> Result<Value, CallError> {
        match tokio::time::timeout(timeout, self.answer).await {
            Err(_) => Err(CallError::Timeout),
            Ok(Err(_)) => Err(CallError::Closed),
            Ok(Ok(reply)) if reply["status"] == "ok" => Ok(reply),
            Ok(Ok(reply)) => Err(CallError::Failed(reply)),
        }
    }
}
