//! 记忆的端口的实现（施工 R-3 中，`docs/blueprint/memory.md`「工具」、第二条、第九条）：三件工具经它记、忘、搜。
//!
//! 每个主会话一份 [`Calls`]（这一间、这个会话、听众），每次调用照它造一个端口，带上这一轮和这次调用：记下的出处是这一轮、
//! `by` 是这次调用。怎么记、怎么挑在 [`Keeper`]（施工 R-3 补挪出去，协议也用）；碰磁盘的在阻塞线程里做。听众：本机的会话
//! 是属主。

use std::sync::Arc;

use miyu_kernel::facts::Present;
use miyu_kernel::id::{AccountId, CallId, SessionId, TurnId};
use miyu_kernel::origin::{By, Person, Tool};
use miyu_kernel::session::Injection;
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_recall::{MemoryId, Source};
use miyu_store::recall::Room;
use miyu_tool::{FoundMemory, MEMORIES, MemoryPort, Pending, Refused, Remember, Searched, TURNS};

use crate::blocking::blocking;
use crate::config::TurnConfig;

use super::{Extractor, Keeper, Memory, Query, Stamp, Using};

/// 一个主会话的记忆：每次调用照它造端口。
#[derive(Clone)]
pub(crate) struct Calls {
    keeper: Keeper,
    session: SessionId,
    /// 属主：远程算向量的用量记在他名下（施工 R-5 补）；抽取的也是（施工 R-6 上）。
    owner: AccountId,
    /// 这个会话的抽取（施工 R-6 上）：闹钟、在路上的那一次。
    extractor: Arc<Extractor>,
    /// 是本机的会话（施工 R-8）：只有本机的联想。群里的听众还没接上（`memory.md` 第一条第 1 款），主人的事不往群里带。
    local: bool,
}

impl Calls {
    /// 会话 `session`（属主 `owner`）的记忆，放在 `room` 那一间；听众是属主（本机的会话）。`local` 是本机的会话（不是场所的）。
    pub(crate) fn new(
        memory: &Arc<Memory>,
        room: Room,
        owner: &AccountId,
        session: &SessionId,
        local: bool,
    ) -> Calls {
        let hearers = vec![By::Person(Person::new(owner.clone()))];
        Calls {
            keeper: Keeper::new(memory, room, hearers),
            session: session.clone(),
            owner: owner.clone(),
            extractor: Arc::default(),
            local,
        }
    }

    /// 抽取用的（施工 R-6 上）：记在哪一间的那一份、哪个会话、谁付钱、这个会话的抽取状态。
    pub(crate) fn extracting(&self) -> (&Keeper, &SessionId, &AccountId, &Arc<Extractor>) {
        (&self.keeper, &self.session, &self.owner, &self.extractor)
    }

    /// 记忆放在哪一间：交给 [`crate::Handle`]，协议照它找这个会话的记忆（施工 R-3 补）。
    pub(crate) fn room(&self) -> &Room {
        self.keeper.room()
    }

    /// 回合开始要不要交常驻的摘要、交什么（施工 R-4 上，`summary.rs`）：日期照会话的时区 `offset`，在阻塞线程里读。
    pub(crate) async fn summary(
        &self,
        offset: UtcOffset,
        present: Vec<Present>,
    ) -> Option<Injection> {
        let keeper = self.keeper.clone();
        blocking(move || keeper.summary(offset, &present)).await
    }

    /// 这一轮联想带什么（施工 R-8，`recall.rs`）：只给本机的会话；`said` 是这一轮人说的话，照意思找照这一轮的配置 `config`
    /// （`models.embedding`，远程的用量记在属主名下），日期照会话的时区 `offset`。
    pub(crate) async fn recall(
        &self,
        said: String,
        offset: UtcOffset,
        present: Vec<Present>,
        config: &TurnConfig,
    ) -> Option<Injection> {
        if !self.local {
            return None;
        }
        let using = Using {
            config: Arc::clone(config),
            owner: self.owner.clone(),
        };
        self.keeper.recall(&using, &said, offset, &present).await
    }

    /// 这一次调用的端口：第 `turn` 轮（没有在跑的回合的没有）、调用 `call_id`、派出去的时刻 `at`；照意思找照这一轮的配置
    /// `config`（`models.embedding`，施工 R-5 补）。
    pub(crate) fn port(
        &self,
        turn: Option<TurnId>,
        call_id: CallId,
        at: Timestamp,
        config: &TurnConfig,
    ) -> Arc<dyn MemoryPort> {
        let using = Using {
            config: Arc::clone(config),
            owner: self.owner.clone(),
        };
        Arc::new(Port(Arc::new(Inner {
            calls: self.clone(),
            turn,
            call_id,
            at,
            using,
        })))
    }
}

struct Port(Arc<Inner>);

struct Inner {
    calls: Calls,
    turn: Option<TurnId>,
    call_id: CallId,
    at: Timestamp,
    using: Using,
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
        Box::pin(async move {
            // 照意思找的（施工 R-5 下）：先算问句的向量（`off` 的、等不到的只走关键词），搜完在后台补这一间缺的。
            let (keeper, using) = (inner.calls.keeper.clone(), inner.using.clone());
            let near = match keeper.vectors() {
                Some(vectors) => vectors.query(&using, &query).await,
                None => None,
            };
            let found = blocking(move || inner.search(&query, forgotten, near.as_ref())).await;
            if keeper.vectors().is_some() {
                blocking(move || keeper.fill(&using)).await;
            }
            found
        })
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

    /// 出处是这一轮。记下了的在后台补它的向量（施工 R-5 五补：不等下一次搜）。
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
        let saved = self.calls.keeper.save(self.stamp(), remember, sources)?;
        self.calls.keeper.fill(&self.using);
        Ok(saved)
    }

    /// 记下的最多 [`MEMORIES`] 条在前，别的会话里以前的对话最多 [`TURNS`] 段在后。
    fn search(
        &self,
        query: &str,
        forgotten: bool,
        near: Option<&Query>,
    ) -> Result<Searched, String> {
        let keeper = &self.calls.keeper;
        let memories = keeper
            .search(query, forgotten, MEMORIES, near)?
            .into_iter()
            .map(|entry| FoundMemory {
                id: entry.id,
                class: entry.class,
                text: entry.text,
                at: entry.at,
                retired: entry.retired.is_some(),
            })
            .collect();
        let turns = keeper.turns(query, TURNS, &self.calls.session, near)?;
        Ok(Searched { memories, turns })
    }
}
