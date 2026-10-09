//! 命令回执（施工 O-25 上，`onebot.md` 第一条「斜杠命令」第 7 条；18 第十节「命令回执带着 3 秒后撤回入队」）：斜杠命令的回执、
//! 被拒的那一句和别的一样入队（施工 O-25 中，`sending`）；群里的 NapCat 回了编号的，等 `bridge.json` 的
//! `receipt_recall_seconds` 以后经收进这个群的那个机器人号那时的连接 `delete_msg` 撤回，群里不留机器人的内部状态。私聊的不撤；
//! 发命令的那条消息不撤。
//!
//! 等几秒、撤，是一个另起的任务（[`Route`] 的 `chores`）：等 NapCat 回应在照先后交回的那一串（`sending`）里，回了编号才另起
//! 这个任务；等几秒不放进那一串，不然她后面的话记 `venue.delivered` 要跟着等几秒（「施工时定的」第 114、119 条）。回执是核心
//! 写的，不是她的话，不过出站链。

use std::sync::Arc;
use std::time::Duration;

use super::sending::{Piece, What};
use super::{Peer, Route};
use crate::TARGET;
use crate::core::Gone;
use crate::listen::bots::Bots;
use crate::onebot::{Lead, delete_msg};

impl Route {
    /// 把斜杠命令的回执、被拒的那一句 `said` 发回会话 `session` 的那个私聊、群（「斜杠命令」第 2、4、7 条）：去掉首尾空白，空的
    /// 不发；入队。
    ///
    /// # Errors
    ///
    /// 入队时写不出去、等的时候核心断开。
    pub(super) async fn receipt(&mut self, session: &str, said: &str) -> Result<(), Gone> {
        let said = said.trim();
        if said.is_empty() {
            return Ok(());
        }
        let piece = Piece {
            what: What::Receipt,
            text: said.to_string(),
            lead: Lead::default(),
        };
        self.enqueue(session, None, piece).await
    }
}

/// 群里 `peer` 那个群的回执 NapCat 回了编号 `message`：等 `after` 以后经 `bots` 里那个机器人号那时的连接撤回。那时没连着、撤
/// 不成的记一行。
pub(super) async fn recall(message: i64, peer: Peer, bots: Arc<Bots>, after: Duration) {
    let venue = &peer.venue;
    tokio::time::sleep(after).await;
    let Some(link) = bots.get(peer.bot) else {
        tracing::warn!(target: TARGET, venue = %venue, message, bot = peer.bot, "receipt not recalled, bot not connected");
        return;
    };
    let (action, params) = delete_msg(message);
    match link.calls.call(&link.out, action, params).await {
        Ok(_) => tracing::info!(target: TARGET, venue = %venue, message, "receipt recalled"),
        Err(error) => {
            tracing::warn!(target: TARGET, venue = %venue, message, error = ?error, "receipt not recalled");
        }
    }
}

#[cfg(test)]
mod tests;
