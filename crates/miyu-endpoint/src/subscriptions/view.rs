//! 视图流的转发任务（施工 9-8 下，`docs/blueprint/view.md`「视图流」）：会话的推送喂进订阅时那一页的投影，交出的变化写成
//! `view.add`、`view.update`、`view.append`、`view.hidden`、`view.remove` 推出去。
//!
//! - 订阅的回应带着最新一页的条目，它一定先写出去：之后的推送都是在它上面的变化。
//! - 读得慢的：攒着的推送一次喂完，同一条接着的 `view.append` 并成一条；还是掉了队的推 `resync`（`stream: "view"`），这个
//!   订阅停了，头重新订阅拿新的一页。
//! - 连接的 `ui.language` 改了，从下一批起照新的字（[`Projector::retext`]）。
//! - 会话状态（施工 9-8 补上，`view/status.rs`）：一批算完和上一份比，变了推整份 `view.status`。批里有落了盘的事件才向会话
//!   actor 重要一份「当前的」（用量、权限、工作区），只有增量的不打扰它。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::json;
use tokio::sync::mpsc;

use miyu_kernel::id::SessionId;
use miyu_session::{Current, Ended, Handle, Pushed, Subscription};
use miyu_view::{Change, Projector};

use crate::Core;
use crate::hello::Shaken;
use crate::view::status::{Fixed, whole};

/// 一次最多攒几份推送一起喂：再多就先写出去，免得回应等太久。
const BATCH: usize = 64;

/// 视图流要的：喂过订阅那一页的投影，换字时用的连接、语言，算会话状态要的会话、定下的几样、上一份。
pub(crate) struct View {
    core: Arc<Core>,
    shaken: Shaken,
    language: &'static str,
    projector: Projector,
    handle: Handle,
    fixed: Fixed,
    current: Option<Current>,
    status: serde_json::Value,
}

impl std::fmt::Debug for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("View")
            .field("language", &self.language)
            .finish_non_exhaustive()
    }
}

impl View {
    /// 订阅那一刻的：`projector` 喂过回应里那一页，字是 `language` 的；`current` 是和订阅同一步拿的「当前的」。
    pub(crate) fn new(
        core: Arc<Core>,
        shaken: Shaken,
        language: &'static str,
        projector: Projector,
        handle: Handle,
        fixed: Fixed,
        current: Option<Current>,
    ) -> View {
        let mut view = View {
            core,
            shaken,
            language,
            projector,
            handle,
            fixed,
            current,
            status: serde_json::Value::Null,
        };
        view.status = view.whole();
        view
    }

    /// 这一刻的整份会话状态：订阅的回应带它。
    pub(crate) fn status(&self) -> &serde_json::Value {
        &self.status
    }

    fn whole(&self) -> serde_json::Value {
        whole(
            &self.core,
            &self.handle,
            self.projector.status(),
            self.current.as_ref(),
            &self.fixed,
        )
    }

    /// 一批喂完以后：有落了盘的事件的，重要一份「当前的」；和上一份比，变了交回要推的那一条。
    async fn restatus(&mut self, session: &SessionId, persisted: bool) -> Option<String> {
        if persisted && let Ok(current) = self.handle.current().await {
            self.current = Some(current);
        }
        let status = self.whole();
        if status == self.status {
            return None;
        }
        self.status = status;
        Some(
            json!({"jsonrpc": "2.0", "method": "view.status",
                "params": {"session": session, "status": self.status}})
            .to_string(),
        )
    }

    /// 连接的语言改了：换一套字。读不成的照旧用原来的（运行日志里记过了）。
    async fn follow_language(&mut self) {
        let language = self.shaken.now(&self.core).language;
        if language == self.language {
            return;
        }
        let resources = self.core.resources.clone();
        let loaded =
            tokio::task::spawn_blocking(move || crate::view::texts(&resources, language)).await;
        if let Ok(Ok(texts)) = loaded {
            self.projector.retext(Arc::new(texts));
            self.language = language;
        }
    }

    /// 一批推送喂进投影，交回要写出去的几行，照先后。
    fn feed(&mut self, session: &SessionId, batch: &[Arc<Pushed>]) -> Vec<String> {
        let mut lines = Vec::new();
        let mut changes = Vec::new();
        for pushed in batch {
            match pushed.as_ref() {
                Pushed::Events(events) => {
                    for event in events {
                        changes.extend(self.projector.event(event));
                    }
                }
                Pushed::Transient(transient) => changes.extend(self.projector.transient(transient)),
            }
        }
        flush(session, &mut changes, &mut lines);
        lines
    }
}

/// 攒着的变化写成推送：接着的、同一条的 `view.append` 并成一条。
fn flush(session: &SessionId, changes: &mut Vec<Change>, lines: &mut Vec<String>) {
    let mut merged: Vec<Change> = Vec::with_capacity(changes.len());
    for change in changes.drain(..) {
        if let (
            Some(Change::Append { id, text }),
            Change::Append {
                id: next,
                text: more,
            },
        ) = (merged.last_mut(), &change)
            && id == next
        {
            text.push_str(more);
            continue;
        }
        merged.push(change);
    }
    lines.extend(merged.iter().map(|change| line(session, change)));
}

