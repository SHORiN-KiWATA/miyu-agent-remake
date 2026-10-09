//! 贴表情（施工 O-25 下，`onebot.md` 第一条「贴表情」；18 第七节那张表的「贴表情」一列，旧版的效果）：群里冲她来、续聊她的那一条，
//! 判过要回的，先在那条消息上贴一个表情（`bridge.json` 的 `reaction_emoji`，出厂 QQ 表情 289），让人知道她看到了、在回；收了它的
//! 那一轮发出去第一段、那一轮结束、贴了以后过了 `reaction_seconds`，先到哪个算哪个，摘掉。
//!
//! - 什么时候贴（[`Route::react`]）：`called.rs` 判下来要回、`session.respond` 成了以后（「施工时定的」第 133 条），主触发是冲她来、
//!   续聊的；贴在判的几条里最后一条上（和引用同一条）。顶替接过去的，前一条贴着的摘掉（第 134 条）。
//! - 贴着的几条（[`Reactions`]）：会话、序号 → 进了哪一轮、摘的信号。推来的事件（[`Reactions::seen`]）：`turn.started`、
//!   `turn.joined` 记下进了哪一轮，那一轮的 `venue.delivered`、`turn.ended` 发摘的信号。
//! - 贴、等、摘是一个另起的任务（[`mark`]，放进 `Route` 的 `chores`）：三种摘法谁先到算谁，放在一个任务里自己就只摘一次
//!   （第 131 条）。不入出站队列、不记事件：贴、摘没成的记一行运行日志就算了；桥停下时贴着的不摘（施工单「要定的」第 3 条）。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use miyu_chat::{Kind, Supersede};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::Seq;
use tokio::sync::oneshot;

use super::decide::Decision;
use super::{Peer, Route};
use crate::TARGET;
use crate::listen::bots::Bots;
use crate::onebot::{CallError, emoji_like};

/// 贴着的几条，和贴什么、多久摘。
pub(crate) struct Reactions {
    /// 贴哪个表情（`bridge.json` 的 `reaction_emoji`）。
    emoji: String,
    /// 贴了以后过多久还没摘的摘掉（`reaction_seconds`）。
    after: Duration,
    /// （会话编号，序号）→ 贴着的那一条。任务已经完了的（到时候摘了、贴不上）下一次记新的时拿掉。
    held: HashMap<(String, u64), Held>,
}

/// 贴着的一条。
struct Held {
    /// 收了它的那一轮的回合编号；还没进哪一轮的是空的。
    turn: Option<u64>,
    /// 摘的信号：发给贴它的那个任务。
    off: oneshot::Sender<()>,
}

/// 贴在哪、贴什么。
#[derive(Debug, Clone)]
pub(super) struct Mark {
    /// 那个群：收进它的机器人号、群号，场所只记运行日志。
    pub(super) peer: Peer,
    /// 那一条的平台编号，原样（`venue.msg`）。
    pub(super) message: String,
    /// 表情的编号。
    pub(super) emoji: String,
}

impl Reactions {
    /// 一条都没贴：贴 `emoji`，贴了以后过了 `after` 摘。
    pub(crate) fn new(emoji: String, after: Duration) -> Reactions {
        Reactions {
            emoji,
            after,
            held: HashMap::new(),
        }
    }

    /// 记下会话 `session` 里序号是 `msg` 的那一条贴上了：交回摘的信号的那一头，交给贴它的任务。
    pub(super) fn hold(&mut self, session: &str, msg: u64) -> oneshot::Receiver<()> {
        self.held.retain(|_, held| !held.off.is_closed());
        let (off, signal) = oneshot::channel();
        let held = Held { turn: None, off };
        self.held.insert((session.to_string(), msg), held);
        signal
    }

    /// 摘会话 `session` 里序号是 `msg` 的那一条：发摘的信号，不再记着。没贴着的不管。
    pub(super) fn off(&mut self, session: &str, msg: u64) {
        if let Some(held) = self.held.remove(&(session.to_string(), msg))
            && held.off.send(()).is_err()
        {
            // 任务已经完了（到时候摘过了、贴不上）：没什么可摘的。
        }
    }

