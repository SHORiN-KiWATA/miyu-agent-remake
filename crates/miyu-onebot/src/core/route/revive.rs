//! 起来就订阅（施工 O-32，`onebot.md` 第一条「群里怎么叫她」第 1 条、「出站队列」第 6 条）：桥每次连上核心、握手以后，先经核心的
//! `venue.sessions` 列出名下的场所会话和终端管理员的私聊（`venues.md`「列场所会话」），群的照「群里怎么叫她」第 1 条从头订阅；
//! 私聊的照私聊的办法（「怎么走」第 7 条：白名单成员的、终端管理员的接，陌生人的不接），从头订阅一次。不用等每个场所来一条消息才找：重启以后她接着说的照样发。
//!
//! 订阅补来的那一段（`backlog`）补完了：认出发到哪（这时还不知道的，照日志），期限以内、这一回合没入队的她的话照常过出站链、
//! 入队、发（补发）；入队过的、过了期限的不补。

use std::time::Instant;

use miyu_chat::Venue;
use miyu_kernel::id::{ExternalId, VenueId};
use serde_json::json;

use super::backlog::Backlog;
use super::outbound::{group_ctx, pass, private_ctx};
use super::projection::Speaking;
use super::sending::What;
use super::{Peer, Route, applied};
use crate::TARGET;
use crate::core::{Gone, reason};
use crate::onebot::{Lead, To, person, to_of};

impl Route {
    /// 起来时订阅名下的场所会话（见模块开头）。核心拒了的（进程里的测试不是系统账号，回 `no_system_account`）记一行，照旧等
    /// 每个场所来一条消息再找。认不出的场所（别的平台、写法不对）、不接的私聊（[`Route::taken`]）不订阅。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(super) async fn revive(&mut self) -> Result<(), Gone> {
        let reply = self.core.call("venue.sessions", json!({})).await?;
        if let Some(reason) = reason(&reply) {
            tracing::warn!(target: TARGET, reason, "venue sessions not listed");
            return Ok(());
        }
        let listed = reply["result"]["sessions"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut count = 0_usize;
        for one in &listed {
            let (Some(session), Some(venue)) = (one["session"].as_str(), one["venue"].as_str())
            else {
                continue;
            };
            let Some(venue) = VenueId::parse(venue).ok().and_then(|id| Venue::parse(&id)) else {
                continue;
            };
            let followed = match to_of(&venue) {
                Some(To::Group(_)) => {
                    let followed = self.follow(session, 0).await?;
                    if followed {
                        self.venues
                            .insert(venue.id().to_string(), session.to_string());
                    }
                    followed
                }
                Some(To::Private(user)) => match person(user) {
                    Ok(peer) if self.taken(&peer).await? => {
                        self.subscribe(session, Some(0)).await?
                    }
                    _ => false,
                },
                None => false,
            };
            count += usize::from(followed);
        }
        tracing::info!(target: TARGET, count, "venue sessions followed");
        Ok(())
    }

    /// 列出来的私聊，对方是 `peer`：照私聊的办法接不接（「怎么走」第 7 条）。白名单成员的接；终端管理员的接（核心列出来的他在这个
    /// 平台的私聊）：问核心的 `venue.binding`，问到的记下（同「平台工具（一）」第 3 条）；别的（陌生人、从白名单里删了的）不接。
    /// 核心拒了的（照说不会：桥是系统账号）记一行，不接。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    async fn taken(&mut self, peer: &ExternalId) -> Result<bool, Gone> {
        if self.whitelisted(peer) {
            return Ok(true);
        }
        if let Some(admin) = self.bindings.known(peer.as_str(), Instant::now()) {
            return Ok(admin);
        }
        let reply = self
            .core
            .call("venue.binding", json!({"id": peer.as_str()}))
            .await?;
        if let Some(reason) = reason(&reply) {
            tracing::warn!(target: TARGET, id = peer.as_str(), reason, "binding not asked");
            return Ok(false);
        }
        let admin = reply["result"]["account"].is_string();
        self.bindings.note(peer.as_str(), admin, Instant::now());
        Ok(admin)
    }

    /// 会话 `session` 订阅上了、补到 `upto`：留着的补来的推送照先后收下（群的进投影，`heard`；私聊的 `say_privately`），再补发。
    /// 本机的钟读不出的（照说不会）什么都不补。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(super) async fn replay(&mut self, session: &str, upto: u64) -> Result<(), Gone> {
        let since = applied::clock().map_or(i64::MAX, |clock| self.waiting.since(clock.now));
        self.backlogs
            .insert(session.to_string(), Backlog::new(upto, since));
        self.gather(session).await?;
        match self.backlogs.remove(session) {
            Some(backlog) => self.resend(session, backlog).await,
            None => Ok(()),
        }
    }

    /// 补发会话 `session` 补来的、期限以内的她的话（见模块开头）：还不知道发到哪的照日志认，认不出的记一行、不补（等那个场所来一条
    /// 消息）。出站链丢了的不再记一行：那一句当时照常过过链（入队过的照这一回合去重丢掉）。拆出来的第一段入队过的，引用、@
    /// 跟着它当时发过了，这一次不带。
    async fn resend(&mut self, session: &str, mut backlog: Backlog) -> Result<(), Gone> {
        if !self.peers.contains_key(session) {
            let Some((bot, to, venue)) = backlog.peer() else {
                tracing::info!(target: TARGET, session, "bot not known yet");
                return Ok(());
            };
            self.peers
                .insert(session.to_string(), Peer { bot, to, venue });
        }
        let Some(peer) = self.peers.get(session).cloned() else {
            return Ok(());
        };
        let (Some(venue), Some(clock)) = (Venue::parse(&peer.venue), applied::clock()) else {
            return Ok(());
        };
        let params = self.applied(&venue).params;
        for said in backlog.take_said() {
            let sent = backlog.sent(said.turn);
            let (ctx, aim, to) = match said.speaking {
                Some(speaking) => {
                    let speaking = Speaking { sent, ..speaking };
                    let ctx = group_ctx(&speaking, clock.now, params.outbound.clone());
                    (ctx, speaking.aim, speaking.to)
                }
                None => (private_ctx(sent, params.outbound.clone()), None, Vec::new()),
            };
            let Ok(mut passed) = pass(&said.text, &ctx, aim.as_ref(), params.split_chars) else {
                continue;
            };
            let first = passed.pieces.first().cloned();
            passed.pieces = backlog.unsent(said.turn, passed.pieces);
            if passed.pieces.is_empty() {
                continue;
            }
            if passed.pieces.first() != first.as_ref() {
                passed.lead = Lead::default();
            }
            let chars = said.text.chars().count();
            tracing::info!(target: TARGET, venue = %peer.venue, chars, "reply resent");
            let what = What::Reply {
                turn: said.turn,
                to,
            };
            self.say(session, passed, what).await?;
        }
        Ok(())
    }
}
