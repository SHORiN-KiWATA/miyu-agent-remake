//! 找会话（`onebot.md` 第一条「怎么走」第 7、9 条，「群消息」第 3 条；施工 O-22 从 `route.rs` 挪出来，群也走这里）：场所
//! → 会话编号只记在内存里，每个场所桥起来以后第一次要用时问一次 `venue.session`；会话不在了忘掉、再问一次。
//!
//! - 私聊：`venue.session {venue, kind: "private", peer}`；不是主人（`no_system_account`），或者回应的属主是桥自己（核心
//!   O-4 中以后陌生人的会话归系统账号，「施工时定的」第 49 条）：不接，会话编号不记、不订阅。问到了订阅（第 9 条）。
//! - 群：`venue.session {venue, kind: "group", persona?, preset?, cwd?}`（照场所规则，`applied`）；群的会话本来就归桥自己的
//!   系统账号，不照属主认陌生人。规则写了不存在的人格、预设（`unknown_persona`、`unknown_preset`、`preset_invalid`）：不接。
//!   问到了从头订阅（`after: 0`，施工 O-23，「群里怎么叫她」第 1 条）：补来的收进这个群的投影；会话不在了连投影一起忘掉。
//!   订阅回应的 `persona`（这个群会话用的人格）交给判官那边记下（施工 O-23 补）。
//! - 不接的同一个场所只记一行运行日志（桥起来以后；「施工时定的」第 66 条）。

use miyu_chat::Venue;
use miyu_kernel::id::ExternalId;
use serde_json::{Value, json};

use super::projection::Projection;
use super::{Message, Peer, Route};
use crate::TARGET;
use crate::core::{Gone, reason};
use crate::onebot::To;
use crate::rules::Applied;

/// 发消息的方法：只有它带场所的格（`command.run` 不收这一格）。
pub(super) const SEND: &str = "session.send";

/// 一个场所：找会话问什么、回执发到哪。
pub(super) struct Place {
    /// `venue.session` 的参数。
    opening: Value,
    /// 回执、回话发到哪；场所编号在这里。
    pub(super) peer: Peer,
}

impl Place {
    /// 机器人号 `bot` 收进来的、和号 `user`（平台上是 `external`）的私聊 `venue`（第 7 条）。
    pub(super) fn private(venue: &Venue, external: &ExternalId, bot: i64, user: i64) -> Place {
        Place {
            opening: json!({"venue": venue.id(), "kind": "private", "peer": external}),
            peer: Peer {
                bot,
                to: To::Private(user),
                venue: venue.id().clone(),
            },
        }
    }

    /// 机器人号 `bot` 收进来的、群号是 `group` 的群 `venue`，场所规则照 `applied`（「群消息」第 3 条）。
    pub(super) fn group(venue: &Venue, applied: &Applied, bot: i64, group: i64) -> Place {
        let mut opening = json!({"venue": venue.id(), "kind": "group"});
        for (key, value) in super::applied::opening(applied) {
            opening[key] = json!(value);
        }
        Place {
            opening,
            peer: Peer {
                bot,
                to: To::Group(group),
                venue: venue.id().clone(),
            },
        }
    }

    /// 是不是私聊。
    fn private_chat(&self) -> bool {
        matches!(self.peer.to, To::Private(_))
    }
}

impl Route {
    /// 把 `message` 照 `method`（[`SEND`]、`command.run`）交给它那个场所的会话：`{session, text, as}`，[`SEND`] 另带
    /// `venue`，编号是这条消息的命令编号（第 8 条）。交回会话编号和最后一次的回应（接受的、拒绝的都原样），这时记下这个
    /// 会话的回执、回话发给谁（「施工时定的」第 36 条）；不接的、找不到会话的是空的（已经记了运行日志）。
    pub(super) async fn deliver(
        &mut self,
        message: &Message,
        method: &str,
    ) -> Result<Option<(String, Value)>, Gone> {
        let mut params = json!({"text": message.text, "as": message.acting});
        if method == SEND {
            params["venue"] = message.fields.clone();
        }
        let found = self
            .on_session(&message.place, Some(&message.id), method, params)
            .await?;
        if let Some((session, _)) = &found {
            self.peers
                .insert(session.clone(), message.place.peer.clone());
        }
        Ok(found)
    }

