//! 反向调用（施工 O-2 上，`docs/blueprint/providers.md`「怎么走」第 1、2 条）：核心发给一个连接的请求，编号 `core-<n>`，照连接
//! 从 1 数起；对上编号的回应交给等它的，对不上的不理。连接断了（[`Peer::close`]），在等的和以后发的都了结成 [`Gone`]。

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
    out: mpsc::Sender<String>,
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
    /// 连接断了。
    closed: bool,
}

impl Peer {
    /// 往 `out` 写的一头。
    pub(crate) fn new(out: mpsc::Sender<String>) -> Peer {
        Peer {
            out,
            state: Arc::default(),
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
        let (id, answer) = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if state.closed {
                return Err(Gone);
            }
            state.last += 1;
            let id = format!("core-{}", state.last);
            let (tell, answer) = oneshot::channel();
            state.waiting.insert(id.clone(), tell);
            (id, answer)
        };
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        if self.out.send(line.to_string()).await.is_err() {
            self.forget(&id);
            return Err(Gone);
        }
        answer.await.map_err(|_| Gone)
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

    /// 连接断了：在等的都了结，以后发的直接了结。
    pub(crate) fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.closed = true;
        state.waiting.clear();
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

#[cfg(test)]
mod tests;
