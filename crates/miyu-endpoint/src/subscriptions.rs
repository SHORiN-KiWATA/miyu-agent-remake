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
//!
//! 会话的视图流（施工 9-8 下，`subscriptions/view.rs`）占同一个位置：一个会话在一个连接上要么是事件流、要么是视图流，回应
//! 一样经它写出去。
//!
//! 配置的推送另有一个订阅（施工 8-4，`subscriptions/config.rs`）：一个连接至多一个，`config.set` 的回应经它写出去。会话列表
//! 的推送也是（施工 9-5，`subscriptions/sessions.rs`）：订阅的回应经它写出去，排在推送前面。扩展的状态、软件包列表的推送同会话
//! 列表（施工 9-4 补、F-8 三补，`subscriptions/listed.rs`）。
//!
//! 在后台答的回应（施工 R-7 补，`/dream`）办完了照那一刻的会话订阅走，表在 `subscriptions/later.rs`（[`Later`]）。记忆日志的
//! 订阅（施工 R-12 上，`subscriptions/memory.rs`）：一间至多一个，先补、再回应、再推。

mod config;
mod extension_config;
mod later;
mod listed;
mod memory;
mod sessions;
mod view;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use miyu_kernel::event::Event;
use miyu_kernel::id::SessionId;
use miyu_session::{Ended, Pushed, Subscription};

use crate::Core;
use config::ConfigForwarder;
use extension_config::ExtensionConfigForwarder;
pub(crate) use later::Later;
use listed::{Listed, ListedForwarder};
pub(crate) use memory::MemoryAt;
use memory::MemoryForwarder;
use sessions::SessionsForwarder;
pub(crate) use view::View;

/// 一个连接上的订阅：一个会话一个，配置的至多一个（施工 8-4）。
#[derive(Debug, Default)]
pub(crate) struct Subscriptions {
    live: BTreeMap<SessionId, Forwarder>,
    config: Option<ConfigForwarder>,
    sessions: Option<SessionsForwarder>,
    extensions: Option<ListedForwarder>,
    /// 软件包列表的推送（施工 F-8 三补）。
    packages: Option<ListedForwarder>,
    /// 核心拉起的扩展自己的配置的推送（施工 9-4 下下）：握手以后起，不用订阅。
    extension_config: Option<ExtensionConfigForwarder>,
    /// 会话订阅交回应的那一头的弱引用：开通道时一起记下，在后台答的回应照它走（施工 R-7 补）。
    later: Later,
    /// 记忆日志的订阅（施工 R-12 上）：照订阅时写的那一间，一间一个。
    memory: BTreeMap<MemoryAt, MemoryForwarder>,
}

/// 一条回应经哪个订阅写出去。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// 给这个会话的命令：经它的订阅，排在这条命令产生的推送后面。
    Session(SessionId),
    /// `config.set`：经配置的订阅，排在这一次的推送后面（施工 8-4）。
    Config,
    /// 会话列表的订阅的回应：经它写出去，排在之后的推送前面（施工 9-5）。
    Sessions,
    /// 扩展的状态的订阅的回应：同上（施工 9-4 补）。
    Extensions,
    /// 软件包列表的订阅的回应：同上（施工 F-8 三补）。
    Packages,
    /// 记忆日志的订阅的回应：经它写出去，排在补的后面、之后的推送前面（施工 R-12 上）。
    Memory(MemoryAt),
}

/// 一个订阅的转发任务，交回应给它的那一头，和这个订阅还在不在推。
#[derive(Debug)]
struct Forwarder {
    replies: mpsc::UnboundedSender<String>,
    pushing: Arc<AtomicBool>,
    /// 是视图流（施工 9-8 下）：一个会话在一个连接上只有一个订阅，事件流和视图流互相换掉。
    view: bool,
    /// 放完交回订阅：换一个新的时，旧的拿回来再放下，这个头一直算看着（施工 3-8 六补）。
    task: JoinHandle<Subscription>,
}

