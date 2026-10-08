//! 订阅一个会话的推送（`docs/blueprint/session/actor.md`「推送和订阅」）：施工 9-6 上从 `handle.rs` 挪出来（那一份到了 500 行）。

use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::mpsc;

use super::{Message, Pushed};
use crate::current::Current;

/// 一个订阅。
#[derive(Debug)]
pub struct Subscription {
    pushes: broadcast::Receiver<Arc<Pushed>>,
    /// 掉过队了：这个订阅作废，头重新订阅。
    lagged: bool,
    /// 放下时告诉 actor 少了一个看着的头（施工 7-9）；测试里直接造的没有。只为了它放下的那一刻拿着，不读。
    _watching: Option<Watching>,
    /// 订阅那一刻「当前的」几样（施工 9-6 上）：和订阅在 actor 的同一步里拿。测试里直接造的没有。
    current: Option<Current>,
}

/// 一个看着会话的头：跟着订阅走，放下时往 actor 的收件箱送一声（施工 7-9）。拿的是弱的一头：不因为还有订阅，就不让
/// 拿着 `Handle` 的都放下以后 actor 退出（`docs/blueprint/session/actor.md` 第 9 条）。
#[derive(Debug)]
pub(super) struct Watching(pub(super) mpsc::WeakUnboundedSender<Message>);

impl Drop for Watching {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "actor 已经退出了：没人要知道少了一个头，丢掉"
    )]
    fn drop(&mut self) {
        if let Some(inbox) = self.0.upgrade() {
            let _ = inbox.send(Message::Unsubscribed);
        }
    }
}

/// 订阅断了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// 读得太慢，掉了队：中间漏了推送，这个订阅作废，要重新订阅（`04-核心协议.md` 第七节的 resync）。
    Lagged,
    /// 会话停了。
    Stopped,
}

impl Subscription {
    pub(crate) fn new(pushes: broadcast::Receiver<Arc<Pushed>>) -> Subscription {
        Subscription {
            pushes,
            lagged: false,
            _watching: None,
            current: None,
        }
    }

    /// 向 actor 要来的：放下时告诉它（`watching`），带着那一刻的几样（`current`）。
    pub(super) fn watched(mut self, watching: Watching, current: Current) -> Subscription {
        self._watching = Some(watching);
        self.current = Some(current);
        self
    }

    /// 订阅那一刻「当前的」几样（施工 9-6 上）：人设的权限、还在跑的任务、累计的用量和计数。协议照它回 `subscribe`。
    pub fn current(&self) -> Option<&Current> {
        self.current.as_ref()
    }

    /// 下一份推送。
    ///
    /// # Errors
    ///
    /// 掉了队，或者会话停了。掉过一次队，以后一直是 [`Ended::Lagged`]。
    pub async fn next(&mut self) -> Result<Arc<Pushed>, Ended> {
        if self.lagged {
            return Err(Ended::Lagged);
        }
        match self.pushes.recv().await {
            Ok(pushed) => Ok(pushed),
            Err(RecvError::Lagged(_)) => {
                self.lagged = true;
                Err(Ended::Lagged)
            }
            Err(RecvError::Closed) => Err(Ended::Stopped),
        }
    }

    /// 不等：已经到了的下一份；还没到的，交回 `None`。协议端点收到命令的回应时，先把已经到了的
    /// 推送都写出去，再写回应（`04-核心协议.md` 第六节第 2 条）。
    ///
    /// # Errors
    ///
    /// 同 [`Subscription::next`]。
    pub fn try_next(&mut self) -> Option<Result<Arc<Pushed>, Ended>> {
        if self.lagged {
            return Some(Err(Ended::Lagged));
        }
        match self.pushes.try_recv() {
            Ok(pushed) => Some(Ok(pushed)),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Lagged(_)) => {
                self.lagged = true;
                Some(Err(Ended::Lagged))
            }
            Err(TryRecvError::Closed) => Some(Err(Ended::Stopped)),
        }
    }
}
