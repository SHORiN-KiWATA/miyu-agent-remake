//! 她的话发回去（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 1、9 条；O-25 上接上出站链，私聊的「怎么走」第 10 条也在
//! 这里）：群会话推来的事件（订阅时从头补来的、之后推来的）走同一条路收进这个群的投影（`projection`）；她新说的话（序号大于
//! 订阅时的 `upto`）先过出站链（`outbound`），丢了的记一行，过了的照纯文本拆段，一段一条 `send_group_msg`，第一段带引用和 @，
//! NapCat 回了成功的记 `venue.delivered`。私聊里她的话也过出站链、拆段，`send_private_msg` 发回去，不记 `venue.delivered`。
//!
//! 发的一步照私聊的办法（「怎么走」第 10 条，「施工时定的」第 10 条）：这里照先后放进写队列，等 NapCat 回应交给别的任务；
//! 任务交回发出去了的那一段（[`Delivered`]），照放进写队列的先后交回来（「施工时定的」第 84 条），由拿着跟核心的连接的这一头
//! 记 `venue.delivered`。出站队列随 O-25 下。

use miyu_chat::{OutCtx, Venue};
use miyu_kernel::event::Event;
use miyu_kernel::id::{ExternalId, VenueId};
use serde_json::{Value, json};

use super::outbound::{Passed, group_ctx, pass, private_ctx, why_name};
use super::projection::{Aim, Speaking};
use super::{Peer, Route, applied, reply_text};
use crate::TARGET;
use crate::core::Gone;
use crate::onebot::{Pending, number};

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
    /// 这一段（不带引用、@）。
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

    /// 她在群会话 `session` 里新说的一段话 `text`（第 9 条）：过出站链（情形照投影交出的 `speaking`、本机此刻、这个群此刻的
    /// 参数），过了的照先后放进写队列，发出去了的记 `venue.delivered`。本机的钟读不出的（照说不会）记一行、不发。
    async fn speak(&mut self, session: &str, speaking: Speaking, text: &str) {
        let Some(peer) = self.peers.get(session).cloned() else {
            return;
        };
        let Some(venue) = Venue::parse(&peer.venue) else {
            return;
        };
        let Some(clock) = applied::clock() else {
            tracing::warn!(target: TARGET, venue = %peer.venue, "clock not readable, reply dropped");
            return;
        };
        let params = self.applied(&venue).params;
        let ctx = group_ctx(&speaking, clock.now, params.outbound);
        let aim = speaking.aim.as_ref();
        let Some(passed) = passed(&peer.venue, text, &ctx, aim, params.split_chars) else {
            return;
        };
        let delivered = Delivered {
            session: session.to_string(),
            turn: speaking.turn,
            to: speaking.to,
            msg: String::new(),
            text: String::new(),
        };
        self.send_pieces(&peer, passed, Some(delivered)).await;
    }

    /// 私聊会话 `session` 推来的、她的一条回话 `event`（「怎么走」第 10 条）：过出站链（两样都是假，这一轮发出去的照桥自己
    /// 记的），过了的照先后放进写队列。
    pub(super) async fn say_privately(&mut self, session: &str, event: &Value) {
        let Some(peer) = self.peers.get(session).cloned() else {
            return;
        };
        let Some(venue) = Venue::parse(&peer.venue) else {
            return;
        };
        let turn = event["turn"].as_u64().unwrap_or(0);
        let params = self.applied(&venue).params;
        let spoken = self.spoken.entry(session.to_string()).or_default();
        let ctx = private_ctx(spoken.of(turn), params.outbound);
        let text = reply_text(event);
        let Some(passed) = passed(&peer.venue, &text, &ctx, None, params.split_chars) else {
            return;
        };
        spoken.add(turn, passed.text.clone());
        self.send_pieces(&peer, passed, None).await;
    }

    /// 把过了链的 `passed` 一段一条照先后放进 `peer` 那个机器人号现在的连接的写队列，第一段带上引用和 @；等回应交给别的任务。
    /// 群里的照 `delivered` 填好每一段，发出去了交回来记；私聊的是空的。放不进去的（没连着、连接断了）记一行，后面的不再发。
    async fn send_pieces(&mut self, peer: &Peer, passed: Passed, delivered: Option<Delivered>) {
        let mut lead = passed.lead;
        for piece in passed.pieces {
            let Some(pending) = self.begin(peer, &piece, &std::mem::take(&mut lead)).await else {
                return;
            };
            let delivered = delivered.clone().map(|delivered| Delivered {
                text: piece.clone(),
                ..delivered
            });
            let chars = piece.chars().count();
            let venue = peer.venue.clone();
            self.sending
                .push_back(tokio::spawn(wait(pending, delivered, venue, chars)));
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

/// 她在场所 `venue` 的一句 `text` 过出站链（情形 `ctx`，她回的那一条 `aim`，一段最多 `split_chars` 个字符）：丢了的记一行
/// `reply dropped`（原因，不记原文）、交回空的（第 9 条第 2 项）。
fn passed(
    venue: &VenueId,
    text: &str,
    ctx: &OutCtx,
    aim: Option<&Aim>,
    split_chars: usize,
) -> Option<Passed> {
    match pass(text, ctx, aim, split_chars) {
        Ok(passed) => Some(passed),
        Err(why) => {
            let chars = text.chars().count();
            tracing::info!(target: TARGET, venue = %venue, why = why_name(why), chars, "reply dropped");
            None
        }
    }
}

/// 等 NapCat 回应发出去的一段（`chars` 个字符，只记运行日志）：群里的成了、回了 `message_id` 的交回 `delivered`（[`Delivered::msg`]
/// 填上）；没成、没回编号的记一行，私聊的，都交回空的。
async fn wait(
    pending: Pending,
    delivered: Option<Delivered>,
    venue: VenueId,
    chars: usize,
) -> Option<Delivered> {
    let reply = match pending.wait().await {
        Ok(reply) => reply,
        Err(error) => {
            tracing::warn!(target: TARGET, venue = %venue, chars, error = ?error, "reply not sent");
            return None;
        }
    };
    tracing::info!(target: TARGET, venue = %venue, chars, "reply sent");
    let mut delivered = delivered?;
    let Some(msg) = number(&reply["data"]["message_id"]) else {
        tracing::warn!(target: TARGET, venue = %venue, chars, "delivered without a message id");
        return None;
    };
    delivered.msg = msg.to_string();
    Some(delivered)
}
