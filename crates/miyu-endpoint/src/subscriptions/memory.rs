//! 记忆日志的订阅（施工 R-12 上，`docs/blueprint/protocol.md` 的流 `memory`，`memory.md`「协议」）：一个连接一间至多一个，一个
//! 转发任务。照会话的事件流的「补发」：先把补的一行行写出去，再写订阅的回应（`{"upto"}`，经它写），再照先后推之后追加的；推的
//! 是记忆日志里那一行原样（`memory.event`）。读得慢、掉了队，推一条 `resync`，这个订阅就停了，头带着最后见过的序号重新订阅。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Map, Value};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use miyu_kernel::event::Event;
use miyu_session::Following;

/// 订阅的哪一间：照订阅时写的原样（`persona`、`session` 至多一个，都不写的是默认人格那一间），推送照原样带回去。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MemoryAt {
    pub(crate) persona: Option<String>,
    pub(crate) session: Option<String>,
}

impl MemoryAt {
    /// 推送里带回去的那几格。
    fn fields(&self) -> Map<String, Value> {
        let mut fields = Map::new();
        if let Some(persona) = &self.persona {
            fields.insert("persona".to_string(), Value::String(persona.clone()));
        }
        if let Some(session) = &self.session {
            fields.insert("session".to_string(), Value::String(session.clone()));
        }
        fields
    }
}

/// 一间记忆日志的订阅的转发任务。
#[derive(Debug)]
pub(super) struct MemoryForwarder {
    /// 交订阅的回应给它的那一头：只交一次。
    reply: Option<oneshot::Sender<String>>,
    /// 转发任务：连接断了、换了新的、取消了跟着停。
    task: JoinHandle<()>,
}

impl MemoryForwarder {
    /// 起一个：先写补的，再等订阅的回应、写出去，再照先后写之后的。
    pub(super) fn start(
        at: MemoryAt,
        following: Following,
        out: mpsc::Sender<String>,
    ) -> MemoryForwarder {
        let (reply, answer) = oneshot::channel();
        let task = tokio::spawn(forward(at, following, answer, out));
        MemoryForwarder {
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

impl Drop for MemoryForwarder {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 转发：补的、回应、之后的，照这个先后；`live` 关了的是掉了队（推 `resync`）或者日志那一头放下了（不说什么）。写队列关了
/// （连接断了）就停。
async fn forward(
    at: MemoryAt,
    following: Following,
    answer: oneshot::Receiver<String>,
    out: mpsc::Sender<String>,
) {
    let Following {
        filled,
        mut live,
        lagged,
        ..
    } = following;
    for event in &filled {
        if out.send(pushed(&at, event)).await.is_err() {
            return;
        }
    }
    let Ok(reply) = answer.await else {
        return;
    };
    if out.send(reply).await.is_err() {
        return;
    }
    while let Some(event) = live.recv().await {
        if out.send(pushed(&at, &event)).await.is_err() {
            return;
        }
    }
    resync_if_lagged(&at, &lagged, &out).await;
}

/// 掉了队的推一条 `resync`，记一行运行日志。
async fn resync_if_lagged(at: &MemoryAt, lagged: &Arc<AtomicBool>, out: &mpsc::Sender<String>) {
    if !lagged.load(Ordering::SeqCst) {
        return;
    }
    tracing::warn!(target: "miyu::endpoint", stream = "memory", "lagged, resync");
    let mut params = at.fields();
    params.insert("stream".to_string(), Value::String("memory".to_string()));
    let line = serde_json::json!({"jsonrpc": "2.0", "method": "resync", "params": params});
    if out.send(line.to_string()).await.is_err() {
        // 连接断了：没人收了。
    }
}

/// 推一行：`memory.event`，日志里那一行原样放在 `event`（同会话事件流的推送，不再解析一遍），后面接订阅时写的那一格。
fn pushed(at: &MemoryAt, event: &Event) -> String {
    let rest: String = at
        .fields()
        .iter()
        .map(|(name, value)| format!(",{}:{value}", Value::String(name.clone())))
        .collect();
    format!(
        r#"{{"jsonrpc":"2.0","method":"memory.event","params":{{"event":{}{rest}}}}}"#,
        event.to_line()
    )
}