    /// 在场所 `place` 的会话上调 `method`：`params` 补上 `session`，命令编号是 `id`（空的自己编）。会话不在了
    /// （`session_not_found`、`session_stopped`）忘掉、再找、再调，只重来一次（「施工时定的」第 8 条）。交回会话编号和最后
    /// 一次的回应；不接的、找不到会话的是空的。
    pub(super) async fn on_session(
        &mut self,
        place: &Place,
        id: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Option<(String, Value)>, Gone> {
        let venue = place.peer.venue.as_str();
        for retried in [false, true] {
            let session = match self.venues.get(venue) {
                Some(session) => session.clone(),
                None => match self.find(place).await? {
                    Some(session) => session,
                    None => return Ok(None),
                },
            };
            let mut params = params.clone();
            params["session"] = json!(session);
            let reply = match id {
                Some(id) => self.core.call_as(id, method, params).await?,
                None => self.core.call(method, params).await?,
            };
            if !retried
                && matches!(
                    reason(&reply),
                    Some("session_not_found" | "session_stopped")
                )
            {
                self.venues.remove(venue);
                self.groups.remove(&session);
                continue;
            }
            return Ok(Some((session, reply)));
        }
        Ok(None)
    }

    /// 找回场所 `place` 的会话，私聊的再订阅它，群的从头订阅（第 7、9 条，「群消息」第 3 条，「群里怎么叫她」第 1 条）。不接的、
    /// 问不到的、订阅不上的是空的（记一行运行日志）。
    async fn find(&mut self, place: &Place) -> Result<Option<String>, Gone> {
        let venue = &place.peer.venue;
        let reply = self
            .core
            .call("venue.session", place.opening.clone())
            .await?;
        // 私聊的回应带了会话的属主、正是桥自己的账号：陌生人（核心 O-4 中以后照常造会话，属主是系统账号）。私聊只接主人，
        // 照 `no_system_account` 办（「施工时定的」第 49 条）。没带属主的照常接。群的会话本来就归桥自己的账号。
        let own = place.private_chat()
            && reply["result"]["account"]
                .as_str()
                .is_some_and(|owner| Some(owner) == self.core.account.as_deref());
        match reason(&reply) {
            None if !own => {}
            None | Some("no_system_account") => {
                if self.refused.insert(venue.to_string()) {
                    tracing::info!(target: TARGET, venue = %venue, "not the owner, not taken");
                }
                return Ok(None);
            }
            Some(reason @ ("unknown_persona" | "unknown_preset" | "preset_invalid")) => {
                if self.refused.insert(venue.to_string()) {
                    tracing::warn!(target: TARGET, venue = %venue, reason, "group not recorded, fix the venue rules");
                }
                return Ok(None);
            }
            Some(other) => {
                tracing::warn!(target: TARGET, venue = %venue, reason = other, "venue session not found");
                return Ok(None);
            }
        }
        let Some(session) = reply["result"]["session"].as_str().map(str::to_string) else {
            tracing::warn!(target: TARGET, venue = %venue, "venue session without an id");
            return Ok(None);
        };
        let subscribed = if place.private_chat() {
            self.subscribe(&session).await?
        } else {
            // 补来的有她正在说的话的（订阅时有一轮在跑），照这里发回群里。
            self.peers.insert(session.clone(), place.peer.clone());
            self.follow(&session, 0).await?
        };
        if !subscribed {
            return Ok(None);
        }
        self.venues.insert(venue.to_string(), session.clone());
        Ok(Some(session))
    }

    /// 订阅群会话 `session` 的事件流，补 `after` 以后的（施工 O-23，「群里怎么叫她」第 1 条）：头一次是 0，从头补；掉了队的是
    /// 收到的最后一条。补来的照先后收进这个群的投影，序号不大于回应的 `upto` 的她的话不发。订阅不上的记一行、交回假，这个群的
    /// 会话和投影都忘掉：下一条照第 7 条再找、从头订阅（掉了队再订阅不上的，投影不再跟着日志走，留着会判错）。订阅上了的，
    /// 回应的 `persona`（这个群会话用的人格，没有的是无人格）交给判官那边记下（施工 O-23 补）。
    pub(super) async fn follow(&mut self, session: &str, after: u64) -> Result<bool, Gone> {
        let params = json!({"session": session, "stream": "events", "after": after});
        let reply = self.core.call("subscribe", params).await?;
        if let Some(reason) = reason(&reply) {
            tracing::warn!(target: TARGET, session, reason, "not subscribed");
            self.groups.remove(session);
            self.venues.retain(|_, found| found != session);
            return Ok(false);
        }
        let upto = reply["result"]["upto"].as_u64().unwrap_or(after);
        self.judges
            .subscribed(session, reply["result"]["persona"].as_str());
        self.groups
            .entry(session.to_string())
            .or_insert_with(|| Projection::new(upto))
            .caught_up(upto);
        self.catch_up(session).await;
        Ok(true)
    }

    /// 订阅会话 `session` 的事件流，不写 `after`（第 9 条）：只要以后的新事件。订阅不上的记一行、交回假。
    pub(super) async fn subscribe(&mut self, session: &str) -> Result<bool, Gone> {
        let params = json!({"session": session, "stream": "events"});
        let reply = self.core.call("subscribe", params).await?;
        if let Some(reason) = reason(&reply) {
            tracing::warn!(target: TARGET, session, reason, "not subscribed");
            return Ok(false);
        }
        Ok(true)
    }
}
