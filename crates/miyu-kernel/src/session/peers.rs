//! 别的会话发来的话，内核这一头（施工 C-2，`docs/blueprint/cross-session.md` 第四条、第五条第 1 到 3 款，
//! `docs/blueprint/kernel/session.md`「别的会话发来的话」）。
//!
//! 认：命令 `Send`，`by` 是一个会话，它既不是这个会话的父会话，也不是这个会话派的子代理的子会话（账本的
//! [`crate::ledger::Ledger::is_peer`]）。过了防刷屏的，照「别的 harness 发来的话」收（`messages.rs`）：它是别处来的，不带
//! 回合编号，照回报的规矩叫不叫醒她，不是人说的话。
//!
//! 防刷屏照账本算，纯逻辑，时刻用这条命令的 `at`，载入时照日志算得回来，重启以后数不会清零。先查一字不差的（它不占
//! 限速的数），再查限速，最后查没听到的上限。过不了的拒绝，什么都不记。
//!
//! 「空了告诉我」（施工 C-6，`cross-session.md` 第六条，`kernel/session.md`「空了的通知」）也在这里：
//!
//! - 等的这一边：被等的会话交来的通知（命令 `PeerIdle`）账本说在等的，记 `peer.idle`（`idle`），照回报的规矩叫不叫醒她；
//!   不在等的拒绝 `unknown_watch`。执行器交来的到点、不在了（输入 `WatchEnded`），还在等、确实到了点的记 `peer.idle`
//!   （`expired`、`gone`，`by` 是内核，`cause` 是订它的那一轮的），只记下、不叫醒。
//! - 两边都用的查询：在等哪几个会话（[`Session::watching`]，执行器照它去订、计时），「空了」（[`Session::vacant`]），通知
//!   带的那一行（[`Session::last_line`]）。

use super::Session;
use super::action::{Action, Reason};
use super::input::Input;
use super::jobs::Waker;
use super::rejected;
use crate::block::Block;
use crate::event::{Body, IdleReason, PeerIdle};
use crate::id::{CommandId, SessionId};
use crate::ledger::digest;
use crate::origin::By;
use crate::time::Timestamp;

/// 一小时多少毫秒。
const HOUR_MS: i64 = 3_600_000;

impl Session {
    /// 会话 `from` 这时发来 `blocks` 过不过得了防刷屏：过得了的没有，过不了的交回原因码。
    ///
    /// - 过去 `peers.window` 秒里（照 `at` 往前数，含正好那么久以前的那一刻）这个发话方记下过一字不差的一句：
    ///   `duplicate_message`。
    /// - 过去 `peers.window` 秒里这个发话方记下了 `peers.burst` 句：`too_many_messages`。
    /// - 还没听到的别的会话的话，不分发话方，已经有 `peers.unread` 句：`inbox_full`。
    pub(super) fn flooding(
        &self,
        from: &SessionId,
        at: Timestamp,
        blocks: &[Block],
    ) -> Option<Reason> {
        let peers = &self.policy.peers;
        let window = i64::try_from(peers.window.saturating_mul(1000)).unwrap_or(i64::MAX);
        let since = at.unix_millis().saturating_sub(window);
        let digest = digest(blocks);
        let mut recent = 0;
        for said in self.ledger.peer_said(from, since) {
            if *said == digest {
                return Some(Reason::DuplicateMessage);
            }
            recent += 1;
        }
        if recent >= peers.burst {
            return Some(Reason::TooManyMessages);
        }
        if self.ledger.unheard_from_peers() >= peers.unread {
            return Some(Reason::InboxFull);
        }
        None
    }

    /// 在等哪几个会话的通知、各从哪一刻算起，照编号（施工 C-6，账本的 `watching()`）：订它的那一轮还没撤掉、那以后没收到过
    /// 它的通知的。纯查询：执行器每送完一批照它看有没有新多出来的，去订、计时（`cross-session.md` 第六条第 3、8 款）。
    pub fn watching(&self) -> Vec<(SessionId, Timestamp)> {
        self.ledger
            .watching()
            .map(|(session, since)| (session.clone(), since))
            .collect()
    }

    /// 订了多久没等到通知就作废，单位小时（施工 C-6，策略数据 `peers.watch_hours`）。纯查询：执行器照它和 [`Session::watching`]
    /// 的起算时刻计时，到点交 `WatchEnded`。
    pub fn watch_hours(&self) -> u64 {
        self.policy.peers.watch_hours
    }