impl Subscriptions {
    /// 在后台答的任务拿的那一份（施工 R-7 补）：办完了照那一刻的会话订阅写回应。
    pub(crate) fn later(&self) -> Later {
        self.later.clone()
    }

    /// 这个会话有没有还在推的事件流订阅。掉了队、会话停了的，不算：头重新订阅，换一个新的；视图流也不算，订阅事件流换掉它。
    pub(crate) fn has(&self, session: &SessionId) -> bool {
        self.live
            .get(session)
            .is_some_and(|forwarder| !forwarder.view && forwarder.pushing.load(Ordering::Acquire))
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
        let (replies, waiting) = self.later.open(session.clone());
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
            view: false,
            task,
        };
        // 旧的不掐：丢掉它交回应的那一头，它放完排着的就退。
        drop(self.live.insert(session, forwarder));
    }

    /// 订阅会话 `session` 的视图流（施工 9-8 下）：起一个转发任务，先写订阅的回应（交给它的第一条回应，带着最新一页的条目），
    /// 再把推送喂进 `view` 的投影、写成变化。原来有一个的，调之前先 [`Subscriptions::take`] 拿掉。
    pub(crate) fn add_view(
        &mut self,
        session: SessionId,
        subscription: Subscription,
        view: View,
        out: mpsc::Sender<String>,
    ) {
        let (replies, waiting) = self.later.open(session.clone());
        let pushing = Arc::new(AtomicBool::new(true));
        let task = tokio::spawn(view::forward(
            session.clone(),
            subscription,
            view,
            waiting,
            out,
            Arc::clone(&pushing),
        ));
        let forwarder = Forwarder {
            replies,
            pushing,
            view: true,
            task,
        };
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

    /// 订阅配置的推送（施工 8-4）：还在推的，还是那一个；没有的、掉了队的，起一个新的。`system` 是 `ui.language` 是 `auto`
    /// 时用的语言。
    pub(crate) fn add_config(
        &mut self,
        core: &Arc<Core>,
        system: &'static str,
        out: &mpsc::Sender<String>,
    ) {
        if self.config.as_ref().is_some_and(ConfigForwarder::pushing) {
            return;
        }
        let pushes = core.hub.subscribe();
        self.config = Some(ConfigForwarder::start(
            Arc::clone(core),
            pushes,
            system,
            out.clone(),
        ));
    }

    /// 取消订阅配置的推送：已经交给转发任务的回应照样放完，它才退。
    pub(crate) fn remove_config(&mut self) {
        self.config = None;
    }

    /// 订阅会话列表的推送（施工 9-5）：先拿收推送的一头，再要一份列表（排在这之前的变化后面算），起一个新的转发任务替掉原来
    /// 的；交回订阅的回应 `{"sessions": […]}`，它经 [`Target::Sessions`] 交给新的转发任务写出去。
    pub(crate) async fn add_sessions(
        &mut self,
        core: &Arc<Core>,
        out: &mpsc::Sender<String>,
    ) -> Result<serde_json::Value, crate::refusal::Refusal> {
        let pushes = core.listing.subscribe();
        let snapshot = core.listing.snapshot(core).await?;
        self.sessions = Some(SessionsForwarder::start(
            pushes,
            snapshot.number,
            out.clone(),
        ));
        Ok(serde_json::json!({"sessions": snapshot.sessions}))
    }

    /// 取消订阅会话列表的推送：转发任务当场停。
    pub(crate) fn remove_sessions(&mut self) {
        self.sessions = None;
    }

    /// 核心拉起的扩展握了手（施工 9-4 下下，`extension_config.rs`）：包是 `package`，握手交出去的是 `handed`，`current` 是
    /// 握手时拿的盯配置的那一头；变了推 `extension.config`。握手的回应写出去以后调。
    pub(crate) fn add_extension_config(
        &mut self,
        package: String,
        handed: BTreeMap<String, serde_json::Value>,
        current: tokio::sync::watch::Receiver<Arc<crate::config::Config>>,
        out: &mpsc::Sender<String>,
    ) {
        self.extension_config = Some(ExtensionConfigForwarder::start(
            package,
            handed,
            current,
            out.clone(),
        ));
    }

    /// 订阅一个照编号推整项的列表（扩展的状态，施工 9-4 补；软件包列表，施工 F-8 三补）：先起新的转发任务（它先拿收推送的
    /// 几头，排在这之前的变化照样推，推的是这一刻的整项，重复了也对）替掉原来的，再算一份整的；交回订阅的回应
    /// （`{"extensions": […]}`、`{"packages": […]}`），它经 [`Target::Extensions`]、[`Target::Packages`] 交给新的转发任务写出去。
    /// `shaken` 是这个连接握手时记下的：推的时候照这一刻的语言挑名字。
    pub(crate) fn add_listed(
        &mut self,
        core: &Arc<Core>,
        packages: bool,
        shaken: crate::hello::Shaken,
        out: &mpsc::Sender<String>,
    ) -> Result<serde_json::Value, crate::refusal::Refusal> {
        let (listed, slot) = match packages {
            true => (Listed::Packages, &mut self.packages),
            false => (Listed::Extensions, &mut self.extensions),
        };
        *slot = Some(ListedForwarder::start(
            Arc::clone(core),
            listed,
            shaken,
            out.clone(),
        ));
        match listed {
            Listed::Packages => crate::packages::list(core, shaken.now(core)),
            Listed::Extensions => Ok(crate::extensions::status(core, shaken.now(core))),
        }
    }

    /// 取消订阅扩展的状态（`packages` 是假的）、软件包列表的推送：转发任务当场停。
    pub(crate) fn remove_listed(&mut self, packages: bool) {
        match packages {
            true => self.packages = None,
            false => self.extensions = None,
        }
    }

    /// 订阅记忆日志的 `at` 那一间（施工 R-12 上）：起一个转发任务，先写补的；原来订阅着这一间的换掉。订阅的回应经
    /// [`Target::Memory`] 交给它。
    pub(crate) fn add_memory(
        &mut self,
        at: MemoryAt,
        following: miyu_session::Following,
        out: &mpsc::Sender<String>,
    ) {
        let forwarder = MemoryForwarder::start(at.clone(), following, out.clone());
        drop(self.memory.insert(at, forwarder));
    }

    /// 取消订阅记忆日志的 `at` 那一间：转发任务当场停。没订阅着的不碍事。
    pub(crate) fn remove_memory(&mut self, at: &MemoryAt) {
        drop(self.memory.remove(at));
    }

    /// 写一条回应：`target` 订阅着的，交给它的转发任务，排在已经到了的推送后面；没订阅的直接放进写
    /// 队列。写队列关了（连接断了），交回 `false`。
    pub(crate) async fn reply(
        &mut self,
        target: Option<&Target>,
        line: String,
        out: &mpsc::Sender<String>,
    ) -> bool {
        let line = match target {
            Some(Target::Session(session)) => match self.live.get(session) {
                Some(forwarder) => match forwarder.replies.send(line) {
                    Ok(()) => return true,
                    Err(mpsc::error::SendError(line)) => line,
                },
                None => line,
            },
            Some(Target::Sessions) => match &mut self.sessions {
                Some(forwarder) => match forwarder.reply(line) {
                    Ok(()) => return true,
                    Err(line) => line,
                },
                None => line,
            },
            Some(target @ (Target::Extensions | Target::Packages)) => {
                let slot = match target {
                    Target::Packages => &mut self.packages,
                    _ => &mut self.extensions,
                };
                match slot {
                    Some(forwarder) => match forwarder.reply(line) {
                        Ok(()) => return true,
                        Err(line) => line,
                    },
                    None => line,
                }
            }
            Some(Target::Memory(at)) => match self.memory.get_mut(at) {
                Some(forwarder) => match forwarder.reply(line) {
                    Ok(()) => return true,
                    Err(line) => line,
                },
                None => line,
            },
            Some(Target::Config) => match &self.config {
                Some(forwarder) => match forwarder.reply(line) {
                    Ok(()) => return true,
                    Err(line) => line,
                },
                None => line,
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
        if let Some(config) = &self.config {
            config.abort();
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
