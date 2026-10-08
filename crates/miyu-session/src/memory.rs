//! 回合索引（施工 R-2 上，`docs/blueprint/memory.md`「怎么走」第一条）：主会话每落一批，把结束了的人开的回合放进这个人格的
//! 回合库，撤销的拿掉、恢复的读回，载入时照整份事件补上落下的。撤销的每一轮另埋一块墓碑、恢复的揭掉，记忆的出处照它判
//! 活不活（施工 R-3 上，第二条第 4 款）。怎么算是纯逻辑（`miyu_recall::TurnFeed`），这里只管接线和写库。
//!
//! 回合库是派生的：更新失败记一行 `WARN memory index not updated`，会话照常；照到的位置没往前挪，下次载入照日志补。

mod keeper;
mod port;

pub use keeper::{Filter, Keeper, Stamp};
pub(crate) use port::Calls;

use std::sync::{Arc, OnceLock};

use miyu_kernel::event::Event;
use miyu_kernel::id::{AccountId, SessionId};
use miyu_policy::memory::MemoryScope;
use miyu_recall::{Change, TurnFeed, key, replay};
use miyu_store::memory::MemoryLogs;
use miyu_store::recall::{Edit, Opened, RecallIndex, RecallIndexes, Room};

use crate::TARGET;

/// 核心一份的记忆（施工 R-3 中）：回合库的登记、记忆日志的登记。会话表交给每个会话，主会话照它更新回合索引、给三件工具造
/// 端口。
#[derive(Debug)]
pub struct Memory {
    /// 回合库的登记（施工 R-2 上）。
    pub turns: Arc<RecallIndexes>,
    /// 记忆日志的登记（施工 R-3 上）。
    pub logs: Arc<MemoryLogs>,
}

/// 接上会话 `session` 的记忆：回合索引（[`Turns::connect`]）和三件工具的端口（[`Calls`]）。范围是 `off` 的（子会话、
/// `--no-memory`）不接，核心没交记忆的（测试里自己造的）没有。在阻塞线程里调。
///
/// 记忆放在哪照范围（`memory.md`「范围」）：跟着人格的在记忆账号 `account` 的那一间，只在这个会话里的在会话自己的目录
/// 里（属主 `owner` 的）。听众照属主，不照记忆账号：出厂人格的记忆归属主，系统账号开的会话归管理员，听的人还是属主。
pub(crate) fn connect(
    memory: Option<&Arc<Memory>>,
    scope: MemoryScope,
    account: &AccountId,
    owner: &AccountId,
    persona: &str,
    session: &SessionId,
    events: &[Event],
) -> (Option<Turns>, Option<Calls>) {
    let room = match scope {
        MemoryScope::Persona => Room::persona(account, persona),
        MemoryScope::Session => Room::session(owner, session),
        MemoryScope::Off => return (None, None),
    };
    let Some(memory) = memory else {
        return (None, None);
    };
    let turns = Turns::connect(&memory.turns, &room, persona, session, events);
    (turns, Some(Calls::new(memory, room, owner, session)))
}

/// 会话的记忆的范围：子会话（`child`）不管交的、快照里的是什么都是 `off`（17 第二节，以前造的子会话快照里没有这一格）；
/// 主会话照交的 `given`。
pub(crate) fn scope(child: bool, opened: bool, given: MemoryScope) -> MemoryScope {
    if child || !opened {
        MemoryScope::Off
    } else {
        given
    }
}

/// 一个会话的回合索引：它的回合库（用到才开）、增量的 `TurnFeed`。
pub(crate) struct Turns {
    /// 会话编号：键的前一半，`marks` 里的来源。
    session: SessionId,
    /// 核心的登记，和这个会话的那一间：回合库照它开。
    recall: Arc<RecallIndexes>,
    room: Room,
    /// 人格的编号：只用来记日志。
    persona: String,
    /// 开了的回合库：新造的会话到第一次真要写的时候才开，造会话不多一次开库、建表、同步。
    index: OnceLock<Arc<RecallIndex>>,
    /// 增量算。
    feed: TurnFeed,
}