    /// 「空了」（施工 C-6，`cross-session.md` 第六条第 4 款）：内核说空闲（[`Session::idle`]），而且它派的子代理都不欠它
    /// 回报（[`Session::waiting_children`] 是空的）。和 `miyu ask` 等到的是同一个时刻。后台命令不算。收到了「要重启了」的
    /// 不算：被重启打断的那一轮再起来接着干，这时说空了是假的。纯查询：被等的那一边的 actor 每送完一批照它看要不要发通知。
    pub fn vacant(&self) -> bool {
        self.idle() && !self.restarting && self.ledger.waiting_children().next().is_none()
    }

    /// 通知里带的那一行（施工 C-6，第六条第 6 款）：最近结束的那一轮最后一条有字的回复的第一行，去掉前后空白；超过
    /// `peers.status_chars` 个字的截到那么多个字，末尾接 `…`。和向上回报拿正文是同一个认法（`kernel/session.md`「向上回报」
    /// 第 3 条）。那一轮一个字都没说的没有。纯查询：被等的那一边发通知时交。
    pub fn last_line(&self) -> Option<String> {
        let line = self.duty.answer().trim().lines().next()?.trim();
        if line.is_empty() {
            return None;
        }
        let limit = self.policy.peers.status_chars;
        Some(match line.chars().nth(limit) {
            Some(_) => line.chars().take(limit).chain(['…']).collect(),
            None => line.to_string(),
        })
    }

    /// 被等的会话交来的通知：`by` 是它，这边账本说在等它的，记一条 `peer.idle`（`idle`，带它交来的那一行），不带回合编号，
    /// `cause` 是这个命令；照回报的规矩叫不叫醒她（它是别处来的，不是哪个任务的，和别的会话发来的话一样一律算会叫醒她的）。
    /// 落了盘回应，只附这一条的序号。不在等的（没订过、订它的那一轮撤掉了、已经收到过、作废了）、`by` 不是一个会话的：
    /// 拒绝，`unknown_watch`，什么都不记。
    pub(super) fn peer_idle(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        status: Option<String>,
    ) -> Vec<Action> {
        let By::Session(from) = &by else {
            return vec![rejected(id, Reason::UnknownWatch)];
        };
        let body = Body::PeerIdle(PeerIdle {
            session: from.id.clone(),
            reason: IdleReason::Idle,
            status,
        });
        match self.land(at, by, Some(id.clone()), body, Some(Waker::Peer)) {
            Ok(events) => {
                self.accept(id, vec![events[0].seq]);
                vec![Action::Append(events)]
            }
            Err(_) => vec![rejected(id, Reason::UnknownWatch)],
        }
    }

    /// 执行器交来的等不到了：还在等 `session`，`gone` 的、`expired` 的到了点（起算以后 `peers.watch_hours` 小时，含正好那一刻）
    /// 的，记一条 `peer.idle`，`by` 是内核，`cause` 是订它的那一轮的，只记下、不叫醒（照 `undone`、`aborted` 的回报）。别的
    /// （不在等了、还没到点、别的原因）不理：订了又订的，旧的计时到了不算。读回日志的时候到的先放着，读回来再记。
    pub(super) fn watch_ended(
        &mut self,
        at: Timestamp,
        session: SessionId,
        reason: IdleReason,
    ) -> Vec<Action> {
        if let Some(reading) = self.reading.as_mut() {
            reading.later.push(Input::WatchEnded {
                at,
                session,
                reason,
            });
            return Vec::new();
        }
        let Some((since, cause)) = self.ledger.watch_of(&session) else {
            return Vec::new();
        };
        let hours = i64::try_from(self.policy.peers.watch_hours).unwrap_or(i64::MAX);
        let due = since
            .unix_millis()
            .saturating_add(hours.saturating_mul(HOUR_MS));
        let ended = match reason {
            IdleReason::Gone => true,
            IdleReason::Expired => at.unix_millis() >= due,
            IdleReason::Idle | IdleReason::Other(_) => false,
        };
        if !ended {
            return Vec::new();
        }
        let cause = cause.cloned();
        let body = Body::PeerIdle(PeerIdle {
            session,
            reason,
            status: None,
        });
        match self.land(at, By::Kernel, cause, body, None) {
            Ok(events) => vec![Action::Append(events)],
            Err(_) => Vec::new(),
        }
    }
}

/// 一条 `peer.idle` 叫不叫醒她（施工 C-6，`cross-session.md` 第六条第 7、8 款）：空下来了的叫醒；作废、不在了的只记下，
/// 它们不是做出来的结果。不认识的原因叫醒：她至少知道等到了头，照回报不认识的原因。
pub(super) fn idle_wakes(idle: &PeerIdle) -> bool {
    matches!(idle.reason, IdleReason::Idle | IdleReason::Other(_))
}
