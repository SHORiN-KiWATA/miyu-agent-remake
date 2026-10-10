//! 一间记忆和这次谁在听（施工 R-3 补，`docs/blueprint/memory.md`「工具」「协议」、第二条、第九条）：记、改、忘、清空、搜、列
//! 都在这里。她的三件工具（`port.rs`）和协议（端点的 `memory.*`、`/remember`）都调它，只是 `by`、出处、听众不一样。碰记忆日志
//! 和检索库，调的一方放在阻塞线程里。
//!
//! - 听众：一条记忆的听众里有这次会被谁看到的每一个人，才算合（17 L6）；不合的当没有，不让人知道有。
//! - 出处活不活照回合库的墓碑（第二条第 4 款）：一处都没有（人记的）的算活；有的，活着一处就算。

use std::collections::BTreeMap;
use std::sync::Arc;

use miyu_kernel::id::{CommandId, ModuleId, Seq, SessionId, TurnId};
use miyu_kernel::origin::{By, Module};
use miyu_kernel::time::Timestamp;
use miyu_recall::extract::Candidate;
use miyu_recall::{
    Cleared, Entry, Extracted, MemoryEvent, MemoryId, Retired, Saved, Skipped, Source, fuse,
};
use miyu_store::memory::{MemoryLog, Report};
use miyu_store::recall::{Opened, Room};
use miyu_tool::{FoundTurn, Refused, Remember};

use crate::TARGET;

use super::Memory;
use super::vectors::{Query, Target, Using, Vectors};

/// 一间记忆，连同这次会被谁看到（听众）。可以复制：每次调用照它做。
#[derive(Debug, Clone)]
pub struct Keeper {
    pub(super) memory: Arc<Memory>,
    room: Room,
    hearers: Vec<By>,
}

/// 谁、什么时候、经哪条命令写的。
#[derive(Debug, Clone)]
pub struct Stamp {
    /// 时刻。
    pub at: Timestamp,
    /// 写它的那一方：她经工具记的是那次调用，人记的是那个人。
    pub by: By,
    /// 人经协议、斜杠命令写的，那条命令的编号：同一个编号再来只算一次，交回头一次的结果（04 第六节第 1 条）。她经工具写的
    /// 没有。
    pub cause: Option<CommandId>,
}

/// 列出的条件（协议的 `memory.list`）。
#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// 只要这一类。
    pub class: Option<String>,
    /// 只要出处在这个会话里的。
    pub from: Option<SessionId>,
    /// 作废的也要。
    pub forgotten: bool,
    /// 最多几条。
    pub limit: usize,
}

impl Keeper {
    /// 记忆 `memory` 里的 `room` 那一间，这次会被 `hearers` 看到。
    pub fn new(memory: &Arc<Memory>, room: Room, hearers: Vec<By>) -> Keeper {
        Keeper {
            memory: Arc::clone(memory),
            room,
            hearers,
        }
    }

    /// 哪一间。
    pub fn room(&self) -> &Room {
        &self.room
    }

    /// 记一条：出处 `sources`，听众是这次的听众。写了 `replaces` 的是改那一条：它得现在算数、听众合。交回新的编号；这条命令
    /// 做过的交回那一次的（记忆日志认编号）。
    ///
    /// # Errors
    ///
    /// `replaces` 没有这一条（听众不合的也当没有）、已经不算了；记忆日志开不了、写不进。
    pub fn save(
        &self,
        stamp: Stamp,
        remember: Remember,
        sources: Vec<Source>,
    ) -> Result<MemoryId, Refused> {
        let log = self.log()?;
        if let Some(old) = remember.replaces {
            self.current(&log, old)?;
        }
        let saved = Saved {
            class: remember.class,
            text: remember.text,
            sources,
            audience: self.hearers.clone(),
            replaces: remember.replaces,
            about: None,
        };
        self.append(&log, stamp, &MemoryEvent::Saved(saved))
            .map(|(id, _)| id)
    }

    /// 抽取要的几样（施工 R-6 上）：核心没交的没有，不抽。
    pub(crate) fn extraction(&self) -> Option<super::Extraction> {
        self.memory.extraction().cloned()
    }

    /// 人格记忆这时装着没有（施工 R-10）：没装的开着的会话不交摘要、不抽。
    pub(crate) fn installed(&self) -> bool {
        self.memory.installed()
    }

