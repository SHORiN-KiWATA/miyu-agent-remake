//! 「空了告诉我」，被等的这一边（施工 C-6，2026-10-01 项目主人定改成这样，`docs/blueprint/cross-session.md`
//! 第六条第 3 到 6、9 款，`docs/blueprint/session/actor.md`「被等的名单」）：谁在等这个会话空下来，只记在 actor
//! 的内存里，不进内核、不进日志，核心一停就没了（等的那一边载入以后会再订）。同一个会话只记一个，后订的替掉先订的。
//!
//! 名单上每一项多一个「上膛」的标记。订进来的时候这个会话已经空着：不当场发，等它下一次忙完才发，免得带的是它
//! 上一轮的旧回答；除非订的起算时刻不晚于它上一次忙完的时刻（带话又订、这边手快先忙完了一轮的情形），照样当场
//! 上膛、发出去，不然等的那边要白等到 12 小时作废。这个会话一忙起来，名单上的每一项都上膛：它下一次空下来时就
//! 能发，不管订进来的时候是不是已经空着。
//!
//! 每送完一批看一次（`actor.rs` 的 `drain`）：忙着（[`Session::vacant`] 是假）的，记下「忙过」，名单上的都上膛；
//! 闲着、刚才还记着忙过的，清掉「忙过」，记下这一刻刚忙完。然后给名单上上膛的每一个发通知、清掉它们；没上膛的
//! 留着，等下一次忙完。发通知不挡着 actor：一个一个起任务发，任务起好了才写「没有在跑的回合」，免得核心在通知
//! 的路上空闲退出。
//!
//! 通知：经会话表给等的那个会话一个命令 `PeerIdle`，`by` 是这个会话，带它最近结束的那一轮最后一条有字的回复的第一行
//! （[`Session::last_line`]）。命令编号 `<这个会话>/idle/<等的会话>/<那一边起算时刻的毫秒数>`：同一次订只有一个编号，
//! 交重了那一边认得出（内核照编号只生效一次）；又订了一次的是新的编号，不会被当成重的吞掉。那一边正在撤销、恢复（`restoring`）
//! 的退避着再交，别的拒绝、交不到的记一行运行日志就完了。
//!
//! [`Session::vacant`]: miyu_kernel::session::Session::vacant
//! [`Session::last_line`]: miyu_kernel::session::Session::last_line

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use tracing::Instrument;

use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::{By, Session as From};
use miyu_kernel::session::{Command, Outcome, Reason};
use miyu_kernel::time::Timestamp;

use super::Actor;
use crate::TARGET;
use crate::spawn::SessionPort;

/// 那一边正在撤销、恢复时，第一次等多久再交，之后每次翻倍（毫秒）。
const RETRY_FIRST_MS: u64 = 100;

/// 一共最多等多久（毫秒）：撤销、恢复早该做完了。
const RETRY_TOTAL_MS: u64 = 30_000;

/// 名单上的一项：等的会话这次订的起算时刻，和有没有「上膛」。2026-10-01 项目主人定：空着时订进来的先不上膛，
/// 等这个会话下一次从忙变空才上膛；忙着订进来的，或者起算时刻不晚于它上一次忙完的时刻的，当场上膛。
#[derive(Debug, Clone, Copy)]
pub(super) struct Waiter {
    since: Timestamp,
    armed: bool,
}

/// 在等这个会话空下来的：谁在等，和它这一项的起算时刻、上没上膛。
pub(super) type Waiters = BTreeMap<SessionId, Waiter>;

impl Actor {
    /// 会话 `watcher` 等这个会话空下来：记进名单（同一个会话只记一个，后订的替掉先订的）。这个会话正忙着，或者
    /// `since` 不晚于它上一次忙完的时刻（`finished_at`），当场上膛；不然不上膛，留着等它下一次忙完。
    pub(super) fn add_waiter(&mut self, watcher: SessionId, since: Timestamp) {
        let armed = !self.session.vacant() || self.finished_at.is_some_and(|at| at >= since);
        self.waiters.insert(watcher, Waiter { since, armed });
        self.notify_if_vacant();
    }

