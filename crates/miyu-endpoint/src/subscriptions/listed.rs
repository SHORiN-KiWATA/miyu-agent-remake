//! 照编号推整项的两个列表的订阅：扩展的状态（施工 9-4 补，`docs/blueprint/extensions.md`「推送」）、软件包列表（施工 F-8 三补，
//! `protocol.md`「软件包列表的推送」）。一个连接各至多一个，一个转发任务。订阅的回应也交给它，它先写回应、再转推送：收到
//! 「哪个包变了」，照这一刻的样子、这个连接这一刻的语言算那一项写出去。推的是整项、照这一刻算，晚到的、重复的都对，不用编号排
//! 先后。读得慢、掉了队，推一条 `resync`（`{"stream": "extensions"}`、`{"stream": "packages"}`），这个订阅就停了，头重新订阅。
//! 再订阅总是换一个新的。

use std::sync::Arc;

use serde_json::json;
use tokio::sync::broadcast::{self, error::RecvError};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::Core;
use crate::hello::Shaken;

/// 哪一个列表。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Listed {
    /// 扩展的状态：推 `extension.changed`。
    Extensions,
    /// 软件包列表：推 `package.changed`；扩展的状态变了，列表里那一项的状态、开关跟着变，也推。
    Packages,
}

impl Listed {
    /// 订阅的流叫什么。
    fn stream(self) -> &'static str {
        match self {
            Listed::Extensions => "extensions",
            Listed::Packages => "packages",
        }
    }

    /// 收「哪个包变了」的几头。
    fn receivers(
        self,
        core: &Core,
    ) -> (
        broadcast::Receiver<String>,
        Option<broadcast::Receiver<String>>,
    ) {
        match self {
            Listed::Extensions => (core.extensions.subscribe(), None),
            Listed::Packages => (
                core.package_changes.subscribe(),
                Some(core.extensions.subscribe()),
            ),
        }
    }

    /// 包 `id` 变了推的那一行。扩展的状态里没有这个包的不推；软件包列表里没有了的推 `entry: null`。
    fn line(self, core: &Core, id: &str, shaken: Shaken) -> Option<String> {
        let line = match self {
            Listed::Extensions => {
                let entry = crate::extensions::entry(core, id, shaken.now(core))?;
                json!({"jsonrpc": "2.0", "method": "extension.changed", "params": {"entry": entry}})
            }
            Listed::Packages => {
                let entry = crate::packages::entry(core, id, shaken.now(core));
                json!({"jsonrpc": "2.0", "method": "package.changed", "params": {"entry": entry, "package": id}})
            }
        };
        Some(line.to_string())
    }
}

/// 一个列表的订阅的转发任务。
#[derive(Debug)]
pub(super) struct ListedForwarder {
    /// 交订阅的回应给它的那一头：只交一次。
    reply: Option<oneshot::Sender<String>>,
    /// 转发任务：连接断了、换了新的、取消了跟着停。
    task: JoinHandle<()>,
}

impl ListedForwarder {
    /// 起一个：先拿收推送的几头（排在这之后的变化照样推），等订阅的回应、写出去，再转推送，照 `shaken` 这个连接这一刻的语言
    /// 算，写进 `out`。
    pub(super) fn start(
        core: Arc<Core>,
        listed: Listed,
        shaken: Shaken,
        out: mpsc::Sender<String>,
    ) -> ListedForwarder {
        let (reply, answer) = oneshot::channel();
        let pushes = listed.receivers(&core);
        let task = tokio::spawn(forward(core, listed, shaken, pushes, answer, out));
        ListedForwarder {
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

impl Drop for ListedForwarder {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 转发：回应先写，再一条条推；掉了队推 `resync` 停下。
async fn forward(
    core: Arc<Core>,
    listed: Listed,
    shaken: Shaken,
    (mut first, mut second): (
        broadcast::Receiver<String>,
        Option<broadcast::Receiver<String>>,
    ),
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
        let got = tokio::select! {
            got = first.recv() => got,
            got = next(&mut second) => got,
        };
        match got {
            Ok(id) => {
                let Some(line) = listed.line(&core, &id, shaken) else {
                    continue;
                };
                if out.send(line).await.is_err() {
                    break;
                }
            }
            Err(RecvError::Lagged(_)) => {
                tracing::warn!(target: "miyu::endpoint", stream = listed.stream(), "lagged, resync");
                let line = json!({"jsonrpc": "2.0", "method": "resync", "params": {"stream": listed.stream()}});
                if out.send(line.to_string()).await.is_err() {
                    // 连接断了：没人收。
                }
                break;
            }
            Err(RecvError::Closed) => break,
        }
    }
}

/// 第二头的下一条；没有第二头的永远等着。
async fn next(receiver: &mut Option<broadcast::Receiver<String>>) -> Result<String, RecvError> {
    match receiver {
        Some(receiver) => receiver.recv().await,
        None => std::future::pending().await,
    }
}
