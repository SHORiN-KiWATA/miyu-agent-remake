//! 记忆日志的登记（施工 R-3 上，`docs/blueprint/memory.md`「对外的样子」的记忆日志、底账、记忆库）：一个账号、一个人格一份
//! 记忆日志，`home/<账号>/modules/memory/<人格>/`，照会话日志的外壳和写法（[`SessionLog`]：崩了截掉半行、同步、按段）。
//!
//! - 几个会话同时往一份里写：核心一份登记，一份日志一把锁，序号在锁里领，所以连续不重。
//! - 底账（`miyu_recall::MemoryBook`）跟着日志在内存里，打开时照日志算一遍：一个人格的记忆几百到几千条，不多背。
//! - 记忆库是派生的检索库（`index/recall/memory-<人格>.db`，键是编号，照到的来源是 `log`）：追加时顺手写，写不进不挡住
//!   追加，交回给调的一方记日志（[`Appended::indexed`]）；打开时照到的位置比日志短就照日志补。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use miyu_kernel::event::Event;
use miyu_kernel::id::{AccountId, Seq};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use miyu_recall::{MemoryBook, MemoryEvent, MemoryId, from_event, to_event};

use crate::log::{OpenError, SEGMENT_LIMIT, SessionLog};
use crate::recall::{DbError, Edit, Opened, RecallIndex};
use crate::root::DataRoot;

/// 记忆库里照到的来源：整份记忆日志只有这一个。
const SOURCE: &str = "log";

/// 记忆日志的登记：照（账号、人格）开、留着。核心里一份。
#[derive(Debug)]
pub struct MemoryLogs {
    /// 数据根。
    root: DataRoot,
    /// 开过的。
    open: Mutex<BTreeMap<(AccountId, String), Arc<MemoryLog>>>,
}

/// 一份记忆日志，连同它的底账、记忆库。
#[derive(Debug)]
pub struct MemoryLog {
    /// 日志和底账在一把锁里：序号在锁里领，底账跟着日志一条条算。
    inner: Mutex<Inner>,
    /// 记忆库：派生的。
    index: RecallIndex,
}

#[derive(Debug)]
struct Inner {
    log: SessionLog,
    book: MemoryBook,
}

/// 这一回第一次开一份记忆日志时，派生的那些是什么情形：调的一方照它记运行日志。
#[derive(Debug)]
pub struct Report {
    /// 记忆库开得怎么样。
    pub index: Opened,
    /// 照日志补了几条进记忆库：照到的位置跟得上日志的是 0。
    pub filled: usize,
    /// 照日志补记忆库出了错（没补上的下次再补）。
    pub caught_up: Option<DbError>,
    /// 日志里读不懂的记忆事件（底账跳过它们）：哪一条、为什么。
    pub unreadable: Vec<(Seq, String)>,
}

/// 追加了一条。
#[derive(Debug)]
pub struct Appended {
    /// 它的编号。
    pub id: MemoryId,
    /// 记忆库写上了没有：派生的，没写上不挡住追加，下次打开时照日志补。
    pub indexed: Result<(), DbError>,
}

/// 记忆日志出错。
#[derive(Debug)]
pub enum MemoryError {
    /// 日志打不开：坏在中间、读写出错（[`OpenError`]）。
    Open(OpenError),
    /// 写不进日志。
    Write(std::io::Error),
    /// 事件写不成（不会发生，如实交回）。
    Encode(String),
}

impl MemoryLogs {
    /// 一份空的登记，用到哪一份才开哪一份。
    pub fn new(root: &DataRoot) -> MemoryLogs {
        MemoryLogs {
            root: root.clone(),
            open: Mutex::new(BTreeMap::new()),
        }
    }

