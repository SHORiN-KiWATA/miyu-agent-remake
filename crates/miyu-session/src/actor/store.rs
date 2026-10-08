//! 落盘和读回（施工 3-7 中；施工 6-9 从 `actor.rs` 挪出来，那边放不下了）：在阻塞线程里写一批、同步；撤掉压缩时读回
//! 更早的日志（`docs/blueprint/session/actor.md` 第 4 条）。写不进去、读不回来，会话都停下：内存里的会话和磁盘对不上了，
//! 从磁盘重新载入最清楚（`07-存储.md` 第四节「写不进去」）。

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;
use miyu_kernel::session::Input;

use super::{Actor, Stop};
use crate::TARGET;
use crate::current::{Current, Tally};
use crate::effects;

impl Actor {
    /// 这一刻「当前的」几样（施工 9-6 上）：订阅时和订阅在同一步里交回。权限、还在跑的任务照内核这一刻的，累计的照给头看的
    /// 那一份。
    pub(super) fn current(&self) -> Current {
        Current {
            permission: self.session.permission().clone(),
            jobs: self.session.running_started(),
            tally: self.counted().tally.clone(),
        }
    }

    /// 载入时照整份日志算的累计（施工 9-6 上）：放进给头看的那一份，之后每落一批盘加上这一批。
    pub(crate) fn count_from(&self, tally: Tally) {
        self.counted().tally = tally;
    }

    /// 给头看的那一份，拿着锁。
    fn counted(&self) -> std::sync::MutexGuard<'_, crate::shown::Shown> {
        self.shown
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 在阻塞线程里写、同步，写完交回「落盘了」。写不进去就停下。撤销、恢复落了盘，她看过的照日志重算一遍
    /// （施工 4-7 上）：重算不出来的记一条运行日志，照旧用原来的，改的工具照样先核对。
    pub(super) async fn append(&mut self, events: Vec<Event>) -> Result<Option<Input>, Stop> {
        let Some(upto) = events.last().map(|event| event.seq) else {
            return Ok(None);
        };
        let mut store = self.store.take().ok_or(Stop)?;
        let reseen = effects::reverts(&events);
        // 派出去的任务、回报记进名册（施工 7-4）：写不进去的，会话照样停下，记了也不要紧。
        self.jobs.note(&events);
        // 累计的也是（施工 9-6 上）：写完、内核记下「落了盘」之前收件箱不动，订阅插不进来，和它拿到的落盘到哪一条对得上。
        self.counted().tally.add(&events);
        // 阻塞线程带着会话的 span：那边记的 `session index not updated` 也有会话编号（施工 3-8 七补）。
        let span = tracing::Span::current();
        let written = tokio::task::spawn_blocking(move || {
            span.in_scope(|| {
                let result = store.append(&events);
                let seen = (reseen && result.is_ok())
                    .then(|| store.events().map(|all| effects::seen_in(&all)));
                (store, result, seen)
            })
        })
        .await;
        match written {
            Ok((store, Ok(()), seen)) => {
                self.store = Some(store);
                match seen {
                    Some(Ok(seen)) => self.tools.see(seen),
                    Some(Err(error)) => {
                        tracing::warn!(target: TARGET, error = %error, "seen files not rebuilt");
                    }
                    None => {}
                }
                Ok(Some(Input::Stored {
                    at: self.clock.now(),
                    upto,
                }))
            }
            Ok((_, Err(error), _)) => {
                tracing::warn!(target: TARGET, kind = ?error.kind(), "write failed, stopped");
                Err(Stop)
            }
            Err(error) => {
                if error.is_panic() {
                    tracing::error!(target: TARGET, "panicked, stopped");
                }
                Err(Stop)
            }
        }
    }

    /// 读回日志（施工 6-9）：在阻塞线程里只读地一段一段读，交回第 `from` 条起的。读不了的（日志坏了、磁盘出错），记一行
    /// `read back failed, stopped`，会话停下；读的线程 panic 了，记一行 `panicked, stopped`。
    pub(super) async fn read_back(&mut self, from: Seq) -> Result<Input, Stop> {
        let store = self.store.take().ok_or(Stop)?;
        let read = tokio::task::spawn_blocking(move || {
            let events = store.events_from(from);
            (store, events)
        })
        .await;
        match read {
            Ok((store, Ok(events))) => {
                self.store = Some(store);
                Ok(Input::ReadBack {
                    at: self.clock.now(),
                    from,
                    events,
                })
            }
            Ok((_, Err(error))) => {
                tracing::warn!(target: TARGET, error = %error, "read back failed, stopped");
                Err(Stop)
            }
            Err(error) => {
                if error.is_panic() {
                    tracing::error!(target: TARGET, "panicked, stopped");
                }
                Err(Stop)
            }
        }
    }
}
