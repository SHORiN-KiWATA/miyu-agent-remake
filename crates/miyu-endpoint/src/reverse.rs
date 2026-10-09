//! 反向调用（施工 O-2 上，`docs/blueprint/providers.md`「怎么走」第 1、2 条）：核心发给一个连接的请求，编号 `core-<n>`，照连接
//! 从 1 数起；对上编号的回应交给等它的，对不上的不理。连接断了（[`Peer::close`]），在等的和以后发的都了结成 [`Gone`]。不等了的
//! （等它的 future 被丢掉）从表里拿掉；通知（[`Peer::notify`]，`tool.cancel`）不带编号、不等回应（施工 O-2 下）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

use crate::wire::Response;

/// 连接断了：发不出去，或者等不到回应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Gone;

/// 一个连接上核心这边的一头：往它发请求、收它的回应。克隆的是同一个。
#[derive(Clone)]
pub(crate) struct Peer {
    state: Arc<Mutex<State>>,
}

impl std::fmt::Debug for Peer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Peer")
    }
}

/// 发出去还没回的。
#[derive(Default)]
struct State {
    /// 上一条的编号。
    last: u64,
    /// 编号到等它的。
    waiting: HashMap<String, oneshot::Sender<Result<Value, Value>>>,
    /// 往连接写的一头；连接断了放掉，是 `None`（施工 O-2 再补：提供者表里存着这一头，留着它，写的那一头就一直不结束，
    /// 核心察觉不到扩展退出，崩了不重新拉起）。
    out: Option<mpsc::Sender<String>>,
}

impl Peer {
    /// 往 `out` 写的一头。
    pub(crate) fn new(out: mpsc::Sender<String>) -> Peer {
        Peer {
            state: Arc::new(Mutex::new(State {
                out: Some(out),
                ..State::default()
            })),
        }
    }

    /// 发一条请求，等它的回应：`result`，或者 `error` 的原样。
    ///
    /// # Errors
    ///
    /// 连接断了。
    pub(crate) async fn call(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Result<Value, Value>, Gone> {
        let (id, answer, out) = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(out) = state.out.clone() else {
                return Err(Gone);
            };
            state.last += 1;
            let id = format!("core-{}", state.last);
            let (tell, answer) = oneshot::channel();
            state.waiting.insert(id.clone(), tell);
            (id, answer, out)
        };
        // 不等了的（这个 future 被丢掉、发不出去、等到了）都从表里拿掉。
        let _waiting = Waiting {
            peer: self,
            id: &id,
        };
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        if out.send(line.to_string()).await.is_err() {
            return Err(Gone);
        }
        answer.await.map_err(|_| Gone)
    }

    /// 发一条通知（施工 O-2 下）：不带编号、不等回应。连接断了、写队列满了的发不出去，交回 `false`。
    pub(crate) fn notify(&self, method: &str, params: Value) -> bool {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(out) = state.out.as_ref() else {
            return false;
        };
        let line = json!({"jsonrpc": "2.0", "method": method, "params": params});
        out.try_send(line.to_string()).is_ok()
    }

    /// 收到一条回应：交给等它的，对不上的不理。
    pub(crate) fn answer(&self, response: Response) {
        let tell = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .waiting
            .remove(&response.id);
        if let Some(tell) = tell {
            drop(tell.send(response.outcome));
        }
    }

    /// 连接断了：在等的都了结，以后发的直接了结；往连接写的那一头放掉，写的任务才结束得了（施工 O-2 再补）。
    pub(crate) fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.waiting.clear();
        state.out = None;
    }

    /// 不等这一条了。
    fn forget(&self, id: &str) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .waiting
            .remove(id);
    }
}

/// 在等的一条：丢掉时从表里拿掉。
struct Waiting<'a> {
    peer: &'a Peer,
    id: &'a str,
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.peer.forget(self.id);
    }
}

#[cfg(test)]
mod tests;
