//! 群会话推来的事件（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 1、9 条）：订阅时从头补来的、之后推来的走同一条路，
//! 收进这个群的投影（`projection`）；她新说的话（序号大于订阅时的 `upto`）照群聊内核的 `plain` 转成纯文本、`split` 照这个群的
//! 参数拆开，一段一条 `send_group_msg`，NapCat 回了成功的记 `venue.delivered`。
//!
//! 发的一步照私聊的办法（第 10 条，「施工时定的」第 10 条）：这里照先后放进写队列，等 NapCat 回应交给别的任务；任务交回
//! 发出去了的那一段（[`Delivered`]），照放进写队列的先后交回来（「施工时定的」第 84 条），由拿着跟核心的连接的这一头记
//! `venue.delivered`。引用和 @、去重、清理、出站队列随 O-25。

use miyu_chat::{Venue, plain, split};
use miyu_kernel::event::Event;
use miyu_kernel::id::{ExternalId, VenueId};
use serde_json::{Value, json};

use super::projection::Speaking;
use super::{Route, reply_text};
use crate::TARGET;
use crate::core::Gone;
use crate::onebot::{Pending, message_to, number};

/// 记成的事件（`venues.md`「桥记的事件」第 3 条）。
const DELIVERED: &str = "venue.delivered";

/// 发出去了的一段：NapCat 回了成功，记 `venue.delivered` 要的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Delivered {
    /// 哪个群的会话（主线）：`line` 也是它。
    session: String,
    /// 回合编号。
    turn: u64,
    /// 这一轮回的人。
    to: Vec<ExternalId>,
    /// NapCat 回的 `message_id`，写成十进制的字。
    msg: String,
    /// 这一段。
    text: String,
}

impl Route {
    /// 群会话 `session` 推来的一条 `pushed`（`event` 推送）：落了盘的收进投影（瞬时的没有序号，不看），她新说的话发回群里。
    pub(super) async fn heard(&mut self, session: &str, pushed: &Value) {
        let raw = &pushed["params"]["event"];
        if raw["seq"].as_u64().is_none() {
            return;
        }
        let event: Event = match serde_json::from_value(raw.clone()) {
            Ok(event) => event,
            Err(error) => {
                tracing::warn!(target: TARGET, session, error = %error, "event not understood");
                return;
            }
        };
        let Some(group) = self.groups.get_mut(session) else {
            return;
        };
        if let Some(speaking) = group.take(&event) {
            self.speak(session, speaking, &reply_text(raw)).await;
        }
    }

    /// 把会话 `session` 留着没办的推送照先后收进投影（第 3 条：判以前；第 1 条：订阅以后）。
    pub(super) async fn catch_up(&mut self, session: &str) {
        for pushed in self.core.take_events(session) {
            self.heard(session, &pushed).await;
        }
    }

    /// 她在群会话 `session` 里新说的一段话 `text`：转成纯文本、照这个群的 `split_chars` 拆开，一段一条照先后放进写队列；拆出来
    /// 是空的不发，收进这个群的那个机器人号没连着的记一行、丢掉（第 9 条）。
    async fn speak(&mut self, session: &str, speaking: Speaking, text: &str) {
        let Some(peer) = self.peers.get(session).cloned() else {
            return;
        };
        let split_chars = match Venue::parse(&peer.venue) {
            Some(venue) => self.applied(&venue).params.split_chars,
            None => return,
        };
        let pieces = split(&plain(text), split_chars);
        if pieces.is_empty() {
            return;
        }
        let Some(link) = self.bots.get(peer.bot) else {
            let chars: usize = pieces.iter().map(|piece| piece.chars().count()).sum();
            tracing::info!(target: TARGET, venue = %peer.venue, bot = peer.bot, chars, "bot not connected, reply dropped");
            return;
        };
        for piece in pieces {
            let (action, params) = message_to(peer.to, &piece);
            let pending = match link.calls.begin(&link.out, action, params).await {
                Ok(pending) => pending,
                Err(error) => {
                    tracing::warn!(target: TARGET, venue = %peer.venue, error = ?error, "reply not sent");
                    return;
                }
            };
            let delivered = Delivered {
                session: session.to_string(),
                turn: speaking.turn,
                to: speaking.to.clone(),
                msg: String::new(),
                text: piece,
            };
            let venue = peer.venue.clone();
            self.sending
                .push_back(tokio::spawn(wait(pending, delivered, venue)));
        }
    }

    /// 发出去了的一段记 `venue.delivered`（第 9 条）：`line` 是这个群的会话（主线），命令编号自己编。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(super) async fn delivered(&mut self, delivered: Delivered) -> Result<(), Gone> {
        let body = json!({
            "line": delivered.session,
            "turn": delivered.turn,
            "to": delivered.to,
            "msg": delivered.msg,
            "text": delivered.text,
            "images": [],
        });
        self.append(&delivered.session, None, DELIVERED, body).await
    }
}

/// 等 NapCat 回应发出去的一段：成了、回了 `message_id` 的交回它（[`Delivered::msg`] 填上）；没成、没回编号的记一行，交回空的。
async fn wait(pending: Pending, mut delivered: Delivered, venue: VenueId) -> Option<Delivered> {
    let chars = delivered.text.chars().count();
    let reply = match pending.wait().await {
        Ok(reply) => reply,
        Err(error) => {
            tracing::warn!(target: TARGET, venue = %venue, chars, error = ?error, "reply not sent");
            return None;
        }
    };
    tracing::info!(target: TARGET, venue = %venue, chars, "reply sent");
    let Some(msg) = number(&reply["data"]["message_id"]) else {
        tracing::warn!(target: TARGET, venue = %venue, chars, "delivered without a message id");
        return None;
    };
    delivered.msg = msg.to_string();
    Some(delivered)
}