    /// 账号 `account`、人格 `persona` 的记忆日志：开过的交回同一份；没有的建一份空的。这一回第一次开的另交回派生的那些是
    /// 什么情形（[`Report`]）。
    ///
    /// # Errors
    ///
    /// 日志坏在中间、读写出错：这一份这一回用不了，下次再开再试。
    pub fn open(
        &self,
        account: &AccountId,
        persona: &str,
    ) -> Result<(Arc<MemoryLog>, Option<Report>), MemoryError> {
        let mut open = self.lock();
        let slot = (account.clone(), persona.to_string());
        if let Some(log) = open.get(&slot) {
            return Ok((Arc::clone(log), None));
        }
        let dir = self
            .root
            .account_dir(account)
            .join("modules")
            .join("memory")
            .join(persona);
        let (log, events) = match SessionLog::open(&dir, SEGMENT_LIMIT) {
            Ok(opened) => opened,
            Err(OpenError::Missing(_)) => (
                SessionLog::create(&dir, SEGMENT_LIMIT).map_err(MemoryError::Write)?,
                Vec::new(),
            ),
            Err(error) => return Err(MemoryError::Open(error)),
        };
        let mut book = MemoryBook::default();
        let mut unreadable = Vec::new();
        for event in &events {
            if let Err(why) = book.see(event) {
                unreadable.push((event.seq, why));
            }
        }
        let (index, opened) = RecallIndex::open(&self.index_path(account, persona));
        let (filled, caught_up) = match catch_up(&index, &events) {
            Ok(filled) => (filled, None),
            Err(error) => (0, Some(error)),
        };
        let log = Arc::new(MemoryLog {
            inner: Mutex::new(Inner { log, book }),
            index,
        });
        open.insert(slot, Arc::clone(&log));
        Ok((
            log,
            Some(Report {
                index: opened,
                filled,
                caught_up,
                unreadable,
            }),
        ))
    }

    /// 记忆库在哪。
    fn index_path(&self, account: &AccountId, persona: &str) -> PathBuf {
        self.root
            .index(account)
            .join("recall")
            .join(format!("memory-{persona}.db"))
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<(AccountId, String), Arc<MemoryLog>>> {
        self.open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl MemoryLog {
    /// 追加一条：时刻 `at`，`by` 写它的那一方。先落盘，再算底账、写记忆库。
    ///
    /// # Errors
    ///
    /// 写不进日志：这一条没记下。
    pub fn append(
        &self,
        at: Timestamp,
        by: By,
        event: &MemoryEvent,
    ) -> Result<Appended, MemoryError> {
        let mut inner = self.lock();
        let seq = inner.log.next_seq();
        let line = to_event(seq, at, by, event).map_err(MemoryError::Encode)?;
        inner
            .log
            .append(std::slice::from_ref(&line))
            .map_err(MemoryError::Write)?;
        // 刚写成的一定读得懂：读不懂的是 bug，照样不挡住，底账照旧。
        if let Err(why) = inner.book.see(&line) {
            return Err(MemoryError::Encode(why));
        }
        let indexed = match event {
            MemoryEvent::Saved(saved) => self.index.apply(
                SOURCE,
                &[Edit::Put {
                    key: MemoryId::new(seq).to_string(),
                    text: saved.text.clone(),
                    at,
                }],
                seq,
            ),
            // 作废的不用改记忆库：搜得到，挑不挑是用的一方照底账判。
            MemoryEvent::Retired(_) => Ok(()),
        };
        Ok(Appended {
            id: MemoryId::new(seq),
            indexed,
        })
    }

    /// 照底账读：`read` 拿着锁跑，别做慢的事。
    pub fn book<R>(&self, read: impl FnOnce(&MemoryBook) -> R) -> R {
        read(&self.lock().book)
    }

    /// 在记忆库里照关键词找 `text`，最多 `limit` 条，最相关的在前。改掉的、作废的照样在里面，挑不挑是用的一方照底账判。
    ///
    /// # Errors
    ///
    /// 记忆库读不了。
    pub fn search(&self, text: &str, limit: usize) -> Result<Vec<MemoryId>, DbError> {
        Ok(self
            .index
            .search(text, limit)?
            .into_iter()
            .filter_map(|hit| MemoryId::parse(&hit.key))
            .collect())
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// 照日志补记忆库：照到的位置以后记下的，放进去，照到最后一条。交回补了几条；跟得上的补 0 条（只挪一下照到哪，打开时
/// 一次）。
fn catch_up(index: &RecallIndex, events: &[Event]) -> Result<usize, DbError> {
    let Some(last) = events.last() else {
        return Ok(0);
    };
    let mark = index.mark(SOURCE)?;
    let edits: Vec<Edit> = events
        .iter()
        .filter(|event| mark.is_none_or(|mark| event.seq > mark))
        .filter_map(|event| match from_event(event) {
            Some(Ok(MemoryEvent::Saved(saved))) => Some(Edit::Put {
                key: MemoryId::new(event.seq).to_string(),
                text: saved.text,
                at: event.at,
            }),
            _ => None,
        })
        .collect();
    index.apply(SOURCE, &edits, last.seq)?;
    Ok(edits.len())
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryError::Open(error) => error.fmt(f),
            MemoryError::Write(error) => error.fmt(f),
            MemoryError::Encode(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for MemoryError {}
