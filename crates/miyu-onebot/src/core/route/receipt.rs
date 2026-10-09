//! 命令回执（施工 O-25 上，`onebot.md` 第一条「斜杠命令」第 7 条；18 第十节「命令回执带着 3 秒后撤回入队」）：斜杠命令的回执、
//! 被拒的那一句，私聊的照发回话的办法发、不撤；群里的发出去，NapCat 回了编号的，等 `bridge.json` 的
//! `receipt_recall_seconds` 以后经收进这个群的那个机器人号那时的连接 `delete_msg` 撤回，群里不留机器人的内部状态。发命令的
//! 那条消息不撤。
//!
//! 等回应、等几秒、撤，是一个另起的任务（[`Route`] 的 `recalls`）：不放进照先后交回的那一串（`sending`），不然她后面的话记
//! `venue.delivered` 要跟着等几秒。回执是核心写的，不是她的话，不过出站链。

use std::sync::Arc;
use std::time::Duration;

use super::{Peer, Route};
use crate::TARGET;
use crate::listen::bots::Bots;
use crate::onebot::{Lead, Pending, To, delete_msg, number};

impl Route {
    /// 把斜杠命令的回执、被拒的那一句 `said` 发回会话 `session` 的那个私聊、群（「斜杠命令」第 2、4、7 条）：私聊的照
    /// `send_back`；群里的去掉首尾空白、空的不发，放进写队列，交给撤回的任务。
    pub(super) async fn receipt(&mut self, session: &str, said: &str) {
        let Some(peer) = self.peers.get(session).cloned() else {
            return;
        };
        if matches!(peer.to, To::Private(_)) {
            self.send_back(session, said).await;
            return;
        }
        let said = said.trim();
        if said.is_empty() {
            return;
        }
        let Some(pending) = self.begin(&peer, said, &Lead::default()).await else {
            return;
        };
        let chars = said.chars().count();
        let bots = Arc::clone(&self.bots);
        self.recalls
            .spawn(recall(pending, peer, bots, self.recall, chars));
    }
}

/// 等 NapCat 回应发进 `peer` 那个群的回执（`chars` 个字符，只记运行日志）；回了编号的，等 `after` 以后经 `bots` 里那个机器人号
/// 那时的连接撤回。发不出去、没回编号、那时没连着、撤不成的记一行。
async fn recall(pending: Pending, peer: Peer, bots: Arc<Bots>, after: Duration, chars: usize) {
    let venue = &peer.venue;
    let reply = match pending.wait().await {
        Ok(reply) => reply,
        Err(error) => {
            tracing::warn!(target: TARGET, venue = %venue, chars, error = ?error, "reply not sent");
            return;
        }
    };
    tracing::info!(target: TARGET, venue = %venue, chars, "reply sent");
    let Some(message) = number(&reply["data"]["message_id"]) else {
        tracing::warn!(target: TARGET, venue = %venue, "receipt not recalled, no message id");
        return;
    };
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