    /// 会话 `session` 抽到了它日志的第几条（施工 R-6 上，`memory.md` 第六条）；还没抽过的是 `None`。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了。
    pub(crate) fn extracted(&self, session: &SessionId) -> Result<Option<Seq>, Refused> {
        Ok(self.log()?.book(|book| book.extracted(session)))
    }

    /// 记下会话 `session` 抽出来的几条（施工 R-6 上）：`by` 是记忆模块，出处各是那一轮，听众是这次的听众；再记一条它抽到了
    /// 第 `upto` 条（整段跳过的带上为什么）。交回记下了几条。候选在前、记号在后：中途写不进的，下次重抽这一段。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了、写不进。
    pub(crate) fn record_extraction(
        &self,
        at: Timestamp,
        session: &SessionId,
        upto: Seq,
        found: Vec<Candidate>,
        skipped: Option<Skipped>,
    ) -> Result<u32, Refused> {
        let log = self.log()?;
        let by = By::Module(Module {
            id: ModuleId::parse(super::MODULE).expect("合模块编号的写法"),
        });
        let mut count = 0;
        for candidate in found {
            let Some(turn) = Seq::new(candidate.turn).map(TurnId::new) else {
                continue;
            };
            let saved = Saved {
                class: candidate.class,
                text: candidate.text,
                sources: vec![Source {
                    session: session.clone(),
                    turn,
                }],
                audience: self.hearers.clone(),
                replaces: None,
                about: candidate.about,
            };
            let stamp = Stamp {
                at,
                by: by.clone(),
                cause: None,
            };
            self.append(&log, stamp, &MemoryEvent::Saved(saved))?;
            count += 1;
        }
        let mark = Extracted {
            session: session.clone(),
            upto,
            count,
            skipped,
        };
        let stamp = Stamp {
            at,
            by,
            cause: None,
        };
        self.append(&log, stamp, &MemoryEvent::Extracted(mark))?;
        Ok(count)
    }

    /// 改一条（协议的 `memory.update`）：新记一条 `replaces` 它，类照旧的，出处空。
    ///
    /// # Errors
    ///
    /// 同 [`Keeper::save`]。
    pub fn update(&self, stamp: Stamp, id: MemoryId, text: String) -> Result<MemoryId, Refused> {
        let log = self.log()?;
        if let Some((done, _)) = done(&log, &stamp) {
            return Ok(done);
        }
        let class = self.current(&log, id)?.class;
        let remember = Remember {
            class,
            text,
            replaces: Some(id),
        };
        self.save(stamp, remember, Vec::new())
    }

    /// 作废一条。
    ///
    /// # Errors
    ///
    /// 没有这一条（听众不合的也当没有）、已经不算了；记忆日志开不了、写不进。
    pub fn retire(&self, stamp: Stamp, id: MemoryId, why: String) -> Result<(), Refused> {
        let log = self.log()?;
        if done(&log, &stamp).is_some() {
            return Ok(());
        }
        self.current(&log, id)?;
        self.append(&log, stamp, &MemoryEvent::Retired(Retired { id, why }))
            .map(|_| ())
    }

    /// 清空（第二条第 5 款）：`session` 是只清从这个会话来的，没有的是整间。交回清掉了几条。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了、写不进。
    pub fn clear(&self, stamp: Stamp, session: Option<SessionId>) -> Result<usize, Refused> {
        let log = self.log()?;
        self.append(&log, stamp, &MemoryEvent::Cleared(Cleared { session }))
            .map(|(_, cleared)| cleared)
    }

    /// 搜记下的，最多 `limit` 条，最相关的在前：只给现在算数的（`forgotten` 时作废的也给）、出处活着的、听众合的。照关键词找；
    /// 有问句的向量 `near` 的（施工 R-5 下）再照意思找，两路照名次合（`miyu_recall::fuse`）。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了、记忆库读不了。
    pub fn search(
        &self,
        query: &str,
        forgotten: bool,
        limit: usize,
        near: Option<&Query>,
    ) -> Result<Vec<Entry>, String> {
        let log = self.log().map_err(failed)?;
        // 多要几倍：改掉的、作废的、出处死了的、听众不合的挑掉以后还够。
        let wide = limit.saturating_mul(4);
        let ids = log.search(query, wide).map_err(|error| error.to_string())?;
        let close = match near {
            Some(near) => log
                .index()
                .nearest(&near.model, &near.vector, wide)
                .map_err(|error| error.to_string())?
                .into_iter()
                .filter_map(|hit| MemoryId::parse(&hit.key).map(|id| (id, hit.similar)))
                .collect(),
            None => Vec::new(),
        };
        let ids = fuse(&ids, &close);
        let found = log.book(|book| {
            ids.iter()
                .filter_map(|id| book.get(*id).cloned())
                .collect::<Vec<Entry>>()
        });
        Ok(found
            .into_iter()
            .filter(|entry| self.shown(entry, forgotten))
            .take(limit)
            .collect())
    }

