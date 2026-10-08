//! 扩展的状态的订阅（施工 9-4 补，`docs/blueprint/extensions.md`「推送」）：一个连接至多一个，一个转发任务。订阅的回应也交给
//! 它，它先写回应、再转推送：收到「哪个包变了」，照这一刻的状态、这个连接这一刻的语言算那一项，写成 `extension.changed`。
//! 推的是整项、照这一刻算，晚到的、重复的都对，不用编号排先后。读得慢、掉了队，推一条 `resync`（`{"stream":"extensions"}`），
//! 这个订阅就停了，头重新订阅。再订阅总是换一个新的。

use std::sync::Arc;

use serde_json::json;
use tokio::sync::broadcast::{self, error::RecvError};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::Core;
use crate::hello::Shaken;

/// 扩展的状态的订阅的转发任务。
#[derive(Debug)]
pub(super) struct ExtensionsForwarder {
    /// 交订阅的回应给它的那一头：只交一次。
    reply: Option<oneshot::Sender<String>>,
    /// 转发任务：连接断了、换了新的、取消了跟着停。
    task: JoinHandle<()>,
}

impl ExtensionsForwarder {
    /// 起一个：先等订阅的回应、写出去，再从 `pushes` 转，照 `shaken` 这个连接这一刻的语言算，写进 `out`。
    pub(super) fn start(
        core: Arc<Core>,
        shaken: Shaken,
        pushes: broadcast::Receiver<String>,
        out: mpsc::Sender<String>,
    ) -> ExtensionsForwarder {
        let (reply, answer) = oneshot::channel();
        let task = tokio::spawn(forward(core, shaken, pushes, answer, out));
        ExtensionsForwarder {
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

impl Drop for ExtensionsForwarder {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 转发：回应先写，再一条条推；掉了队推 `resync` 停下。
async fn forward(
    core: Arc<Core>,
    shaken: Shaken,
    mut pushes: broadcast::Receiver<String>,
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
            Ok(id) => {
                let Some(entry) = crate::extensions::entry(&core, &id, shaken.now(&core)) else {
                    continue;
                };
                let line = json!({"jsonrpc": "2.0", "method": "extension.changed", "params": {"entry": entry}});
                if out.send(line.to_string()).await.is_err() {
                    break;
                }
            }
            Err(RecvError::Lagged(_)) => {
                tracing::warn!(target: "miyu::endpoint", stream = "extensions", "lagged, resync");
                let line =
                    r#"{"jsonrpc":"2.0","method":"resync","params":{"stream":"extensions"}}"#;
                if out.send(line.to_string()).await.is_err() {
                    // 连接断了：没人收。
                }
                break;
            }
            Err(RecvError::Closed) => break,
        }
    }
}