    /// 会话 `session` 推来的一条事件：`turn.started`、`turn.joined` 的触发里有贴着的，记下进了哪一轮（先进的那一轮算）；那一轮
    /// 发出去一段（`venue.delivered`）、结束了（`turn.ended`），摘它收了的那几条。
    pub(super) fn seen(&mut self, session: &str, event: &Event) {
        let turn = event.turn.map(|turn| turn.started().get());
        match (&event.body, turn) {
            (Body::TurnStarted(started), Some(turn)) => {
                self.taken(session, &started.triggers, turn)
            }
            (Body::TurnJoined(joined), Some(turn)) => self.taken(session, &joined.triggers, turn),
            (Body::VenueDelivered(delivered), _) => {
                self.done(session, delivered.turn.started().get())
            }
            (Body::TurnEnded(_), Some(turn)) => self.done(session, turn),
            _ => {}
        }
    }

    /// 那几条（`triggers`）进了回合编号是 `turn` 的那一轮。
    fn taken(&mut self, session: &str, triggers: &[Seq], turn: u64) {
        for seq in triggers {
            if let Some(held) = self.held.get_mut(&(session.to_string(), seq.get()))
                && held.turn.is_none()
            {
                held.turn = Some(turn);
            }
        }
    }

    /// 回合编号是 `turn` 的那一轮发出去了、完了：摘它收了的那几条。
    fn done(&mut self, session: &str, turn: u64) {
        let taken: Vec<u64> = self
            .held
            .iter()
            .filter(|((one, _), held)| one == session && held.turn == Some(turn))
            .map(|((_, msg), _)| *msg)
            .collect();
        for msg in taken {
            self.off(session, msg);
        }
    }
}

impl Route {
    /// 群会话 `session` 判下来要回、`session.respond` 成了的 `decision`（「贴表情」第 1 条）：主触发是冲她来、续聊的，在判的几条里
    /// 最后一条上贴，另起任务等着摘；顶替接过去的，前一条贴着的摘掉。那一条没有平台编号的（照说不会）不贴。
    pub(super) fn react(&mut self, session: &str, decision: &Decision) {
        let Some(passed) = &decision.passed else {
            return;
        };
        if !matches!(
            passed.conditions.primary(),
            Some(Kind::Direct | Kind::Continuation)
        ) {
            return;
        }
        if let Supersede::Inherit { msg, .. } = &passed.supersede {
            self.reactions.off(session, msg.get());
        }
        let (Some(&seq), Some(peer)) = (decision.msgs.last(), self.peers.get(session)) else {
            return;
        };
        let Some(message) = self.groups.get(session).and_then(|group| group.msg(seq)) else {
            return;
        };
        let target = Mark {
            peer: peer.clone(),
            message: message.to_string(),
            emoji: self.reactions.emoji.clone(),
        };
        let off = self.reactions.hold(session, seq);
        let bots = Arc::clone(&self.bots);
        self.chores
            .spawn(mark(target, off, bots, self.reactions.after));
    }
}

/// 贴上 `target`，等摘的信号 `off` 或者过了 `after`，摘掉（「贴表情」第 2、3 条）：贴、摘都经 `bots` 里那个机器人号那时的连接。
/// 贴不上的不摘；信号的一头放下了（桥在停）的不摘（「贴表情」第 4 条）。
pub(super) async fn mark(
    target: Mark,
    off: oneshot::Receiver<()>,
    bots: Arc<Bots>,
    after: Duration,
) {
    if !put(&target, &bots, true).await {
        return;
    }
    let due = tokio::select! {
        signalled = off => signalled.is_ok(),
        () = tokio::time::sleep(after) => true,
    };
    if due {
        put(&target, &bots, false).await;
    }
}

/// 在 `target` 上贴（`on`）、摘一次，记一行运行日志；成了交回真。那时没连着的照断了算（`Closed`）。
async fn put(target: &Mark, bots: &Bots, on: bool) -> bool {
    let (action, params) = emoji_like(&target.message, &target.emoji, on);
    let result = match bots.get(target.peer.bot) {
        Some(link) => link.calls.call(&link.out, action, params).await,
        None => Err(CallError::Closed),
    };
    let (venue, message) = (&target.peer.venue, &target.message);
    match (&result, on) {
        (Ok(_), true) => tracing::info!(target: TARGET, venue = %venue, message, "reaction set"),
        (Ok(_), false) => {
            tracing::info!(target: TARGET, venue = %venue, message, "reaction removed")
        }
        (Err(error), true) => {
            tracing::warn!(target: TARGET, venue = %venue, message, error = ?error, "reaction not set");
        }
        (Err(error), false) => {
            tracing::warn!(target: TARGET, venue = %venue, message, error = ?error, "reaction not removed");
        }
    }
    result.is_ok()
}

#[cfg(test)]
mod tests;
