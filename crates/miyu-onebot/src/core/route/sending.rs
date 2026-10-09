//! 出站队列的那一头（施工 O-25 中，`onebot.md` 第一条「出站队列」；18 第十节、Q15）：她要说出去的一切（群里她的话、限流的
//! 提示、命令回执，私聊的回话）先记 `ext.onebot.venues.queued`、拿到序号，再照先后交 NapCat；结局记下：群里她的话记
//! `venue.delivered`，群里的回执回了编号的交 `receipt` 撤，没成、过期的记 `ext.onebot.venues.failed`。
//!
//! - 门开着（这个群没被禁言，照投影；收进这个会话的那个机器人号连着）的照先后放进那个号现在的连接的写队列，前一段放进去了才
//!   放下一段；等回应交给别的任务，结局照放进写队列的先后交回来（`Route` 的 `sending`，「施工时定的」第 10、84 条）。
//! - 门关着的排着（纯逻辑在 `queue`）。什么时候再看一遍：入队以后、机器人号连上了、记了禁言或解禁以后（`muted`）、定时醒了
//!   （[`Route::wake`]，跟核心的那一头睡到那一刻）：先把过期的记了，再交门开着的（「施工时定的」第 123、125 条）。
//! - 她的话入队记成了就算进这一回合发出去的（去重照它）：群里先算进投影，私聊记进 `Spoken`（「施工时定的」第 112、113 条）。
//! - 退信（施工 O-25 下，「退信」）：她的话记了 `failed` 以后，经核心的 `session.note` 给那个会话记一块 `undelivered` 事实，写着
//!   为什么和那一段的开头（模板是出厂数据，`rules/facts.rs`）；一段一块，提示、回执不退（「施工时定的」第 129、130 条）。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::id::ExternalId;
use miyu_kernel::time::Timestamp;
use serde_json::{Value, json};

use super::projection::{NOTICE, QUEUED, REPLY};
use super::queue::{DISCONNECTED, EXPIRED, Ending, ending};
use super::receipt::recall;
use super::{Peer, Route, applied};
use crate::TARGET;
use crate::core::{Gone, reason};
use crate::onebot::{CallError, Lead, To, message_to};

/// 失败记成的事件（第 4 条）。
const FAILED: &str = "ext.onebot.venues.failed";

/// 发出去了记成的事件（`venues.md`「桥记的事件」第 3 条）。
const DELIVERED: &str = "venue.delivered";

/// `ext.onebot.venues.queued` 的 `kind`：命令回执、被拒的那一句。
const RECEIPT: &str = "receipt";

/// 门关着：她被禁言着。
const MUTED: &str = "muted";

/// 退信那一块事实的 `kind`（施工 O-25 下，「退信」第 2 条）。
const UNDELIVERED: &str = "undelivered";

/// 退信带她那一段的头几个字符（施工 O-25 下，「退信」第 3 条，「施工时定的」第 130 条）：认得出是哪一句就够。
const HEAD: usize = 30;

/// 要说出去的一段是什么（第 1 条）。
#[derive(Debug, Clone)]
pub(super) enum What {
    /// 她的话：回合编号、这一轮回的人（群里的成了记 `venue.delivered`）。
    Reply {
        /// 回合编号。
        turn: u64,
        /// 这一轮回的人；私聊的是空的。
        to: Vec<ExternalId>,
    },
    /// 提示：为什么（`rate_limited`）。
    Notice(&'static str),
    /// 命令回执、被拒的那一句：群里的回了编号的过几秒撤回。
    Receipt,
}

/// 要说出去的一段：是什么、正文（不带引用、@）、前面带的引用和 @。
#[derive(Debug, Clone)]
pub(super) struct Piece {
    /// 是什么。
    pub(super) what: What,
    /// 正文。
    pub(super) text: String,
    /// 前面带的引用和 @：只有她一句话的第一段带。
    pub(super) lead: Lead,
}

/// 排着的一段：入队那一条的序号和这一段。
#[derive(Debug)]
pub(super) struct Item {
    /// 入队那一条（`ext.onebot.venues.queued`）的序号：`failed` 的 `queued` 照它写。
    queued: u64,
    /// 这一段。
    piece: Piece,
}

/// 交出去的一段等来了回应：哪个会话、发到哪、这一段、调用的结果。
#[derive(Debug)]
pub(super) struct Answered {
    /// 会话编号。
    session: String,
    /// 发到哪：交出去那一刻的。
    peer: Peer,
    /// 这一段。
    item: Item,
    /// 调用的结果。
    result: Result<Value, CallError>,
}

impl Route {
    /// 会话 `session` 要说一段 `piece`（第 2 条）：先记 `ext.onebot.venues.queued`（命令编号是 `id`，空的自己编），记成了排进
    /// 队、算进这一回合发出去的（她的话），再看一遍排着的。去掉首尾空白是空的不入队（第 1 条）；不知道发到哪的、本机的钟读不出的
    /// （照说不会）、核心拒了的（照说不会，`append` 记了一行）不发。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(super) async fn enqueue(
        &mut self,
        session: &str,
        id: Option<&str>,
        piece: Piece,
    ) -> Result<(), Gone> {
        let Some(peer) = self.peers.get(session).cloned() else {
            return Ok(());
        };
        if piece.text.trim().is_empty() {
            return Ok(());
        }
        let chars = piece.text.chars().count();
        let Some(clock) = applied::clock() else {
            tracing::warn!(target: TARGET, venue = %peer.venue, chars, "clock not readable, reply dropped");
            return Ok(());
        };
        let body = queued_body(session, &piece);
        let Some(queued) = self.append(session, id, QUEUED, body).await? else {
            return Ok(());
        };
        if let What::Reply { turn, .. } = &piece.what {
            match (peer.to, self.groups.get_mut(session)) {
                (To::Group(_), Some(group)) => group.queued(*turn, &piece.text),
                (To::Group(_), None) => {}
                (To::Private(_), _) => self
                    .spoken
                    .entry(session.to_string())
                    .or_default()
                    .add(*turn, piece.text.clone()),
            }
        }
        if let Some(why) = self.shut(session, clock.now) {
            tracing::info!(target: TARGET, venue = %peer.venue, why, chars, "reply waiting");
        }
        self.waiting
            .push(session, clock.now, Item { queued, piece });
        self.pump().await
    }

