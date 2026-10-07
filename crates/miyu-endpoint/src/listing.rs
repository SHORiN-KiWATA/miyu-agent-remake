//! 会话列表的推送（施工 9-5，`docs/blueprint/protocol.md`「会话列表的推送」）：核心里一个排队的任务，「哪个会话变了」「删了」
//! 「要一份列表」都进它的队，一个接一个算，算好的每一条带一个递增的号，广播给订阅着的连接。订阅的回应里的列表也由它算、
//! 记下算到的号，转发时号不大于它的丢掉：回应以后的推送都比回应里的列表新。
//!
//! 会话报「变了」经会话表的端口（`SessionPort::listing`，`crate::spawn`）；造会话、派子代理、删会话由会话表自己报。一项照会话
//! 列表的索引只算那一个会话（[`crate::list::one`]），和 `session.list` 同一个算法。任务第一次用到时才起，拿着核心的弱引用：
//! 核心拿着它，它再强拿着核心就成了环。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, Weak};

use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc, oneshot};

use miyu_kernel::id::SessionId;

use crate::Core;
use crate::list::{list, one};
use crate::refusal::Refusal;

/// 广播最多攒多少条还没被读走的：读得慢的连接掉了队，推 `resync`。
const QUEUE: usize = 1024;

/// 一条推送：号和写好的那一行。
#[derive(Debug)]
pub(crate) struct Change {
    pub(crate) number: u64,
    pub(crate) line: String,
}

/// 订阅时的一份列表，和算它时排到的号。
pub(crate) struct Snapshot {
    pub(crate) number: u64,
    pub(crate) sessions: Vec<Value>,
}

/// 排队的活。
enum Job {
    Changed(SessionId),
    Removed(SessionId),
    List(oneshot::Sender<Result<Snapshot, Refusal>>),
}

/// 会话列表的推送：排队的那一头和广播。
#[derive(Debug)]
pub(crate) struct Listing {
    queue: OnceLock<mpsc::UnboundedSender<Job>>,
    pushes: broadcast::Sender<Arc<Change>>,
}

impl Default for Listing {
    fn default() -> Listing {
        Listing {
            queue: OnceLock::new(),
            pushes: broadcast::channel(QUEUE).0,
        }
    }
}

impl Listing {
    /// 收推送的一头：先拿它，再要列表，中间的变化一条不漏。
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<Arc<Change>> {
        self.pushes.subscribe()
    }

    /// 会话 `session` 那一项变了。
    pub(crate) fn changed(&self, core: &Arc<Core>, session: SessionId) {
        self.send(core, Job::Changed(session));
    }

    /// 会话 `session` 删了。
    pub(crate) fn removed(&self, core: &Arc<Core>, session: SessionId) {
        self.send(core, Job::Removed(session));
    }

    /// 一份列表，排在这之前报的变化后面算。
    pub(crate) async fn snapshot(&self, core: &Arc<Core>) -> Result<Snapshot, Refusal> {
        let (reply, answer) = oneshot::channel();
        self.send(core, Job::List(reply));
        answer.await.unwrap_or(Err(Refusal::INTERNAL))
    }

    /// 交给排队的任务，第一次用到时起它。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "任务没了只在核心正在退出时，没人要推送了"
    )]
    fn send(&self, core: &Arc<Core>, job: Job) {
        let queue = self.queue.get_or_init(|| {
            let (queue, jobs) = mpsc::unbounded_channel();
            tokio::spawn(work(Arc::downgrade(core), jobs, self.pushes.clone()));
            queue
        });
        let _ = queue.send(job);
    }
}

/// 排队的任务：一个接一个算，号一条一条往上加。
async fn work(
    core: Weak<Core>,
    mut jobs: mpsc::UnboundedReceiver<Job>,
    pushes: broadcast::Sender<Arc<Change>>,
) {
    let number = AtomicU64::new(0);
    let push = |params: Value| {
        let number = number.fetch_add(1, Ordering::Relaxed) + 1;
        let line =
            json!({"jsonrpc": "2.0", "method": "sessions.changed", "params": params}).to_string();
        // 没人订阅着的照样往上加号：订阅的回应记的是这一刻的号。
        drop(pushes.send(Arc::new(Change { number, line })));
    };
    while let Some(job) = jobs.recv().await {
        let Some(core) = core.upgrade() else {
            return;
        };
        match job {
            Job::Changed(session) => {
                if let Some(entry) = entry(&core, &session).await {
                    push(json!({"session": session.as_str(), "entry": entry}));
                }
            }
            Job::Removed(session) => push(json!({"session": session.as_str(), "removed": true})),
            Job::List(reply) => {
                let sessions = list(&core, false, None).await;
                let at = number.load(Ordering::Relaxed);
                drop(reply.send(sessions.map(|sessions| Snapshot {
                    number: at,
                    sessions,
                })));
            }
        }
    }
}

/// 算会话 `session` 的一项：不在了（删了、没有日志）的是空的，在阻塞线程里读索引和日志。
async fn entry(core: &Core, session: &SessionId) -> Option<Value> {
    let busy = core.sessions.busy_ids().await.contains(session);
    let (root, admin, index) = (core.root.clone(), core.admin.clone(), core.index.clone());
    let session = session.clone();
    // 场所会话不推（施工 O-3）：它不在本机的头上。
    tokio::task::spawn_blocking(move || {
        one(&root, &admin, &index, &session, busy)
            .filter(|listed| listed.venue == crate::list::LOCAL)
            .map(|listed| listed.to_json())
    })
    .await
    .ok()
    .flatten()
}

#[cfg(test)]
mod tests;
