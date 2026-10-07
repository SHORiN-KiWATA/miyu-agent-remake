//! 会话列表的订阅（施工 9-5，`docs/blueprint/protocol.md`「会话列表的推送」）：一个连接至多一个，一个转发任务。订阅的回应
//! 也交给它，它先写回应、再转推送：号不大于回应里那份列表的推送丢掉，之后的照先后写成 `sessions.changed`。读得慢、掉了队，
//! 推一条 `resync`（`{"stream":"sessions"}`），这个订阅就停了，头重新订阅。再订阅总是换一个新的。

use std::sync::Arc;

use tokio::sync::broadcast::{self, error::RecvError};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::listing::Change;

/// 会话列表的订阅的转发任务。
#[derive(Debug)]
pub(super) struct SessionsForwarder {
    /// 交订阅的回应给它的那一头：只交一次。
    reply: Option<oneshot::Sender<String>>,
    /// 转发任务：连接断了、换了新的、取消了跟着停。
    task: JoinHandle<()>,
}

impl SessionsForwarder {
    /// 起一个：先等订阅的回应、写出去，再从 `pushes` 转号大于 `after` 的，写进 `out`。
    pub(super) fn start(
        pushes: broadcast::Receiver<Arc<Change>>,
        after: u64,
        out: mpsc::Sender<String>,
    ) -> SessionsForwarder {
        let (reply, answer) = oneshot::channel();
        let task = tokio::spawn(forward(pushes, after, answer, out));
        SessionsForwarder {
            reply: Some(reply),
            task,
        }
    }

    /// 交订阅的回应；已经交过、它已经不收了的，交回这一行。
    pub(super) fn reply(&mut self, line: String) -> Result<(), String> {
        match self.reply.take() {
            Some(reply) => reply.send(line),
            None => Err(line),
        }
    }
}

impl Drop for SessionsForwarder {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 转发：回应先写，再照先后写推送；掉了队推 `resync` 停下。
async fn forward(
    mut pushes: broadcast::Receiver<Arc<Change>>,
    after: u64,
    answer: oneshot::Receiver<String>,
    out: mpsc::Sender<String>,
) {
    let Ok(reply) = answer.await else {
        return;
    };
    if out.send(reply).await.is_err() {
        return;
    }
    loop {
        match pushes.recv().await {
            Ok(change) if change.number <= after => {}
            Ok(change) => {
                if out.send(change.line.clone()).await.is_err() {
                    break;
                }
            }
            Err(RecvError::Lagged(_)) => {
                tracing::warn!(target: "miyu::endpoint", stream = "sessions", "lagged, resync");
                let line = r#"{"jsonrpc":"2.0","method":"resync","params":{"stream":"sessions"}}"#;
                drop(out.send(line.to_string()).await);
                break;
            }
            Err(RecvError::Closed) => break,
        }
    }
}

#[cfg(test)]
mod tests;
