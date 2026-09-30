//! 订阅（`docs/designs/04-核心协议.md` 第五节「三种订阅」、第六节第 2 条、第七节，施工 3-8 中）：一个连接上
//! 每个订阅一个转发任务。它读会话的推送，写成 `event` 通知放进连接的写队列；读得慢、掉了队，推一条
//! `resync`，这个订阅就停了，头重新订阅。
//!
//! 订阅了的会话，命令的回应也交给它的转发任务：会话 actor 先推送、后回应（施工 3-7 中），回应到手时，
//! 这条命令产生的推送一定已经到了；转发任务先把已经到了的推送都放进写队列，再放回应。订阅停了（掉了队、
//! 会话停了），转发任务不退，接着替这个会话转回应，直到连接不要它了：回应一条都不丢，也不用猜它停在
//! 哪一步。取消订阅、换一个新的订阅时，旧的转发任务也不掐：只关掉交回应给它的那一头，它把已经交给它的
//! 回应放完再退（施工 4-9 再补三上）。
//!
//! 带 `after` 订阅的（施工 3-8 六补），转发任务先把补发的那一截一条条放进写队列，再转推送、回应：订阅的回应也交给它，
//! 排在补的后面。换掉原来那一个时，先等它把交给它的都放完、交回它的订阅（[`Subscriptions::take`]），补的才不和它的交错。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use miyu_kernel::event::Event;
use miyu_kernel::id::SessionId;
use miyu_session::{Ended, Pushed, Subscription};

/// 一个连接上的订阅：一个会话一个。
#[derive(Debug, Default)]
pub(crate) struct Subscriptions {
    live: BTreeMap<SessionId, Forwarder>,
}

/// 一个订阅的转发任务，交回应给它的那一头，和这个订阅还在不在推。
#[derive(Debug)]
struct Forwarder {
    replies: mpsc::UnboundedSender<String>,
    pushing: Arc<AtomicBool>,
    /// 放完交回订阅：换一个新的时，旧的拿回来再放下，这个头一直算看着（施工 3-8 六补）。
    task: JoinHandle<Subscription>,
}

impl Subscriptions {
    /// 这个会话有没有还在推的订阅。掉了队、会话停了的，不算：头重新订阅，换一个新的。
    pub(crate) fn has(&self, session: &SessionId) -> bool {
        self.live
            .get(session)
            .is_some_and(|forwarder| forwarder.pushing.load(Ordering::Acquire))
    }

    /// 订阅会话 `session`：起一个转发任务，先把补发的 `backlog` 照先后写进 `out`（不补的是空的），再写推送。原来有一个的，
    /// 换掉：旧的放完已经交给它的回应再退。
    pub(crate) fn add(
        &mut self,
        session: SessionId,
        subscription: Subscription,
        backlog: Vec<Event>,
        out: mpsc::Sender<String>,
    ) {
        let (replies, waiting) = mpsc::unbounded_channel();
        let pushing = Arc::new(AtomicBool::new(true));
        let task = tokio::spawn(forward(
            session.clone(),
            subscription,
            backlog,
            waiting,
            out,
            Arc::clone(&pushing),
        ));
        let forwarder = Forwarder {
            replies,
            pushing,
            task,
        };
        // 旧的不掐：丢掉它交回应的那一头，它放完排着的就退。
        drop(self.live.insert(session, forwarder));
    }

    /// 取消订阅会话 `session`：不再推它的事件。已经交给转发任务的回应照样放完，它才退（施工 4-9 再补三上：
    /// 原来当场掐掉，还没写出去的回应就丢了）。
    pub(crate) fn remove(&mut self, session: &SessionId) {
        drop(self.live.remove(session));
    }

    /// 拿掉会话 `session` 的订阅（施工 3-8 六补）：不再交回应给它，等它把已经交给它的推送、回应都放进写队列，交回它的订阅；
    /// 没有的、转发任务没了的（不会：它只在连接断了时被掐掉），交回 `None`。交回的订阅还算这个头看着，放下它才不算。
    pub(crate) async fn take(&mut self, session: &SessionId) -> Option<Subscription> {
        let forwarder = self.live.remove(session)?;
        drop(forwarder.replies);
        forwarder.task.await.ok()
    }

