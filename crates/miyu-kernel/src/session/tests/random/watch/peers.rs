//! 看守查别的会话发来的话（施工 C-2，`docs/blueprint/cross-session.md` 第四条、第五条第 1 到 3 款）：
//!
//! - 过不了防刷屏的拒绝，原因照看守自己数的：一字不差的 `duplicate_message` 先查、不占数，再查限速 `too_many_messages`，
//!   最后查没听到的上限 `inbox_full`；拒绝的只有一个回应，什么都不记；
//! - 收下的记一条 `message.user`，不带回合编号，`by`、`cause` 照交来的；同一批里没有作废、撤回（它不是人说的话）；
//! - 叫不叫醒她照回报的规矩（`watch/reports.rs` 的 [`Watch::report_check`]）：它不是哪个任务的，一律算会叫醒她的；
//! - 还没听到的：主对话的请求、回复看到了才不算（照 `watch/reports.rs` 排着的那一套）。
//!
//! 空了的通知（施工 C-6，`cross-session.md` 第六条第 7 到 9 款）：
//!
//! - 等的会话交来的「空了」：这时在等它（照内核交给执行器的 `watching()`）的记一条 `peer.idle`，`by` 是它、不带回合编号、
//!   `cause` 是这个命令；不在等的拒绝 `unknown_watch`，什么都不记；
//! - 执行器交的作废、不在了：在等、作废的到了点（随机的策略订了就到点）的记一条，`by` 是内核；别的不理；读回日志的时候
//!   先放着；
//! - 记下了的就不再等它；叫不叫醒她照回报那一套（`watch/reports.rs`）：空下来了的叫醒，作废、不在了的只记下。

use super::*;
use crate::event::IdleReason;
use crate::session::tests::random::peering::LIMITS;

/// 看守记着的别的会话的话。
#[derive(Default)]
pub(in super::super) struct Peers {
    /// 收下的：谁发的、时刻、原话，照先后。
    said: Vec<(By, Timestamp, Vec<Block>)>,
    /// 还没听到的。
    unheard: BTreeSet<Seq>,
}

/// 送进去之前判出来的。
pub(super) enum Expect {
    /// 过不了防刷屏：拒绝，原因是这个。
    Refused(Reason),
    /// 收下：记一条，`by`、`cause` 是这样。
    Recorded(By, CommandId),
}

/// 通知送进去之前判出来的（施工 C-6）。
pub(super) enum Notice {
    /// 不在等：拒绝，`unknown_watch`。
    Refused,
    /// 不理：什么都不出。是在等、还没到点的作废的，`early`。
    Ignored { early: bool },
    /// 记一条，`by`、原因是这样；是哪个会话的。
    Recorded(By, IdleReason, SessionId),
}

impl Watch {
    /// 送进一条输入之前：等的会话交来的「空了」、执行器交的作废和不在了，照内核这时在等的判出该怎样（施工 C-6）。
    pub(super) fn before_notice(&self, session: &Session, input: &Input) -> Option<Notice> {
        let since = |peer: &SessionId| {
            session
                .watching()
                .into_iter()
                .find(|(watched, _)| watched == peer)
                .map(|(_, since)| since)
        };
        match input {
            Input::Command(received) if self.fresh(&received.id) => {
                let Command::PeerIdle { .. } = &received.command else {
                    return None;
                };
                let By::Session(from) = &received.by else {
                    return Some(Notice::Refused);
                };
                Some(match since(&from.id) {
                    Some(_) => {
                        Notice::Recorded(received.by.clone(), IdleReason::Idle, from.id.clone())
                    }
                    None => Notice::Refused,
                })
            }
            Input::WatchEnded {
                at,
                session: peer,
                reason,
            } => {
                let due = |since: Timestamp| {
                    since.unix_millis() + i64::try_from(LIMITS.watch_hours).unwrap() * 3_600_000
                        <= at.unix_millis()
                };
                let ended = self.undo.reading.is_none()
                    && since(peer).is_some_and(|since| match reason {
                        IdleReason::Gone => true,
                        IdleReason::Expired => due(since),
                        _ => false,
                    });
                let early = self.undo.reading.is_none()
                    && *reason == IdleReason::Expired
                    && since(peer).is_some_and(|since| !due(since));
                Some(match ended {
                    true => Notice::Recorded(By::Kernel, reason.clone(), peer.clone()),
                    false => Notice::Ignored { early },
                })
            }
            _ => None,
        }
    }