impl Turns {
    /// 接上会话 `session` 的回合索引，放在 `room` 那一间。在阻塞线程里调。
    ///
    /// `events` 是载入时读到的整份事件：照它铺回 `TurnFeed`，照回合库记着的照到哪以后的补上（一条都没照过的整份补）；
    /// 新造的会话是空的，这时不开库。
    pub(crate) fn connect(
        recall: &Arc<RecallIndexes>,
        room: &Room,
        persona: &str,
        session: &SessionId,
        events: &[Event],
    ) -> Option<Turns> {
        let mut turns = Turns {
            session: session.clone(),
            recall: Arc::clone(recall),
            room: room.clone(),
            persona: persona.to_string(),
            index: OnceLock::new(),
            feed: TurnFeed::default(),
        };
        let Some(last) = events.last() else {
            return Some(turns);
        };
        let mark = turns
            .index()
            .mark(&session.to_string())
            .unwrap_or_else(|error| {
                tracing::warn!(target: TARGET, error = %error, "memory index not read");
                None
            });
        let (feed, changes) = TurnFeed::primed(events, mark);
        turns.feed = feed;
        if mark.is_none_or(|mark| mark < last.seq) {
            turns.write(changes, last, || Ok(events.to_vec()));
        }
        Some(turns)
    }

    /// 这个会话的回合库：第一次用时照登记开，这一回第一次开的记一行。
    fn index(&self) -> &RecallIndex {
        self.index.get_or_init(|| {
            let (index, opened) = self.recall.turns(&self.room);
            if let Some(opened) = opened {
                log_opened(&self.persona, &opened);
            }
            index
        })
    }

    /// 一批事件落了盘：交给 `TurnFeed`，照它交回的改回合库，连同照到了这一批的最后一条。恢复的那几轮要读整份日志
    /// （`read`，很少见）。
    pub(crate) fn advance(
        &mut self,
        events: &[Event],
        read: impl FnOnce() -> Result<Vec<Event>, String>,
    ) {
        let Some(last) = events.last() else {
            return;
        };
        let changes: Vec<Change> = events
            .iter()
            .flat_map(|event| self.feed.see(event))
            .collect();
        self.write(changes, last, read);
    }

    /// 把改动写进回合库，照到 `last`。
    fn write(
        &self,
        changes: Vec<Change>,
        last: &Event,
        read: impl FnOnce() -> Result<Vec<Event>, String>,
    ) {
        let mut edits = Vec::new();
        let mut lost = Vec::new();
        for change in changes {
            match change {
                Change::Put(item) => edits.push(Edit::Put {
                    key: key(&self.session, item.turn),
                    text: item.text,
                    at: item.at,
                }),
                // 撤销的那一轮拿掉，再埋一块墓碑：记忆的出处在它里面的照它判死了（施工 R-3 上）。
                Change::Remove(turn) => {
                    let key = key(&self.session, turn);
                    edits.push(Edit::Remove { key: key.clone() });
                    edits.push(Edit::Bury { key });
                }
                Change::Restored(turns) => {
                    edits.extend(turns.into_iter().map(|turn| Edit::Unbury {
                        key: key(&self.session, turn),
                    }))
                }
                Change::Lost(turns) => lost.extend(turns),
            }
        }
        // 这一批没有要改的不写：照到的位置落在后面不要紧，载入时多铺一截、交回的还是空的（每落一批少写一次库）。
        if edits.is_empty() && lost.is_empty() {
            return;
        }
        if !lost.is_empty() {
            match read() {
                Ok(events) => edits.extend(
                    replay(&events)
                        .into_iter()
                        .filter(|item| lost.contains(&item.turn))
                        .map(|item| Edit::Put {
                            key: key(&self.session, item.turn),
                            text: item.text,
                            at: item.at,
                        }),
                ),
                Err(error) => {
                    // 读不回的不往前挪：下次载入照日志整份补。
                    tracing::warn!(target: TARGET, error = %error, "memory index not updated");
                    return;
                }
            }
        }
        if let Err(error) = self
            .index()
            .apply(&self.session.to_string(), &edits, last.seq)
        {
            tracing::warn!(target: TARGET, error = %error, "memory index not updated");
        }
    }
}

/// 回合库这一回第一次开：新建的记 `INFO`，重建的、用不了的记 `WARN`（照会话列表的索引的说法）。
fn log_opened(persona: &str, opened: &Opened) {
    match opened {
        Opened::Kept => {}
        Opened::Created => tracing::info!(target: TARGET, persona, "memory index created"),
        Opened::Rebuilt(why) => {
            tracing::warn!(target: TARGET, persona, reason = %why, "memory index rebuilt");
        }
        Opened::Unusable(error) => {
            tracing::warn!(target: TARGET, persona, error = %error, "memory index unusable");
        }
    }
}
