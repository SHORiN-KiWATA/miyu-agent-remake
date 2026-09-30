//! 账本里跨会话的几条（施工 C-1，`docs/blueprint/kernel/history.md`「账本查的规矩」，`cross-session.md`「效果
//! peer.watch」「事件 peer.idle」）：在等哪几个会话的通知，`peer.idle` 只认在等的。
//!
//! 订的记录是工具结果的效果 `peer.watch`：一次订记一项，在哪一轮、从那条结果的时刻算起。算不算在等照回合看，撤销、恢复
//! 不动这些记录：订它的那一轮撤掉了就不算，恢复了又算。又订了一次的从新的时刻算，撤掉后订的那一轮，回到前一次的时刻。
//! 收到 `peer.idle`，那个会话以前订的都了结、清掉，再订从新的算起。账本随订的次数长，收到通知时清掉那个会话的。
//!
//! 施工 C-2 多两样，防刷屏用（`cross-session.md` 第五条第 5 款）：收下的别的会话的话，每个发话方照先后记时刻和字的哈希；
//! 还没听到的是哪几句。「别的会话」是 `by` 是会话、既不是父会话（`session.created` 的 `parent`）也不是这个会话派的子代理
//! （[`Ledger::subagent_in`]）。账本不知道窗口多长，时刻和哈希都记着，由内核照策略的数去数：随收下的句数长，一句一个
//! 时刻、一个哈希。

use std::collections::{BTreeMap, BTreeSet};

use super::Ledger;
use crate::block::Block;
use crate::event::{Body, Effect, ErrorClass, Event, IdleReason, PeerIdle, PeerWatch};
use crate::id::{ContentHash, Seq, SessionId, TurnId};
use crate::origin::{By, Session};
use crate::time::Timestamp;

/// 在等的通知。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Peers {
    /// 这个会话自己：知道的才查订的是不是自己（[`Ledger::for_session`]）。
    own: Option<SessionId>,
    /// 每个被等的会话，上一次收到它的通知以后订的几次，照先后。
    watches: BTreeMap<SessionId, Vec<Watch>>,
    /// 父会话：`session.created` 的 `parent`，主会话没有（施工 C-2）。它发来的是交代、留言，不是别的会话的话。
    parent: Option<SessionId>,
    /// 收下的别的会话的话，每个发话方照先后：时刻、字的哈希（施工 C-2）。
    said: BTreeMap<SessionId, Vec<Said>>,
    /// 还没听到的别的会话的话（施工 C-2）：主对话的请求、回复看到了就清掉。
    unheard: BTreeSet<Seq>,
}

/// 收下的一句别的会话的话：时刻、字的哈希。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Said {
    at: Timestamp,
    digest: ContentHash,
}

/// 订了一次：在哪一轮，从哪一刻算起。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Watch {
    turn: TurnId,
    since: Timestamp,
}

impl Ledger {
    /// 会话 `session` 的空账本（施工 C-1）：和 [`Ledger::default`] 一样，只是知道自己是哪个会话，订「空了告诉我」订的是
    /// 自己的拒。内核造会话、载入都用它；只拿账本数东西的读者（例如撤销的回应算停掉的任务）用 `default`，不查这一条。
    pub fn for_session(session: SessionId) -> Ledger {
        Ledger {
            peers: Peers {
                own: Some(session),
                ..Peers::default()
            },
            ..Ledger::default()
        }
    }

    /// 在等哪几个会话的通知，各从哪一刻算起，照编号（施工 C-1）：订它的那一轮还没撤掉的里面，最近订的那一次。收到过
    /// 通知的，那以前订的不算。
    pub fn watching(&self) -> impl Iterator<Item = (&SessionId, Timestamp)> {
        self.peers.watches.iter().filter_map(|(session, watches)| {
            watches
                .iter()
                .rev()
                .find(|watch| self.turns.binary_search(&watch.turn).is_ok())
                .map(|watch| (session, watch.since))
        })
    }

    /// `by` 是会话 `session` 的话是不是别的会话发来的（施工 C-2）：它不是这个会话的父会话，也不是这个会话派的子代理
    /// （被停掉的、撤掉的回合里派的也认）。
    pub fn is_peer(&self, session: &SessionId) -> bool {
        self.peers.parent.as_ref() != Some(session) && self.subagent_in(session).is_none()
    }

    /// 发话方 `from` 收下的话里，时刻不早于 `since`（Unix 毫秒）的那几句的字的哈希，照先后（施工 C-2）。
    pub fn peer_said(&self, from: &SessionId, since: i64) -> impl Iterator<Item = &ContentHash> {
        self.peers
            .said
            .get(from)
            .into_iter()
            .flatten()
            .filter(move |said| said.at.unix_millis() >= since)
            .map(|said| &said.digest)
    }

