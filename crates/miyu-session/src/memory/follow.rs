//! 跟着看一间的记忆日志（施工 R-12 上，`docs/blueprint/memory.md`「协议」的流 `memory`，`tail -f`）：日志就是真相，给的是
//! 日志里那一行原样，头照它自己算怎么画（2026-10-11 项目主人定：给什么核心定，照仿 Linux 的原则给，一切皆文件、日志式）。
//!
//! 照听众挑，和 `memory.list` 一个判法（[`Keeper::hears`]）：这个人听不到的那一条（`ext.memory.saved`）和作废它的不给；整间的
//! （抽到哪、合并、摘要、清空）照给。清掉的哪里都不出来（`memory.md` 第二条第 5 款）：补的时候清掉了的那几条和作废它们的、
//! 清空以前的摘要（里面有清掉的字）不给；出处全死了的当它不在（`17-记忆.md` 第六节），补的时候也不给。之后追加的推的时候
//! 还没清、出处刚记下，照给；之后清空了、撤销了，头照 `memory.list` 重新列（它是现在的样子）。之后的放进一条有界的通道，
//! 攒满了算掉队，不再交。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::mpsc::{self, error::TrySendError};

use miyu_kernel::event::Event;
use miyu_recall::{Entry, MemoryBook, MemoryEvent, MemoryId, from_event};

use super::Keeper;
use super::keeper::failed;

/// 跟着看的一份：补的、到哪为止、之后的。
#[derive(Debug)]
pub struct Following {
    /// 补的：序号大于 `after` 的、这一刻落了盘的，挑过了，照先后。
    pub filled: Vec<Event>,
    /// 这一刻落了盘的最后一条的序号（一条都没有的是 0）：之后的从它后面起。
    pub upto: u64,
    /// 之后追加的，挑过了，照先后。
    pub live: mpsc::Receiver<Event>,
    /// 掉了队：之后的放不进 `live` 了，不再交。`live` 关了以后看它，分得出是掉了队还是日志那一头放下了。
    pub lagged: Arc<AtomicBool>,
}

impl Keeper {
    /// 跟着看这一间的记忆日志：`after` 以后的补（`None` 的不补），之后追加的照先后交；最多攒 `room` 条没读走的，再多算掉队。
    /// 补的那一截从磁盘读，调的一方放在阻塞线程里。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了、补的那一截读不出来：英文的一句为什么。
    pub fn follow(&self, after: Option<u64>, room: usize) -> Result<Following, String> {
        let log = self.log().map_err(failed)?;
        let (sender, live) = mpsc::channel(room.max(1));
        let lagged = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&lagged);
        let keeper = self.clone();
        let follower = Box::new(move |event: &Event, book: &MemoryBook| {
            if !keeper.followed(event, book) {
                return !sender.is_closed();
            }
            match sender.try_send(event.clone()) {
                Ok(()) => true,
                Err(TrySendError::Full(_)) => {
                    flag.store(true, Ordering::SeqCst);
                    false
                }
                Err(TrySendError::Closed(_)) => false,
            }
        });
        let (filled, upto) = log
            .follow(after, follower)
            .map_err(|error| error.to_string())?;
        // 补的里有清空的，它以前的摘要都不给；补的里没有的，清空在 `after` 以前，摘要也在它以前、本来不补。
        let cleared = filled
            .iter()
            .filter(|event| matches!(from_event(event), Some(Ok(MemoryEvent::Cleared(_)))))
            .map(|event| event.seq)
            .max();
        // 出处活不活要读回合库：拿出说的那一条，出了锁再看（同 `list`）。
        let filled: Vec<(Event, Option<Entry>)> = log.book(|book| {
            filled
                .into_iter()
                .filter(|event| self.followed(event, book))
                .filter(|event| {
                    !matches!(from_event(event), Some(Ok(MemoryEvent::Summary(_))))
                        || cleared.is_none_or(|cleared| event.seq > cleared)
                })
                .map(|event| {
                    let entry = about(&event).and_then(|id| book.get(id).cloned());
                    (event, entry)
                })
                .collect()
        });
        let filled = filled
            .into_iter()
            .filter(|(_, entry)| entry.as_ref().is_none_or(|entry| self.alive(entry)))
            .map(|(event, _)| event)
            .collect();
        Ok(Following {
            filled,
            upto,
            live,
            lagged,
        })
    }

    /// 这一行给不给这次的听众看：记下的、作废的照那一条判，清掉了的、听众不合的不给（底账里没有的照给）；别的是整间的、照给。
    fn followed(&self, event: &Event, book: &MemoryBook) -> bool {
        about(event)
            .and_then(|id| book.get(id))
            .is_none_or(|entry| !entry.cleared && self.hears(entry))
    }
}

/// 这一行说的是哪一条：记下的是它自己，作废的是它作废的那一条；别的是整间的，没有。
fn about(event: &Event) -> Option<MemoryId> {
    match from_event(event) {
        Some(Ok(MemoryEvent::Saved(_))) => Some(MemoryId::new(event.seq)),
        Some(Ok(MemoryEvent::Retired(retired))) => Some(retired.id),
        _ => None,
    }
}
