//! 她的话发回去（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 1、9 条；O-25 上接上出站链，私聊的「怎么走」第 10 条也在
//! 这里）：群会话推来的事件（订阅时从头补来的、之后推来的）走同一条路收进这个群的投影（`projection`）；她新说的话（序号大于
//! 订阅时的 `upto`）先过出站链（`outbound`），丢了的记一行，过了的照纯文本拆段，一段一条入队（`sending`，O-25 中：先记
//! `ext.onebot.venues.queued` 再照先后交 NapCat，群里的成了记 `venue.delivered`），第一段带引用和 @。私聊里她的话也过出站链、
//! 拆段、入队，不记 `venue.delivered`。

use miyu_chat::{OutCtx, Venue};
use miyu_kernel::event::Event;
use miyu_kernel::id::VenueId;
use serde_json::Value;

use super::outbound::{Passed, group_ctx, pass, private_ctx, why_name};
use super::projection::{Aim, Speaking};
use super::sending::{Piece, What};
use super::{Route, applied, reply_text};
use crate::TARGET;
use crate::core::Gone;

impl Route {
    /// 群会话 `session` 推来的一条 `pushed`（`event` 推送）：落了盘的收进投影（瞬时的没有序号，不看），她新说的话发回群里。
    ///
    /// # Errors
    ///
    /// 她的话入队时写不出去、等的时候核心断开（施工 O-25 中）。
    pub(super) async fn heard(&mut self, session: &str, pushed: &Value) -> Result<(), Gone> {
        let raw = &pushed["params"]["event"];
        if raw["seq"].as_u64().is_none() {
            return Ok(());
        }
        let event: Event = match serde_json::from_value(raw.clone()) {
            Ok(event) => event,
            Err(error) => {
                tracing::warn!(target: TARGET, session, error = %error, "event not understood");
                return Ok(());
            }
        };
        let Some(group) = self.groups.get_mut(session) else {
            return Ok(());
        };
        match group.take(&event) {
            Some(speaking) => self.speak(session, speaking, &reply_text(raw)).await,
            None => Ok(()),
        }
    }

    /// 把会话 `session` 留着没办的推送照先后收进投影（第 3 条：判以前；第 1 条：订阅以后）。
    ///
    /// # Errors
    ///
    /// 同 [`Route::heard`]。
    pub(super) async fn catch_up(&mut self, session: &str) -> Result<(), Gone> {
        for pushed in self.core.take_events(session) {
            self.heard(session, &pushed).await?;
        }
        Ok(())
    }

    /// 她在群会话 `session` 里新说的一段话 `text`（第 9 条）：过出站链（情形照投影交出的 `speaking`、本机此刻、这个群此刻的
    /// 参数），过了的一段一条入队。本机的钟读不出的（照说不会）记一行、不发。
    async fn speak(&mut self, session: &str, speaking: Speaking, text: &str) -> Result<(), Gone> {
        let Some(peer) = self.peers.get(session).cloned() else {
            return Ok(());
        };
        let Some(venue) = Venue::parse(&peer.venue) else {
            return Ok(());
        };
        let Some(clock) = applied::clock() else {
            tracing::warn!(target: TARGET, venue = %peer.venue, "clock not readable, reply dropped");
            return Ok(());
        };
        let params = self.applied(&venue).params;
        let ctx = group_ctx(&speaking, clock.now, params.outbound);
        let aim = speaking.aim.as_ref();
        let Some(passed) = passed(&peer.venue, text, &ctx, aim, params.split_chars) else {
            return Ok(());
        };
        let what = What::Reply {
            turn: speaking.turn,
            to: speaking.to,
        };
        self.say(session, passed, what).await
    }

    /// 私聊会话 `session` 推来的、她的一条回话 `event`（「怎么走」第 10 条）：过出站链（两样都是假，这一轮发出去的照桥入队时
    /// 自己记的 `Spoken`），过了的一段一条入队。
    ///
    /// # Errors
    ///
    /// 同 [`Route::heard`]。
    pub(super) async fn say_privately(&mut self, session: &str, event: &Value) -> Result<(), Gone> {
        let Some(peer) = self.peers.get(session).cloned() else {
            return Ok(());
        };
        let Some(venue) = Venue::parse(&peer.venue) else {
            return Ok(());
        };
        let turn = event["turn"].as_u64().unwrap_or(0);
        let params = self.applied(&venue).params;
        let spoken = self.spoken.get(session).map(|spoken| spoken.of(turn));
        let ctx = private_ctx(spoken.unwrap_or_default(), params.outbound);
        let text = reply_text(event);
        let Some(passed) = passed(&peer.venue, &text, &ctx, None, params.split_chars) else {
            return Ok(());
        };
        let what = What::Reply {
            turn,
            to: Vec::new(),
        };
        self.say(session, passed, what).await
    }

    /// 把过了链的 `passed` 一段一条照先后入队（施工 O-25 中，`sending`），第一段带上引用和 @。
    async fn say(&mut self, session: &str, passed: Passed, what: What) -> Result<(), Gone> {
        let mut lead = passed.lead;
        for text in passed.pieces {
            let piece = Piece {
                what: what.clone(),
                text,
                lead: std::mem::take(&mut lead),
            };
            self.enqueue(session, None, piece).await?;
        }
        Ok(())
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