    /// 把有东西排着的会话都看一遍（第 3 条）：过了期的记 `expired`，门开着的照先后交 NapCat。本机的钟读不出的（照说不会）记一行，
    /// 这一回不看。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(super) async fn pump(&mut self) -> Result<(), Gone> {
        let Some(clock) = applied::clock() else {
            tracing::warn!(target: TARGET, "clock not readable, queue not looked at");
            return Ok(());
        };
        for session in self.waiting.sessions() {
            let open = self.shut(&session, clock.now).is_none();
            let taken = self.waiting.take(&session, clock.now, open);
            for item in taken.expired {
                self.failed(&session, &item, (EXPIRED, None)).await?;
            }
            for item in taken.ready {
                self.hand(&session, item).await?;
            }
        }
        Ok(())
    }

    /// 离下一次该看一遍还有多久（第 8 条）：排着的里最早的过期时刻、被禁言着的会话禁言到期的时刻；没有排着的是空的，不用醒。
    pub(super) fn wake(&self) -> Option<Duration> {
        let now = applied::clock()?.now;
        let at = self
            .waiting
            .wake(|session| self.groups.get(session)?.muted(now))?;
        let millis = at.unix_millis().saturating_sub(now.unix_millis());
        Some(Duration::from_millis(u64::try_from(millis).unwrap_or(0)))
    }

    /// 会话 `session` 此刻（`now`）门关着的为什么：这个群被禁言着是 `muted`，收进它的那个机器人号没连着是 `disconnected`；
    /// 开着的是空的。
    fn shut(&self, session: &str, now: Timestamp) -> Option<&'static str> {
        if let Some(group) = self.groups.get(session)
            && group.muted(now).is_some()
        {
            return Some(MUTED);
        }
        let connected = self
            .peers
            .get(session)
            .is_some_and(|peer| self.bots.get(peer.bot).is_some());
        (!connected).then_some(DISCONNECTED)
    }

    /// 把排着的一段交 NapCat：放进收进会话 `session` 的那个机器人号现在的连接的写队列，等回应交给别的任务（第 3 条）。放不进去的
    /// （那个号这时没连着、连接正在断）当场照结局办。
    async fn hand(&mut self, session: &str, item: Item) -> Result<(), Gone> {
        let Some(peer) = self.peers.get(session).cloned() else {
            return self.failed(session, &item, (DISCONNECTED, None)).await;
        };
        let begun = match self.bots.get(peer.bot) {
            Some(link) => {
                let (action, params) = message_to(peer.to, &item.piece.text, &item.piece.lead);
                link.calls.begin(&link.out, action, params).await
            }
            None => Err(CallError::Closed),
        };
        let session = session.to_string();
        match begun {
            Ok(pending) => {
                self.sending.push_back(tokio::spawn(async move {
                    let result = pending.wait().await;
                    Answered {
                        session,
                        peer,
                        item,
                        result,
                    }
                }));
                Ok(())
            }
            Err(error) => {
                let result = Err(error);
                self.answered(Answered {
                    session,
                    peer,
                    item,
                    result,
                })
                .await
            }
        }
    }

    /// 交出去的一段有了结局（第 4 条）：没成的记 `failed`；成了的记一行，群里她的话记 `venue.delivered`（回了编号的），群里的回执
    /// 回了编号的过几秒撤回（「斜杠命令」第 7 条），别的不再记。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(super) async fn answered(&mut self, answered: Answered) -> Result<(), Gone> {
        let Answered {
            session,
            peer,
            item,
            result,
        } = answered;
        let msg = match ending(&result) {
            Ending::Sent(msg) => msg,
            Ending::Failed(why, detail) => {
                return self.failed(&session, &item, (why, detail)).await;
            }
        };
        let (venue, chars) = (&peer.venue, item.piece.text.chars().count());
        tracing::info!(target: TARGET, venue = %venue, chars, "reply sent");
        if !matches!(peer.to, To::Group(_)) {
            return Ok(());
        }
        match (item.piece.what, msg) {
            (What::Reply { turn, to }, Some(msg)) => {
                let body = json!({"line": session, "turn": turn, "to": to, "msg": msg.to_string(), "text": item.piece.text, "images": []});
                self.append(&session, None, DELIVERED, body).await.map(drop)
            }
            (What::Reply { .. }, None) => {
                tracing::warn!(target: TARGET, venue = %venue, chars, "delivered without a message id");
                Ok(())
            }
            (What::Receipt, Some(msg)) => {
                let bots = Arc::clone(&self.bots);
                self.chores.spawn(recall(msg, peer, bots, self.recall));
                Ok(())
            }
            (What::Receipt, None) => {
                tracing::warn!(target: TARGET, venue = %venue, "receipt not recalled, no message id");
                Ok(())
            }
            (What::Notice(_), _) => Ok(()),
        }
    }

    /// 没成的一段记 `failed {queued, why, detail?}`（第 4 条），运行日志记一行（不记原文）；她的话再退信（施工 O-25 下，「退信」）。
    async fn failed(
        &mut self,
        session: &str,
        item: &Item,
        (why, detail): (&str, Option<String>),
    ) -> Result<(), Gone> {
        let venue = self
            .peers
            .get(session)
            .map_or_else(String::new, |peer| peer.venue.to_string());
        let chars = item.piece.text.chars().count();
        tracing::warn!(target: TARGET, venue = %venue, why, chars, "reply not sent");
        let mut body = json!({"queued": item.queued, "why": why});
        if let Some(detail) = &detail {
            body["detail"] = json!(detail);
        }
        self.append(session, None, FAILED, body).await?;
        match item.piece.what {
            What::Reply { .. } => {
                let detail = detail.unwrap_or_default();
                self.undelivered(session, &venue, item, (why, &detail))
                    .await
            }
            What::Notice(_) | What::Receipt => Ok(()),
        }
    }

    /// 她的一段 `item` 没发出去（为什么 `why`、NapCat 说的 `detail`，没有的是空的）：照退信的模板写一块，经核心的 `session.note`
    /// 记进会话 `session`，命令编号是入队那一条的序号加 `/note`（「退信」第 2、3 条）。`venue` 只记运行日志。核心拒了的（照说
    /// 不会）记一行，不再试。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    async fn undelivered(
        &mut self,
        session: &str,
        venue: &str,
        item: &Item,
        (why, detail): (&str, &str),
    ) -> Result<(), Gone> {
        let head: String = item.piece.text.trim().chars().take(HEAD).collect();
        let fields = BTreeMap::from([("why", why), ("detail", detail), ("text", head.as_str())]);
        let text = match self.rules.undelivered().render(&fields) {
            Ok(text) => text,
            Err(error) => {
                // 照说不会：起来时查过模板只要这三个字段（`rules/facts.rs`）。
                tracing::warn!(target: TARGET, venue, error = %error, "undelivered not written");
                return Ok(());
            }
        };
        let id = format!("{}/note", item.queued);
        let params = json!({"session": session, "facts": [{"kind": UNDELIVERED, "text": text}]});
        let reply = self.core.call_as(&id, "session.note", params).await?;
        match reason(&reply) {
            None => tracing::info!(target: TARGET, venue, why, "undelivered noted"),
            Some(reason) => tracing::warn!(target: TARGET, venue, reason, "note refused"),
        }
        Ok(())
    }
}

/// 入队那一条的 `body`（第 2 条）：`{kind, text}`，她的话另带 `line`（会话 `session`，主线）、`turn`，提示另带 `reason`。
fn queued_body(session: &str, piece: &Piece) -> Value {
    match &piece.what {
        What::Reply { turn, .. } => {
            json!({"kind": REPLY, "text": piece.text, "line": session, "turn": turn})
        }
        What::Notice(reason) => json!({"kind": NOTICE, "reason": reason, "text": piece.text}),
        What::Receipt => json!({"kind": RECEIPT, "text": piece.text}),
    }
}