    /// 还没听到的别的会话的话有几句，不分发话方（施工 C-2）：排在正在进行的那一轮的回报队里的、记在一边的都算，主对话的
    /// 请求看到了就不算。
    pub fn unheard_from_peers(&self) -> usize {
        self.peers.unheard.len()
    }

    /// `peer.idle`：这时在等那个会话；`idle` 的 `by` 是那个会话，`expired`、`gone` 的是内核，不认识的原因不查 `by`。
    pub(super) fn check_idle(&self, idle: &PeerIdle, by: &By) -> Result<(), String> {
        let session = &idle.session;
        if !self.watching().any(|(watched, _)| watched == session) {
            return Err(format!(
                "session {session} is not being watched: never watched, the watching turn was undone, or its notice already came"
            ));
        }
        let (fits, who) = match &idle.reason {
            IdleReason::Idle => (
                matches!(by, By::Session(Session { id }) if id == session),
                format!("session {session}"),
            ),
            IdleReason::Expired | IdleReason::Gone => {
                (matches!(by, By::Kernel), "the kernel".to_string())
            }
            IdleReason::Other(_) => return Ok(()),
        };
        match fits {
            true => Ok(()),
            false => Err(format!(
                "peer.idle for session {session} with reason {} should be by {who}",
                idle.reason.as_str()
            )),
        }
    }
}

impl Peers {
    /// 一条工具结果的效果：订的不是这个会话自己。照效果的先后，第一个违反的报出来。
    pub(super) fn check_watch(&self, effects: &[Effect]) -> Result<(), String> {
        match watches(effects).find(|watch| self.own.as_ref() == Some(&watch.session)) {
            Some(watch) => Err(format!(
                "session {} is this session: a session cannot watch itself",
                watch.session
            )),
            None => Ok(()),
        }
    }

    /// 记下查过的这一条带来的变化：订的记下在哪一轮、从这条的时刻算起；收到通知，那个会话以前订的都了结。`peer` 是这一条
    /// 是别的会话发来的话时，发话的那个会话（施工 C-2，账本照 [`Ledger::is_peer`] 认好交来）：记下时刻、哈希，算没听到。
    /// 主对话的请求、回复看到了的，不再算没听到。
    pub(super) fn record(&mut self, event: &Event, peer: Option<SessionId>) {
        match &event.body {
            Body::SessionCreated(created) => self.parent = created.parent.clone(),
            Body::MessageUser(message) => {
                if let Some(from) = peer {
                    self.said.entry(from).or_default().push(Said {
                        at: event.at,
                        digest: digest(&message.blocks),
                    });
                    self.unheard.insert(event.seq);
                }
            }
            // 主对话的请求才算她听到了：回顾这类辅助请求、压缩的摘要请求、暂停着没发出去的那一条都不算（照回报的规矩，
            // `kernel/session.md`「回报」）。
            Body::ModelCalled(called)
                if !called.aside()
                    && called.compaction.is_none()
                    && called
                        .error
                        .as_ref()
                        .is_none_or(|error| error.class != ErrorClass::CompactionPaused) =>
            {
                self.unheard.retain(|unheard| *unheard > called.seen);
            }
            Body::MessageAssistant(reply) => {
                self.unheard.retain(|unheard| *unheard > reply.seen);
            }
            // 查过了：工具结果带着正在进行的回合。
            Body::ToolResult(result) => {
                if let Some(turn) = event.turn {
                    for watch in watches(&result.effects) {
                        self.watches
                            .entry(watch.session.clone())
                            .or_default()
                            .push(Watch {
                                turn,
                                since: event.at,
                            });
                    }
                }
            }
            Body::PeerIdle(idle) => {
                self.watches.remove(&idle.session);
            }
            _ => {}
        }
    }
}

/// 一句话的字的哈希（施工 C-2，第五条第 2 款「一个字节都不差」）：内容块照日志里的写法写成 JSON，取 SHA-256。
///
/// # Panics
///
/// 实际不会 panic：内容块里只有字符串、数字和原样的 JSON，写成 JSON 不会失败。
pub(crate) fn digest(blocks: &[Block]) -> ContentHash {
    let bytes = serde_json::to_vec(blocks).expect("内容块写成 JSON 不会失败");
    ContentHash::of(&bytes)
}

/// 效果里订的会话，照先后。
fn watches(effects: &[Effect]) -> impl Iterator<Item = &PeerWatch> {
    effects.iter().filter_map(|effect| match effect {
        Effect::PeerWatch(watch) => Some(watch),
        _ => None,
    })
}