    /// 写一条回应：`session` 订阅着的，交给它的转发任务，排在已经到了的推送后面；没订阅的直接放进写
    /// 队列。写队列关了（连接断了），交回 `false`。
    pub(crate) async fn reply(
        &mut self,
        session: Option<&SessionId>,
        line: String,
        out: &mpsc::Sender<String>,
    ) -> bool {
        let line = match session.and_then(|session| self.live.get(session)) {
            Some(forwarder) => match forwarder.replies.send(line) {
                Ok(()) => return true,
                Err(mpsc::error::SendError(line)) => line,
            },
            None => line,
        };
        out.send(line).await.is_ok()
    }
}

/// 连接断了：转发任务跟着停。
impl Drop for Subscriptions {
    fn drop(&mut self) {
        for forwarder in self.live.values() {
            forwarder.task.abort();
        }
    }
}

/// 转发一个订阅：先补发 `backlog`，再转推送。推送停了（掉了队、会话停了），接着转这个会话的回应，直到连接不要它了；
/// 交回订阅。
async fn forward(
    session: SessionId,
    mut subscription: Subscription,
    backlog: Vec<Event>,
    mut replies: mpsc::UnboundedReceiver<String>,
    out: mpsc::Sender<String>,
    pushing: Arc<AtomicBool>,
) -> Subscription {
    if replay(&session, backlog, &out).await {
        relay(&session, &mut subscription, &mut replies, &out).await;
    }
    pushing.store(false, Ordering::Release);
    while let Some(reply) = replies.recv().await {
        if out.send(reply).await.is_err() {
            break;
        }
    }
    subscription
}

/// 补发（施工 3-8 六补）：照先后一条条写成 `event` 通知放进写队列，和推过来的一样。写队列关了（连接断了），交回 `false`。
async fn replay(session: &SessionId, backlog: Vec<Event>, out: &mpsc::Sender<String>) -> bool {
    for event in backlog {
        if out
            .send(notification(session, &event.to_line()))
            .await
            .is_err()
        {
            return false;
        }
    }
    true
}

/// 转发的主循环：交回来的时候，这个订阅不再推了（连接不要它了的，也交回来）。
async fn relay(
    session: &SessionId,
    subscription: &mut Subscription,
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
                // 先放已经到了的推送；放着放着掉了队，这条回应也照样放出去再停。
                let mut alive = true;
                while let Some(next) = subscription.try_next() {
                    if !deliver(session, next, out).await {
                        alive = false;
                        break;
                    }
                }
                if out.send(reply).await.is_err() || !alive {
                    return;
                }
            }
            next = subscription.next() => {
                if !deliver(session, next, out).await {
                    return;
                }
            }
        }
    }
}

/// 交一份推送出去。这个订阅不再往下了（掉了队、会话停了、连接断了），交回 `false`；掉队的、会话停了的先推一条
/// `resync`。
async fn deliver(
    session: &SessionId,
    next: Result<Arc<Pushed>, Ended>,
    out: &mpsc::Sender<String>,
) -> bool {
    match next {
        Ok(pushed) => {
            for line in lines(session, &pushed) {
                if out.send(line).await.is_err() {
                    return false;
                }
            }
            true
        }
        Err(Ended::Lagged) => {
            tracing::warn!(target: "miyu::endpoint", session = session.as_str(), "lagged, resync");
            if out.send(resync(session)).await.is_err() {
                return false;
            }
            false
        }
        // 会话停了（施工 4-9 再补三上）：也推一条 `resync`，头知道这个订阅没了；重新订阅时，会话照会话表重新载入。
        Err(Ended::Stopped) => {
            tracing::info!(target: "miyu::endpoint", session = session.as_str(), "session stopped, resync");
            if out.send(resync(session)).await.is_err() {
                return false;
            }
            false
        }
    }
}

/// 一份推送写成 `event` 通知：一条事件一个，落了盘的、瞬时的都照 `03-事件模型.md` 的写法原样放进去。
fn lines(session: &SessionId, pushed: &Pushed) -> Vec<String> {
    match pushed {
        Pushed::Events(events) => events
            .iter()
            .map(|event| notification(session, &event.to_line()))
            .collect(),
        Pushed::Transient(transient) => vec![notification(session, &transient.to_line())],
    }
}

/// `event` 通知。事件已经是一行紧凑的 JSON，原样嵌进去，不再解一遍、写一遍。
fn notification(session: &SessionId, event: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","method":"event","params":{{"session":"{}","event":{event}}}}}"#,
        session.as_str()
    )
}

/// `resync` 通知：这个订阅掉了队，或者会话停了，这个订阅停了。
fn resync(session: &SessionId) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","method":"resync","params":{{"session":"{}","stream":"events"}}}}"#,
        session.as_str()
    )
}