    /// 照条件列出，新的在前：给的规矩同 [`Keeper::search`]。
    ///
    /// # Errors
    ///
    /// 记忆日志开不了。
    pub fn list(&self, filter: &Filter) -> Result<Vec<Entry>, String> {
        let log = self.log().map_err(failed)?;
        let all = log.book(|book| book.all().rev().cloned().collect::<Vec<Entry>>());
        Ok(all
            .into_iter()
            .filter(|entry| {
                filter
                    .class
                    .as_ref()
                    .is_none_or(|class| &entry.class == class)
            })
            .filter(|entry| {
                filter
                    .from
                    .as_ref()
                    .is_none_or(|from| entry.sources.iter().any(|source| &source.session == from))
            })
            .filter(|entry| self.shown(entry, filter.forgotten))
            .take(filter.limit)
            .collect())
    }

    /// 搜这一间的以前的对话（回合索引），最多 `limit` 段；`skip` 这个会话自己的不算（她看得见）。照关键词找；有问句的向量
    /// `near` 的（施工 R-5 下）再照意思找，两路照名次合。
    ///
    /// # Errors
    ///
    /// 回合库读不了。
    pub(crate) fn turns(
        &self,
        query: &str,
        limit: usize,
        skip: &SessionId,
        near: Option<&Query>,
    ) -> Result<Vec<FoundTurn>, String> {
        let (turns, _) = self.memory.turns.turns(&self.room);
        let own = format!("{skip}/");
        let wide = limit.saturating_mul(4);
        let hits = turns
            .search(query, wide)
            .map_err(|error| error.to_string())?;
        let close = match near {
            Some(near) => turns
                .nearest(&near.model, &near.vector, wide)
                .map_err(|error| error.to_string())?,
            None => Vec::new(),
        };
        let keys: Vec<String> = hits.iter().map(|hit| hit.key.clone()).collect();
        let similar: Vec<(String, f32)> = close
            .iter()
            .map(|hit| (hit.key.clone(), hit.similar))
            .collect();
        // 合完照键找回字和时刻：两路交回的是同一条的同一份。
        let mut found: BTreeMap<String, (String, Timestamp)> = close
            .into_iter()
            .map(|hit| (hit.key, (hit.text, hit.at)))
            .collect();
        found.extend(hits.into_iter().map(|hit| (hit.key, (hit.text, hit.at))));
        Ok(fuse(&keys, &similar)
            .into_iter()
            .filter(|key| !key.starts_with(&own))
            .filter_map(|key| {
                let (text, at) = found.remove(&key)?;
                let (session, turn) = key.split_once('/')?;
                let turn = turn.parse().ok().and_then(miyu_kernel::id::Seq::new)?;
                Some(FoundTurn {
                    session: SessionId::parse(session).ok()?,
                    turn: TurnId::new(turn),
                    at,
                    text,
                })
            })
            .take(limit)
            .collect())
    }

    /// 照意思找的那一路照 `using` 在后台补这一间缺的向量（施工 R-5 下，`vectors.rs`）：搜的时候起。没接向量的、`off` 的什么
    /// 都不做。
    pub fn fill(&self, using: &Using) {
        let Some(vectors) = self.memory.vectors() else {
            return;
        };
        if let Ok(log) = self.log() {
            vectors.fill(
                using,
                format!("memories {:?}", self.room),
                Target::Memories(log),
            );
        }
        let (turns, _) = self.memory.turns.turns(&self.room);
        vectors.fill(
            using,
            format!("turns {:?}", self.room),
            Target::Turns(turns),
        );
    }

    /// 照意思找的那一路（施工 R-5 下）：核心没接的没有。
    pub fn vectors(&self) -> Option<&Arc<Vectors>> {
        self.memory.vectors()
    }

