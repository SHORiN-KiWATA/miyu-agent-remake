//! 在后台答的回应经哪个订阅写出去（施工 R-7 补，`protocol.md`「一个连接」第 1 条、「先见结果，后见回应」第 1 条）：`/dream`
//! 在后台答，办完了照这一刻的订阅找：给会话的、这个会话这时在这个连接上有订阅的，交给它的转发任务，排在已经到了的推送（它
//! 那条 `command.ran`）后面，和按顺序答的命令一样；别的直接放进写队列。
//!
//! 只放交回应的那一头的弱引用，开通道时一起记下（[`Later::open`]，记和开是一步，漏不了）：后台的任务拿到了只用一下，换订阅、
//! 取消订阅不等它（换的时候要等旧的转发任务放完，拿着强的一头的话，要等到整理完）。取消了、换掉了的，强的一头跟着转发任务的
//! 登记放下，弱的再也拿不到，不用另外拿掉；下一次开通道时顺手清掉。

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::sync::mpsc;

use miyu_kernel::id::SessionId;

use super::Target;

/// 这个连接上每个会话的订阅交回应的那一头（弱的）：连接和它在后台答的任务共用一份。
#[derive(Debug, Clone, Default)]
pub(crate) struct Later(Arc<Mutex<BTreeMap<SessionId, mpsc::WeakUnboundedSender<String>>>>);

impl Later {
    /// 给会话 `session` 的新订阅开一条交回应的通道，记下它（弱的，替掉原来那一条）：交回交的一头（转发任务的登记拿着）和收的
    /// 一头（转发任务拿着）。没人拿着交的一头的顺手清掉，表不越攒越多。
    pub(super) fn open(
        &self,
        session: SessionId,
    ) -> (
        mpsc::UnboundedSender<String>,
        mpsc::UnboundedReceiver<String>,
    ) {
        let (replies, waiting) = mpsc::unbounded_channel();
        let mut map = self.map();
        map.retain(|_, weak| weak.strong_count() > 0);
        map.insert(session, replies.downgrade());
        (replies, waiting)
    }

    /// 写一条在后台答的回应：`target` 是会话、这时有订阅的，交给它的转发任务；别的、那个转发任务不收了的，直接放进写队列
    /// `out`。写队列关了（连接断了）不要紧，没人收了。
    pub(crate) async fn reply(
        &self,
        target: Option<&Target>,
        line: String,
        out: &mpsc::Sender<String>,
    ) {
        let forwarder = match target {
            Some(Target::Session(session)) => self
                .map()
                .get(session)
                .and_then(mpsc::WeakUnboundedSender::upgrade),
            _ => None,
        };
        let line = match forwarder {
            Some(replies) => match replies.send(line) {
                Ok(()) => return,
                Err(mpsc::error::SendError(line)) => line,
            },
            None => line,
        };
        if out.send(line).await.is_err() {
            // 连接断了：这条回应没人收了。
        }
    }

    /// 那一张表：拿着锁的一方崩了也照样用（表里只是弱引用，改到一半也不会错）。
    fn map(&self) -> MutexGuard<'_, BTreeMap<SessionId, mpsc::WeakUnboundedSender<String>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
