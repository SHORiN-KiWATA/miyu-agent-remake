//! 账本里跨会话的几条（施工 C-1，`docs/blueprint/kernel/history.md`「账本查的规矩」，`cross-session.md`「效果
//! peer.watch」「事件 peer.idle」）：在等哪几个会话的通知，`peer.idle` 只认在等的。
//!
//! 订的记录是工具结果的效果 `peer.watch`：一次订记一项，在哪一轮、从那条结果的时刻算起。算不算在等照回合看，撤销、恢复
//! 不动这些记录：订它的那一轮撤掉了就不算，恢复了又算。又订了一次的从新的时刻算，撤掉后订的那一轮，回到前一次的时刻。
//! 收到 `peer.idle`，那个会话以前订的都了结、清掉，再订从新的算起。账本随订的次数长，收到通知时清掉那个会话的。

use std::collections::BTreeMap;

use super::Ledger;
use crate::event::{Body, Effect, Event, IdleReason, PeerIdle, PeerWatch};
use crate::id::{SessionId, TurnId};
use crate::origin::{By, Session};
use crate::time::Timestamp;

/// 在等的通知。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Peers {
    /// 这个会话自己：知道的才查订的是不是自己（[`Ledger::for_session`]）。
    own: Option<SessionId>,
    /// 每个被等的会话，上一次收到它的通知以后订的几次，照先后。
    watches: BTreeMap<SessionId, Vec<Watch>>,
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
                watches: BTreeMap::new(),
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

    /// 记下查过的这一条带来的变化：订的记下在哪一轮、从这条的时刻算起；收到通知，那个会话以前订的都了结。
    pub(super) fn record(&mut self, event: &Event) {
        match &event.body {
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

/// 效果里订的会话，照先后。
fn watches(effects: &[Effect]) -> impl Iterator<Item = &PeerWatch> {
    effects.iter().filter_map(|effect| match effect {
        Effect::PeerWatch(watch) => Some(watch),
        _ => None,
    })
}