    /// 送进去以后：照判出来的查（施工 C-6）。
    pub(super) fn after_notice(
        &mut self,
        session: &Session,
        actions: &[Action],
        notice: Option<Notice>,
    ) {
        let seed = self.seed;
        match notice {
            None => {}
            Some(Notice::Refused) => {
                self.seen_paths.insert("不在等的通知被拒");
                assert!(
                    matches!(
                        actions,
                        [Action::Reply {
                            outcome: Outcome::Rejected {
                                reason: Reason::UnknownWatch,
                                ..
                            },
                            ..
                        }]
                    ),
                    "种子 {seed}：不在等的通知该拒，unknown_watch：{actions:?}"
                );
            }
            Some(Notice::Ignored { early }) => {
                if early {
                    self.seen_paths.insert("没到点的作废不理");
                }
                assert!(
                    actions.is_empty(),
                    "种子 {seed}：不理的作废、不在了：{actions:?}"
                );
            }
            Some(Notice::Recorded(by, reason, peer)) => {
                self.seen_paths.insert(match reason {
                    IdleReason::Expired => "作废了",
                    IdleReason::Gone => "不在了",
                    _ => "收下了空了的通知",
                });
                let first = actions.iter().find_map(|action| match action {
                    Action::Append(events) => events.first(),
                    _ => None,
                });
                assert!(
                    first.is_some_and(|event| matches!(&event.body, Body::PeerIdle(idle)
                        if idle.reason == reason && idle.session == peer)
                        && event.turn.is_none()
                        && event.by == by),
                    "种子 {seed}：通知记一条，不带回合编号，by、原因照判的：{actions:?}"
                );
                assert!(
                    !session
                        .watching()
                        .iter()
                        .any(|(watched, _)| *watched == peer),
                    "种子 {seed}：等到了就不再等"
                );
            }
        }
    }

    /// `by` 是别的会话：一个会话，不是这个会话派的子代理（随机测试的会话是主会话，没有父会话）。
    pub(super) fn is_peer(&self, by: &By) -> bool {
        matches!(by, By::Session(session) if !self
            .reports
            .jobs
            .values()
            .any(|job| job.session.as_ref() == Some(&session.id)))
    }

    /// 送进一条输入之前：别的会话发来的一句，照看守自己数的判出收还是拒。
    pub(super) fn before_peer(&self, input: &Input) -> Option<Expect> {
        let Input::Command(received) = input else {
            return None;
        };
        let Command::Send { blocks, .. } = &received.command else {
            return None;
        };
        if !self.fresh(&received.id) || blocks.is_empty() || !self.is_peer(&received.by) {
            return None;
        }
        let since = received.at.unix_millis() - i64::try_from(LIMITS.window * 1000).unwrap();
        let recent: Vec<&Vec<Block>> = self
            .peers
            .said
            .iter()
            .filter(|(by, at, _)| *by == received.by && at.unix_millis() >= since)
            .map(|(_, _, said)| said)
            .collect();
        Some(if recent.contains(&blocks) {
            Expect::Refused(Reason::DuplicateMessage)
        } else if recent.len() >= LIMITS.burst {
            Expect::Refused(Reason::TooManyMessages)
        } else if self.peers.unheard.len() >= LIMITS.unread {
            Expect::Refused(Reason::InboxFull)
        } else {
            Expect::Recorded(received.by.clone(), received.id.clone())
        })
    }

    /// 送进去以后：照判出来的查。
    pub(super) fn after_peer(&mut self, actions: &[Action], expect: Option<Expect>) {
        let seed = self.seed;
        match expect {
            None => {}
            Some(Expect::Refused(reason)) => {
                self.seen_paths.insert(match reason {
                    Reason::DuplicateMessage => "别的会话一字不差的话被拒",
                    Reason::TooManyMessages => "别的会话说得太多被拒",
                    _ => "没听到的别的会话的话太多被拒",
                });
                assert!(
                    matches!(actions, [Action::Reply { outcome: Outcome::Rejected { reason: got, .. }, .. }] if *got == reason),
                    "种子 {seed}：别的会话的话该拒，{reason:?}：{actions:?}"
                );
            }
            Some(Expect::Recorded(by, cause)) => {
                self.seen_paths.insert("收下了别的会话的话");
                let batch = actions.iter().find_map(|action| match action {
                    Action::Append(events) => Some(events),
                    _ => None,
                });
                let first = batch.and_then(|events| events.first());
                assert!(
                    first.is_some_and(|event| matches!(event.body, Body::MessageUser(_))
                        && event.turn.is_none()
                        && event.by == by
                        && event.cause.as_ref() == Some(&cause)),
                    "种子 {seed}：别的会话的话记一条，不带回合编号，by、cause 照交来的：{actions:?}"
                );
                assert!(
                    batch.is_some_and(|events| !events.iter().any(|event| matches!(
                        event.body,
                        Body::ToolResult(_) | Body::MessageWithdrawn(_)
                    ))),
                    "种子 {seed}：别的会话的话不作废在等人的题、不撤回：{actions:?}"
                );
            }
        }
    }

    /// 追加的一条别的会话的话：记下时刻、原话，算没听到。
    pub(super) fn peer_said(&mut self, event: &Event) {
        if let Body::MessageUser(message) = &event.body {
            self.peers
                .said
                .push((event.by.clone(), event.at, message.blocks.clone()));
            self.peers.unheard.insert(event.seq);
        }
    }

    /// 主对话的请求、回复看到了第 `seen` 条为止：那以前的都听到了。
    pub(super) fn peers_heard(&mut self, seen: Seq) {
        self.peers.unheard.retain(|unheard| *unheard > seen);
    }
}
