//! 别的会话发来的话，内核这一头（施工 C-2，`docs/blueprint/cross-session.md` 第四条、第五条第 1 到 3 款，
//! `docs/blueprint/kernel/session.md`「别的会话发来的话」）。
//!
//! 认：命令 `Send`，`by` 是一个会话，它既不是这个会话的父会话，也不是这个会话派的子代理的子会话（账本的
//! [`crate::ledger::Ledger::is_peer`]）。过了防刷屏的，照「别的 harness 发来的话」收（`messages.rs`）：它是别处来的，不带
//! 回合编号，照回报的规矩叫不叫醒她，不是人说的话。
//!
//! 防刷屏照账本算，纯逻辑，时刻用这条命令的 `at`，载入时照日志算得回来，重启以后数不会清零。先查一字不差的（它不占
//! 限速的数），再查限速，最后查没听到的上限。过不了的拒绝，什么都不记。

use super::Session;
use super::action::Reason;
use crate::block::Block;
use crate::id::SessionId;
use crate::ledger::digest;
use crate::time::Timestamp;

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
}
