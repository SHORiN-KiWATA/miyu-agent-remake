//! 记忆的端口的实现（施工 R-3 中，`docs/blueprint/memory.md`「工具」、第二条、第九条）：三件工具经它记、忘、搜。
//!
//! 每个主会话一份 [`Calls`]（属主、人格、会话、听众），每次调用照它造一个端口，带上这一轮和这次调用：记下的出处是这一轮、
//! `by` 是这次调用。碰记忆日志、检索库的都在阻塞线程里做。
//!
//! - 听众：本机的会话是属主。一条记忆的听众里有这次回复会被谁看到的每一个人，才算合（17 L6）；不合的当没有，不让她知道有。
//! - 出处活不活照回合库的墓碑（第二条第 4 款）：一处都没有（人在界面上记的）的算活；有的，活着一处就算。

use std::sync::Arc;

use miyu_kernel::id::{AccountId, CallId, SessionId, TurnId};
use miyu_kernel::origin::{By, Person, Tool};
use miyu_kernel::time::Timestamp;
use miyu_recall::{Entry, MemoryEvent, MemoryId, Retired, Saved, Source};
use miyu_store::memory::{MemoryLog, Report};
use miyu_store::recall::{Opened, Room};
use miyu_tool::{
    FoundMemory, FoundTurn, MEMORIES, MemoryPort, Pending, Refused, Remember, Searched, TURNS,
};

use crate::TARGET;
use crate::blocking::blocking;

use super::Memory;

/// 一个主会话的记忆：每次调用照它造端口。
#[derive(Clone)]
pub(crate) struct Calls {
    memory: Arc<Memory>,
    /// 记忆放在哪一间（施工 R-3 下）。
    room: Room,
    /// 人格的编号：只用来记日志。
    persona: String,
    session: SessionId,
    hearers: Vec<By>,
}

