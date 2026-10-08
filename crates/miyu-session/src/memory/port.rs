//! 记忆的端口的实现（施工 R-3 中，`docs/blueprint/memory.md`「工具」、第二条、第九条）：三件工具经它记、忘、搜。
//!
//! 每个主会话一份 [`Calls`]（这一间、这个会话、听众），每次调用照它造一个端口，带上这一轮和这次调用：记下的出处是这一轮、
//! `by` 是这次调用。怎么记、怎么挑在 [`Keeper`]（施工 R-3 补挪出去，协议也用）；碰磁盘的在阻塞线程里做。听众：本机的会话
//! 是属主。

use std::sync::Arc;

use miyu_kernel::id::{AccountId, CallId, SessionId, TurnId};
use miyu_kernel::origin::{By, Person, Tool};
use miyu_kernel::time::Timestamp;
use miyu_recall::{MemoryId, Source};
use miyu_store::recall::Room;
use miyu_tool::{FoundMemory, MEMORIES, MemoryPort, Pending, Refused, Remember, Searched, TURNS};

use crate::blocking::blocking;

use super::{Keeper, Memory, Stamp};

/// 一个主会话的记忆：每次调用照它造端口。
#[derive(Clone)]
pub(crate) struct Calls {
    keeper: Keeper,
    session: SessionId,
}

impl Calls {
    /// 会话 `session`（属主 `owner`）的记忆，放在 `room` 那一间；听众是属主（本机的会话）。
    pub(crate) fn new(
        memory: &Arc<Memory>,
        room: Room,
        owner: &AccountId,
        session: &SessionId,
    ) -> Calls {
        let hearers = vec![By::Person(Person::new(owner.clone()))];
        Calls {
            keeper: Keeper::new(memory, room, hearers),
            session: session.clone(),
        }
    }

    /// 记忆放在哪一间：交给 [`crate::Handle`]，协议照它找这个会话的记忆（施工 R-3 补）。
    pub(crate) fn room(&self) -> &Room {
        self.keeper.room()
    }

    /// 这一次调用的端口：第 `turn` 轮（没有在跑的回合的没有）、调用 `call_id`、派出去的时刻 `at`。
    pub(crate) fn port(
        &self,
        turn: Option<TurnId>,
        call_id: CallId,
        at: Timestamp,
    ) -> Arc<dyn MemoryPort> {
        Arc::new(Port(Arc::new(Inner {
            calls: self.clone(),
            turn,
            call_id,
            at,
        })))
    }
}

struct Port(Arc<Inner>);

struct Inner {
    calls: Calls,
    turn: Option<TurnId>,
    call_id: CallId,
    at: Timestamp,
}

impl MemoryPort for Port {
    fn save<'a>(&'a self, remember: Remember) -> Pending<'a, Result<MemoryId, Refused>> {
        let inner = Arc::clone(&self.0);
        Box::pin(async move { blocking(move || inner.save(remember)).await })
    }

    fn retire<'a>(&'a self, id: MemoryId, why: String) -> Pending<'a, Result<(), Refused>> {
        let inner = Arc::clone(&self.0);
        Box::pin(async move {
            blocking(move || inner.calls.keeper.retire(inner.stamp(), id, why)).await
        })
    }

    fn search<'a>(
        &'a self,
        query: String,
        forgotten: bool,
    ) -> Pending<'a, Result<Searched, String>> {
        let inner = Arc::clone(&self.0);
        Box::pin(async move { blocking(move || inner.search(&query, forgotten)).await })
    }
}

impl Inner {
    /// `by` 是这次调用，没有命令编号：重跑一次工具调用是内核的事。
    fn stamp(&self) -> Stamp {
        Stamp {
            at: self.at,
            by: By::Tool(Tool {
                call_id: self.call_id,
            }),
            cause: None,
        }
    }

    /// 出处是这一轮。
    fn save(&self, remember: Remember) -> Result<MemoryId, Refused> {
        let sources = self
            .turn
            .map(|turn| {
                vec![Source {
                    session: self.calls.session.clone(),
                    turn,
                }]
            })
            .unwrap_or_default();
        self.calls.keeper.save(self.stamp(), remember, sources)
    }

    /// 记下的最多 [`MEMORIES`] 条在前，别的会话里以前的对话最多 [`TURNS`] 段在后。
    fn search(&self, query: &str, forgotten: bool) -> Result<Searched, String> {
        let keeper = &self.calls.keeper;
        let memories = keeper
            .search(query, forgotten, MEMORIES)?
            .into_iter()
            .map(|entry| FoundMemory {
                id: entry.id,
                class: entry.class,
                text: entry.text,
                at: entry.at,
                retired: entry.retired.is_some(),
            })
            .collect();
        let turns = keeper.turns(query, TURNS, &self.calls.session)?;
        Ok(Searched { memories, turns })
    }
}