/// 一样变化写成一条推送。
fn line(session: &SessionId, change: &Change) -> String {
    let (method, params) = match change {
        Change::Add { entry, after } => (
            "view.add",
            json!({"session": session, "entry": entry, "after": after}),
        ),
        Change::Update { entry, after } => {
            let mut params = json!({"session": session, "entry": entry});
            if let Some(after) = after {
                params["after"] = json!(after);
            }
            ("view.update", params)
        }
        Change::Append { id, text } => (
            "view.append",
            json!({"session": session, "id": id, "text": text}),
        ),
        Change::Hidden { ids, hidden } => (
            "view.hidden",
            json!({"session": session, "ids": ids, "hidden": hidden}),
        ),
        Change::Remove { id } => ("view.remove", json!({"session": session, "id": id})),
    };
    json!({"jsonrpc": "2.0", "method": method, "params": params}).to_string()
}

/// 转发视图流：先写订阅的回应，再转推送和这个会话的回应；推送停了（掉了队、会话停了），接着转回应，直到连接不要它了。
/// 交回订阅。
pub(super) async fn forward(
    session: SessionId,
    mut subscription: Subscription,
    mut view: View,
    mut replies: mpsc::UnboundedReceiver<String>,
    out: mpsc::Sender<String>,
    pushing: Arc<AtomicBool>,
) -> Subscription {
    // 订阅的回应带着那一页，先写它。
    let first = match replies.recv().await {
        Some(first) => out.send(first).await.is_ok(),
        None => false,
    };
    if first {
        relay(&session, &mut subscription, &mut view, &mut replies, &out).await;
    }
    pushing.store(false, Ordering::Release);
    while let Some(reply) = replies.recv().await {
        if out.send(reply).await.is_err() {
            break;
        }
    }
    subscription
}

/// 转发的主循环：交回来的时候，这个订阅不再推了。
async fn relay(
    session: &SessionId,
    subscription: &mut Subscription,
    view: &mut View,
    replies: &mut mpsc::UnboundedReceiver<String>,
    out: &mpsc::Sender<String>,
) {
    loop {
        tokio::select! {
            biased;
            reply = replies.recv() => {
                let Some(reply) = reply else {
                    return;
                };
                // 先放已经到了的推送，再放回应：回应到手时，这条命令产生的推送一定已经到了。
                let alive = drain(session, subscription, view, None, out).await;
                if out.send(reply).await.is_err() || !alive {
                    return;
                }
            }
            next = subscription.next() => {
                if !drain(session, subscription, view, Some(next), out).await {
                    return;
                }
            }
        }
    }
}

/// 把 `first` 和已经到了的推送（最多 [`BATCH`] 份）一起喂进去、写出去。这个订阅不再往下了（掉了队、会话停了、连接断了），
/// 交回 `false`；掉队的、会话停了的先推一条 `resync`。
async fn drain(
    session: &SessionId,
    subscription: &mut Subscription,
    view: &mut View,
    first: Option<Result<Arc<Pushed>, Ended>>,
    out: &mpsc::Sender<String>,
) -> bool {
    let mut batch = Vec::new();
    let mut ended = None;
    let mut next = first.or_else(|| subscription.try_next());
    while let Some(item) = next {
        match item {
            Ok(pushed) => batch.push(pushed),
            Err(end) => {
                ended = Some(end);
                break;
            }
        }
        if batch.len() >= BATCH {
            break;
        }
        next = subscription.try_next();
    }
    view.follow_language().await;
    let persisted = batch
        .iter()
        .any(|pushed| matches!(pushed.as_ref(), Pushed::Events(_)));
    let mut lines = view.feed(session, &batch);
    lines.extend(view.restatus(session, persisted).await);
    for line in lines {
        if out.send(line).await.is_err() {
            return false;
        }
    }
    match ended {
        None => true,
        Some(end) => {
            match end {
                Ended::Lagged => {
                    tracing::warn!(target: "miyu::endpoint", session = session.as_str(), "view lagged, resync");
                }
                Ended::Stopped => {
                    tracing::info!(target: "miyu::endpoint", session = session.as_str(), "session stopped, view resync");
                }
            }
            // 写不出去也是停：连接断了。
            if out.send(resync(session)).await.is_err() {
                tracing::debug!(target: "miyu::endpoint", session = session.as_str(), "resync not written");
            }
            false
        }
    }
}

/// `resync` 通知：视图流掉了队，或者会话停了，这个订阅停了。
fn resync(session: &SessionId) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","method":"resync","params":{{"session":"{}","stream":"view"}}}}"#,
        session.as_str()
    )
}

#[cfg(test)]
mod tests;