impl Calls {
    /// 会话 `session`（属主 `owner`、人格 `persona`）的记忆，放在 `room` 那一间；听众是属主（本机的会话）。
    pub(crate) fn new(
        memory: &Arc<Memory>,
        room: Room,
        owner: &AccountId,
        persona: &str,
        session: &SessionId,
    ) -> Calls {
        Calls {
            memory: Arc::clone(memory),
            room,
            persona: persona.to_string(),
            session: session.clone(),
            hearers: vec![By::Person(Person::new(owner.clone()))],
        }
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
        Box::pin(async move { blocking(move || inner.retire(id, why)).await })
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
    fn save(&self, remember: Remember) -> Result<MemoryId, Refused> {
        let log = self.log()?;
        if let Some(old) = remember.replaces {
            self.current(&log, old)?;
        }
        let saved = Saved {
            class: remember.class,
            text: remember.text,
            sources: self
                .turn
                .map(|turn| {
                    vec![Source {
                        session: self.calls.session.clone(),
                        turn,
                    }]
                })
                .unwrap_or_default(),
            audience: self.calls.hearers.clone(),
            replaces: remember.replaces,
            about: None,
        };
        self.append(&log, &MemoryEvent::Saved(saved))
    }

    fn retire(&self, id: MemoryId, why: String) -> Result<(), Refused> {
        let log = self.log()?;
        self.current(&log, id)?;
        self.append(&log, &MemoryEvent::Retired(Retired { id, why }))
            .map(|_| ())
    }

    fn search(&self, query: &str, forgotten: bool) -> Result<Searched, String> {
        let log = self.log().map_err(|refused| match refused {
            Refused::Failed(why) => why,
            other => format!("{other:?}"),
        })?;
        // 多要几倍：改掉的、作废的、出处死了的、听众不合的挑掉以后还够。
        let ids = log
            .search(query, MEMORIES * 4)
            .map_err(|error| error.to_string())?;
        let memories = log.book(|book| {
            ids.iter()
                .filter_map(|id| book.get(*id).cloned())
                .filter(|entry| {
                    entry.replaced_by.is_none() && (forgotten || entry.retired.is_none())
                })
                .collect::<Vec<Entry>>()
        });
        let memories = memories
            .into_iter()
            .filter(|entry| self.hears(entry) && self.alive(entry))
            .take(MEMORIES)
            .map(|entry| FoundMemory {
                id: entry.id,
                class: entry.class,
                text: entry.text,
                at: entry.at,
                retired: entry.retired.is_some(),
            })
            .collect();
        let (turns, _) = self.calls.memory.turns.turns(&self.calls.room);
        let own = format!("{}/", self.calls.session);
        let turns = turns
            .search(query, TURNS * 4)
            .map_err(|error| error.to_string())?
            .into_iter()
            .filter(|hit| !hit.key.starts_with(&own))
            .filter_map(|hit| {
                let (session, turn) = hit.key.split_once('/')?;
                let turn = turn.parse().ok().and_then(miyu_kernel::id::Seq::new)?;
                Some(FoundTurn {
                    session: SessionId::parse(session).ok()?,
                    turn: TurnId::new(turn),
                    at: hit.at,
                    text: hit.text,
                })
            })
            .take(TURNS)
            .collect();
        Ok(Searched { memories, turns })
    }

    /// 这一间的记忆日志；这一回第一次开的记一行派生的情形。
    fn log(&self) -> Result<Arc<MemoryLog>, Refused> {
        let (log, report) = self
            .calls
            .memory
            .logs
            .open(&self.calls.room)
            .map_err(|error| Refused::Failed(error.to_string()))?;
        if let Some(report) = report {
            log_report(&self.calls.persona, &report);
        }
        Ok(log)
    }

    /// 编号 `id` 的那一条现在算数、听众合：没有的、听众不合的当没有；改掉的、作废的说已经不算了。
    fn current(&self, log: &MemoryLog, id: MemoryId) -> Result<(), Refused> {
        match log.book(|book| book.get(id).cloned()) {
            Some(entry) if self.hears(&entry) => {
                if entry.current() {
                    Ok(())
                } else {
                    Err(Refused::NotCurrent(id))
                }
            }
            _ => Err(Refused::NoSuch(id)),
        }
    }

    /// 追加一条，`by` 是这次调用；记忆库没写上的记一行，不挡。
    fn append(&self, log: &MemoryLog, event: &MemoryEvent) -> Result<MemoryId, Refused> {
        let by = By::Tool(Tool {
            call_id: self.call_id,
        });
        let appended = log
            .append(self.at, by, event)
            .map_err(|error| Refused::Failed(error.to_string()))?;
        if let Err(error) = appended.indexed {
            tracing::warn!(target: TARGET, error = %error, "memory index not updated");
        }
        Ok(appended.id)
    }

    /// 这次回复会被谁看到的人，都在这一条的听众里。
    fn hears(&self, entry: &Entry) -> bool {
        self.calls.hearers.iter().all(|hearer| {
            entry
                .audience
                .iter()
                .any(|heard| same_person(hearer, heard))
        })
    }

    /// 出处还活着：一处都没有的算活；有的，活着一处就算。回合库读不了的当活的：宁可多想起来，不吞掉人说过的。
    fn alive(&self, entry: &Entry) -> bool {
        entry.sources.is_empty()
            || entry.sources.iter().any(|source| {
                self.calls
                    .memory
                    .turns
                    .alive(&self.calls.room, source)
                    .unwrap_or(true)
            })
    }
}

/// 记忆日志这一回第一次开：派生的那些是什么情形，照会话列表的索引的说法记。
fn log_report(persona: &str, report: &Report) {
    match &report.index {
        Opened::Kept => {}
        Opened::Created => tracing::info!(target: TARGET, persona, "memory log index created"),
        Opened::Rebuilt(why) => {
            tracing::warn!(target: TARGET, persona, reason = %why, "memory log index rebuilt")
        }
        Opened::Unusable(error) => {
            tracing::warn!(target: TARGET, persona, error = %error, "memory log index unusable")
        }
    }
    if let Some(error) = &report.caught_up {
        tracing::warn!(target: TARGET, persona, error = %error, "memory log index not caught up");
    }
    for (seq, why) in &report.unreadable {
        tracing::warn!(target: TARGET, persona, seq = seq.get(), why = why.as_str(), "memory event unreadable");
    }
    if report.filled > 0 {
        tracing::info!(target: TARGET, persona, filled = report.filled, "memory log index caught up");
    }
}

/// 是不是同一个人：有账号的人照账号比（主人在平台私聊里说的带着 `via`，还是这个账号本人，施工 O-3），平台上的人照平台身份
/// 比；别的「谁」不是人，照原样比。
fn same_person(a: &By, b: &By) -> bool {
    match (a, b) {
        (By::Person(a), By::Person(b)) => a.account == b.account,
        (By::External(a), By::External(b)) => a.id == b.id,
        _ => a == b,
    }
}