    /// 这一条给不给看：没改掉、没清掉（`forgotten` 时作废的也给）、听众合、出处活着。
    pub(super) fn shown(&self, entry: &Entry, forgotten: bool) -> bool {
        entry.replaced_by.is_none()
            && !entry.cleared
            && (forgotten || entry.retired.is_none())
            && self.hears(entry)
            && self.alive(entry)
    }

    /// 这一间的记忆日志；这一回第一次开的记一行派生的情形。
    pub(super) fn log(&self) -> Result<Arc<MemoryLog>, Refused> {
        let (log, report) = self
            .memory
            .logs
            .open(&self.room)
            .map_err(|error| Refused::Failed(error.to_string()))?;
        if let Some(report) = report {
            log_report(&self.room, &report);
        }
        Ok(log)
    }

    /// 编号 `id` 的那一条现在算数、听众合，交回它：没有的、听众不合的当没有；改掉的、作废的、清掉的说已经不算了。
    fn current(&self, log: &MemoryLog, id: MemoryId) -> Result<Entry, Refused> {
        match log.book(|book| book.get(id).cloned()) {
            Some(entry) if self.hears(&entry) => {
                if entry.current() {
                    Ok(entry)
                } else {
                    Err(Refused::NotCurrent(id))
                }
            }
            _ => Err(Refused::NoSuch(id)),
        }
    }

    /// 追加一条，交回编号和清掉了几条；记忆库没写上的记一行，不挡。
    pub(super) fn append(
        &self,
        log: &MemoryLog,
        stamp: Stamp,
        event: &MemoryEvent,
    ) -> Result<(MemoryId, usize), Refused> {
        let appended = log
            .append(stamp.at, stamp.by, stamp.cause.as_ref(), event)
            .map_err(|error| Refused::Failed(error.to_string()))?;
        if let Err(error) = appended.indexed {
            tracing::warn!(target: TARGET, error = %error, "memory index not updated");
        }
        Ok((appended.id, appended.cleared))
    }

    /// 这次会被谁看到的人，都在这一条的听众里。
    fn hears(&self, entry: &Entry) -> bool {
        self.hearers.iter().all(|hearer| {
            entry
                .audience
                .iter()
                .any(|heard| same_person(hearer, heard))
        })
    }

    /// 出处还活着：一处都没有的算活；有的，活着一处就算。回合库读不了的当活的：宁可多想起来，不吞掉人说过的。
    fn alive(&self, entry: &Entry) -> bool {
        entry.sources.is_empty()
            || entry
                .sources
                .iter()
                .any(|source| self.memory.turns.alive(&self.room, source).unwrap_or(true))
    }
}

/// 这条命令做过的（`stamp.cause`）：那一条的编号、清掉几条。先看它，再查参数：改一条、作废一条重发时，那一条已经不算了。
fn done(log: &MemoryLog, stamp: &Stamp) -> Option<(MemoryId, usize)> {
    let cause = stamp.cause.as_ref()?;
    log.book(|book| book.done(cause))
}

/// 开不了日志说成一句。
fn failed(refused: Refused) -> String {
    match refused {
        Refused::Failed(why) => why,
        other => format!("{other:?}"),
    }
}

/// 记忆日志这一回第一次开：派生的那些是什么情形，照会话列表的索引的说法记。
fn log_report(room: &Room, report: &Report) {
    let room = room.to_string();
    let room = room.as_str();
    match &report.index {
        Opened::Kept => {}
        Opened::Created => tracing::info!(target: TARGET, room, "memory log index created"),
        Opened::Rebuilt(why) => {
            tracing::warn!(target: TARGET, room, reason = %why, "memory log index rebuilt")
        }
        Opened::Unusable(error) => {
            tracing::warn!(target: TARGET, room, error = %error, "memory log index unusable")
        }
    }
    if let Some(error) = &report.caught_up {
        tracing::warn!(target: TARGET, room, error = %error, "memory log index not caught up");
    }
    for (seq, why) in &report.unreadable {
        tracing::warn!(target: TARGET, room, seq = seq.get(), why = why.as_str(), "memory event unreadable");
    }
    if report.filled > 0 {
        tracing::info!(target: TARGET, room, filled = report.filled, "memory log index caught up");
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