    /// 一批送完了：等的这一边照内核在等的去订、计时（`crate::peers`）；这个会话忙不忙跟着记——忙着的，名单上
    /// 的都上膛；刚从忙变空的，记下忙完的这一刻（`finished_at`）。然后给上膛的发通知。
    pub(super) fn after_batch(&mut self) {
        let now = self.clock.now();
        let agents = self.tools.agents().cloned();
        self.watches
            .sync(&self.session, now, agents.as_ref(), &self.backs);
        let vacant = self.session.vacant();
        if !vacant {
            self.busy_seen = true;
            for waiter in self.waiters.values_mut() {
                waiter.armed = true;
            }
            self.extract_cancel();
        } else if self.busy_seen {
            self.busy_seen = false;
            self.finished_at = Some(now);
            // 从忙到闲：抽取的闹钟（施工 R-6 上）。
            self.extract_when_idle();
        }
        self.notify_if_vacant();
    }

    /// 空了：给名单上上膛的每一个发通知、清掉；没上膛的留着，等下一次忙完。没有会话表的端口的（测试里自己造的
    /// 会话）发不出去，名单照样清空。
    fn notify_if_vacant(&mut self) {
        if self.waiters.is_empty() || !self.session.vacant() {
            return;
        }
        let Some(agents) = self.tools.agents() else {
            self.waiters.clear();
            return;
        };
        let (armed, pending): (Waiters, Waiters) = std::mem::take(&mut self.waiters)
            .into_iter()
            .partition(|(_, waiter)| waiter.armed);
        self.waiters = pending;
        if armed.is_empty() {
            return;
        }
        let status = self.session.last_line();
        for (watcher, waiter) in armed {
            let port = Arc::clone(&agents.port);
            let this = agents.session.clone();
            let status = status.clone();
            tokio::spawn(
                async move { notify(&*port, this, watcher, waiter.since, status).await }
                    .in_current_span(),
            );
        }
    }
}

/// 给 `watcher` 发一次通知，等它回应，记一行运行日志。那一边正在撤销、恢复的，退避着再交同一个命令。
async fn notify(
    port: &dyn SessionPort,
    this: SessionId,
    watcher: SessionId,
    since: Timestamp,
    status: Option<String>,
) {
    let to = watcher.short().to_string();
    let Ok(id) = CommandId::parse(&format!("{this}/idle/{watcher}/{}", since.unix_millis())) else {
        tracing::warn!(target: TARGET, to = to.as_str(), "idle notice has no command id");
        return;
    };
    let by = By::Session(From { id: this });
    let (mut wait, mut waited) = (RETRY_FIRST_MS, 0);
    let outcome = loop {
        let command = Command::PeerIdle {
            status: status.clone(),
        };
        let outcome = port
            .command(watcher.clone(), id.clone(), by.clone(), command)
            .await;
        let restoring = matches!(
            outcome,
            Ok(Outcome::Rejected {
                reason: Reason::Restoring,
                ..
            })
        );
        if !restoring || waited >= RETRY_TOTAL_MS {
            break outcome;
        }
        let now = wait.min(RETRY_TOTAL_MS - waited);
        tokio::time::sleep(Duration::from_millis(now)).await;
        waited += now;
        wait *= 2;
    };
    match outcome {
        Ok(Outcome::Accepted { .. } | Outcome::Recapped { .. }) => {
            tracing::info!(target: TARGET, to = to.as_str(), "idle notice sent");
        }
        Ok(Outcome::Rejected { reason, .. }) => {
            tracing::warn!(target: TARGET, to = to.as_str(), code = reason.code(), "idle notice refused");
        }
        Err(error) => {
            tracing::warn!(target: TARGET, to = to.as_str(), error = error.as_str(), "idle notice refused");
        }
    }
}
