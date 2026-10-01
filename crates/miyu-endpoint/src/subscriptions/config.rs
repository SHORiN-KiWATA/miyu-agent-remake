//! 配置的订阅（`docs/blueprint/config.md`「协议」「订阅配置的推送」，施工 8-4）：一个连接至多一个，一个转发任务。它读配置
//! 服务的推送，照这个连接这一刻的语言写成 `config.changed` 放进连接的写队列；读得慢、掉了队，推一条 `resync`
//! （`{"stream":"config"}`），这个订阅就停了，头重新订阅、`config.get` 补上。
//!
//! `config.set` 的回应也交给它（照会话的订阅那样，`subscriptions.rs`）：配置服务先推、后回应，回应到手时这一次的推送一定
//! 已经在它的队列里了；先把已经到了的推送放进写队列，再放回应，发 `config.set` 的那个连接先见推送、后见回应。停了以后
//! 接着转回应，直到连接不要它了。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::broadcast::{self, error::RecvError, error::TryRecvError};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use miyu_store::human::Human;

use crate::Core;
use crate::config::push::Changed;
use crate::hello::language_of;

/// 配置的订阅的转发任务。
#[derive(Debug)]
pub(super) struct ConfigForwarder {
    /// 交回应给它的那一头。
    replies: mpsc::UnboundedSender<String>,
    /// 还在不在推。
    pushing: Arc<AtomicBool>,
    /// 转发任务：连接断了跟着停。
    task: JoinHandle<()>,
}

impl ConfigForwarder {
    /// 起一个：从 `pushes` 读，照 `system`（`ui.language` 是 `auto` 时用的那一种）和推送带着的配置定语言，写进 `out`。
    pub(super) fn start(
        core: Arc<Core>,
        pushes: broadcast::Receiver<Arc<Changed>>,
        system: &'static str,
        out: mpsc::Sender<String>,
    ) -> ConfigForwarder {
        let (replies, waiting) = mpsc::unbounded_channel();
        let pushing = Arc::new(AtomicBool::new(true));
        let task = tokio::spawn(forward(
            Writer { core, system, out },
            pushes,
            waiting,
            Arc::clone(&pushing),
        ));
        ConfigForwarder {
            replies,
            pushing,
            task,
        }
    }

    /// 还在推：掉了队的不算，头重新订阅，换一个新的。
    pub(super) fn pushing(&self) -> bool {
        self.pushing.load(Ordering::Acquire)
    }

    /// 连接断了：转发任务跟着停。取消订阅、换一个新的不叫它：丢掉这一个，交回应的那一头跟着没了，它把已经交给它的回应
    /// 放完再退。
    pub(super) fn abort(&self) {
        self.task.abort();
    }

    /// 交一条回应给它；它已经不收了的，交回这一行。
    pub(super) fn reply(&self, line: String) -> Result<(), String> {
        self.replies
            .send(line)
            .map_err(|mpsc::error::SendError(line)| line)
    }
}

/// 往写队列里写：照哪一种语言、写到哪。
struct Writer {
    core: Arc<Core>,
    system: &'static str,
    out: mpsc::Sender<String>,
}

impl Writer {
    /// 写一条推送：照推送带着的配置定这一刻的语言。要用的字缺了的这一条不推（已经记了 `WARN`）。写队列关了交回 `false`。
    async fn push(&self, changed: &Changed) -> bool {
        let language = language_of(&changed.config, self.system);
        let line = match Human::load(&self.core.resources, language) {
            Ok(words) => changed.line(&words),
            Err(error) => {
                tracing::warn!(target: "miyu::config", error = %error, "resource unreadable");
                return true;
            }
        };
        match line {
            Ok(line) => self.out.send(line).await.is_ok(),
            Err(_) => true,
        }
    }

    /// 掉了队：推一条 `resync`。
    async fn resync(&self) -> bool {
        tracing::warn!(target: "miyu::endpoint", stream = "config", "lagged, resync");
        let line = r#"{"jsonrpc":"2.0","method":"resync","params":{"stream":"config"}}"#;
        self.out.send(line.to_string()).await.is_ok()
    }
}

/// 转发：先转推送和回应；推送停了（掉了队），接着转回应，直到连接不要它了。
async fn forward(
    writer: Writer,
    mut pushes: broadcast::Receiver<Arc<Changed>>,
    mut replies: mpsc::UnboundedReceiver<String>,
    pushing: Arc<AtomicBool>,
) {
    relay(&writer, &mut pushes, &mut replies).await;
    pushing.store(false, Ordering::Release);
    while let Some(reply) = replies.recv().await {
        if writer.out.send(reply).await.is_err() {
            break;
        }
    }
}

/// 转发的主循环：交回来的时候，这个订阅不再推了（掉了队、连接断了、连接不要它了）。
async fn relay(
    writer: &Writer,
    pushes: &mut broadcast::Receiver<Arc<Changed>>,
    replies: &mut mpsc::UnboundedReceiver<String>,
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
                loop {
                    let next = match pushes.try_recv() {
                        Ok(changed) => writer.push(&changed).await,
                        Err(TryRecvError::Lagged(_)) => {
                            writer.resync().await;
                            false
                        }
                        Err(TryRecvError::Empty | TryRecvError::Closed) => break,
                    };
                    if !next {
                        alive = false;
                        break;
                    }
                }
                if writer.out.send(reply).await.is_err() || !alive {
                    return;
                }
            }
            next = pushes.recv() => {
                let alive = match next {
                    Ok(changed) => writer.push(&changed).await,
                    Err(RecvError::Lagged(_)) => {
                        writer.resync().await;
                        false
                    }
                    // 配置服务没了：核心在停。
                    Err(RecvError::Closed) => false,
                };
                if !alive {
                    return;
                }
            }
        }
    }
}
