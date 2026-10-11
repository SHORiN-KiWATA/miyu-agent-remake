//! 「空了告诉我」，等的这一边的执行器（施工 C-6，`docs/blueprint/cross-session.md` 第六条第 3、8、9 款，
//! `docs/blueprint/session/tools.md`「订、计时、再订」）：工具只报效果 `peer.watch`，订是这里的事。
//!
//! 每送完一批，照内核这时在等的（[`Session::watching`]）和这里订过的比：新多出来的、起算时刻变了的（撤掉又订、又订了
//! 一次），经会话表交给那个会话的 actor 一个「订」，同时按 `peers.watch_hours` 起一个计时；不在等了的（收到了通知、订它的
//! 那一轮撤掉了、作废了），计时撤掉。落了盘才订：一批送完的时候，这一批追加的都已经落了盘（`actor.rs` 的 `drain`），通知
//! 不会赶在这边记下在等以前到。载入、恢复撤销以后，账本里在等的都算新多出来的，走的是同一条路；已经到点的不订，当场交
//! 作废。
//!
//! 订不上的：那个会话不在了，交内核 `gone`；别的（它停了、核心正在停）记一行 `WARN watch not placed`，计时照走，到点作废。
//! 计时到了交内核 `expired`：订了又订的，旧的计时到了，内核照账本的起算时刻不理。
//!
//! [`Session::watching`]: miyu_kernel::session::Session::watching

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::AbortHandle;
use tracing::Instrument;

use miyu_kernel::event::IdleReason;
use miyu_kernel::id::SessionId;
use miyu_kernel::session::Session;
use miyu_kernel::time::Timestamp;

use crate::TARGET;
use crate::agents::Agents;
use crate::port::Back;
use crate::spawn::NotWatched;

/// 一小时多少毫秒。
const HOUR_MS: i64 = 3_600_000;

/// 订过的：每个在等的会话，从哪一刻算起，和它的计时。
#[derive(Default)]
pub(crate) struct Watches {
    placed: BTreeMap<SessionId, (Timestamp, AbortHandle)>,
}

impl Watches {
    /// 一个都没订（施工 V-2 再补）：空闲的会话退不退照它看。
    pub(crate) fn is_empty(&self) -> bool {
        self.placed.is_empty()
    }

    /// 一批送完了：照内核这时在等的去订、计时，不在等了的撤掉计时。`now` 是这一刻，`agents` 是会话表的端口（没有的订不了，
    /// 照样计时、到点作废），到点、不在了交回 `backs`。
    pub(crate) fn sync(
        &mut self,
        session: &Session,
        now: Timestamp,
        agents: Option<&Arc<Agents>>,
        backs: &mpsc::UnboundedSender<Back>,
    ) {
        let watching: BTreeMap<SessionId, Timestamp> = session.watching().into_iter().collect();
        self.placed.retain(|peer, (since, timer)| {
            let still = watching.get(peer) == Some(since);
            if !still {
                timer.abort();
            }
            still
        });
        let hours = i64::try_from(session.watch_hours()).unwrap_or(i64::MAX);
        for (peer, since) in watching {
            if self.placed.contains_key(&peer) {
                continue;
            }
            let due = since
                .unix_millis()
                .saturating_add(hours.saturating_mul(HOUR_MS));
            let wait = due.saturating_sub(now.unix_millis());
            if wait > 0
                && let Some(agents) = agents
            {
                place(agents, peer.clone(), since, backs.clone());
            }
            let timer = expire_after(wait, peer.clone(), backs.clone());
            self.placed.insert(peer, (since, timer));
        }
    }
}

/// 计时撤掉：actor 停了，到点了也没人要。
impl Drop for Watches {
    fn drop(&mut self) {
        for (_, timer) in self.placed.values() {
            timer.abort();
        }
    }
}

/// 起一个任务，经会话表把「`agents.session` 在等 `peer`」交过去。不在了的交回 `gone`，别的订不上的记一行运行日志。
fn place(
    agents: &Arc<Agents>,
    peer: SessionId,
    since: Timestamp,
    backs: mpsc::UnboundedSender<Back>,
) {
    let port = Arc::clone(&agents.port);
    let watcher = agents.session.clone();
    tokio::spawn(
        async move {
            match port.watch(peer.clone(), watcher, since).await {
                Ok(()) => {}
                Err(NotWatched::Gone) => send(&backs, peer, IdleReason::Gone),
                Err(NotWatched::Failed(error)) => {
                    tracing::warn!(target: TARGET, session = peer.short(), error = error.as_str(), "watch not placed");
                }
            }
        }
        .in_current_span(),
    );
}

/// 起一个计时：`wait` 毫秒以后（已经过了的当场）交回 `expired`。交回撤掉它的那一头。
fn expire_after(wait: i64, peer: SessionId, backs: mpsc::UnboundedSender<Back>) -> AbortHandle {
    let wait = Duration::from_millis(u64::try_from(wait).unwrap_or(0));
    tokio::spawn(async move {
        tokio::time::sleep(wait).await;
        send(&backs, peer, IdleReason::Expired);
    })
    .abort_handle()
}

/// 交回 actor。会话停了就送不进去，丢掉。
#[expect(
    clippy::let_underscore_must_use,
    reason = "会话停了：等不到了也没人要，丢掉"
)]
fn send(backs: &mpsc::UnboundedSender<Back>, session: SessionId, reason: IdleReason) {
    let _ = backs.send(Back::WatchEnded { session, reason });
}
