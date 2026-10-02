//! 往用量汇总里写、补（施工 8-15，`docs/blueprint/models.md`「怎么走」第九条第 4、5 条）。照会话列表的索引（施工 3-8 七补）：
//! 行本身重复写不出两行，先写不丢；记到的位置只在正好接得上时往前挪，落后的停在原处，等补的时候读多出来的一截。
//!
//! - 会话每落一批（[`UsageIndex::advance`]）：写这一批里发出去了的请求；这一批从第 1 条起的新起一个位置，别的位置正好是
//!   这一批之前的才挪到这一批之后。
//! - 补（[`UsageIndex::catch_up`]）：先读账号日志记到以后的几条（`usage.purged` 换掉那个会话的单次行、标上 `purged`；
//!   `usage.oneshot` 写一行），再看管理员的会话和回收处里的会话：标了 `purged` 的不读，记到的照 `read_marked` 读多出来的，
//!   没记过的、对不上的整份读。日志读不下去的照读到的写，位置不挪，报给调的一方记一行。
//! - 一次性调用（[`UsageIndex::one_shot`]）：先追加进账号日志，再写一行（编号是账号日志里那一条的序号）。
//!
//! 读日志、读账号日志不拿着库的锁：会话照常往里写，补的那一截写的时候再核对位置。

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{AccountId, EventKind, Seq, SessionId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::raw::RawJson;
use miyu_kernel::time::Timestamp;

use super::purged::Purged;
use super::spent::{OneShotCall, Spent, Who, insert};
use super::{ONESHOT, PURGED, UsageError, UsageIndex, journal_source};
use crate::journal::{self, Entry};
use crate::log::{Mark, OpenError, read_marked};
use crate::root::DataRoot;
use crate::sqlite::integer;

/// 补的时候哪一份没读完：会话编号（或者 `journal:<账号>`）和为什么，调的一方照它记 `WARN usage not indexed`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// 哪一份。
    pub source: String,
    /// 为什么，英文。
    pub error: String,
}

/// `marks` 里记着的一份。
struct Marked {
    who: Who,
    mark: Mark,
    purged: bool,
}

impl UsageIndex {
    /// 会话 `id`（属主等照 `who`）落了盘的一批 `events`：写之前日志在 `before`，写完在 `after`（施工 8-15，
    /// `session/actor.md` 第 5 条）。见模块的说明。用不了的（打不开）什么都不写。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn advance(
        &self,
        id: &SessionId,
        who: &Who,
        before: &Mark,
        events: &[Event],
        after: &Mark,
    ) -> Result<(), UsageError> {
        let rows: Vec<Spent> = events
            .iter()
            .filter_map(|event| Spent::called(id, who, event))
            .collect();
        let created = before.next == Seq::FIRST
            && events
                .first()
                .is_some_and(|event| matches!(event.body, Body::SessionCreated(_)));
        let mut db = self.lock();
        let Some(db) = db.as_mut() else {
            return Ok(());
        };
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        insert(&tx, &rows)?;
        match created {
            true => put_mark(&tx, id.as_str(), who, after, None)?,
            false => move_mark(&tx, id.as_str(), before, after)?,
        }
        tx.commit()?;
        Ok(())
    }

    /// 补：账号 `accounts` 的账号日志、会话、回收处里的会话（见模块的说明）。交回没读完的几份。用不了的什么都不做。
    ///
    /// # Errors
    ///
    /// 读写库出错；列不了放会话的目录。
    pub fn catch_up(&self, accounts: &[AccountId]) -> Result<Vec<Problem>, UsageError> {
        let mut problems = Vec::new();
        for account in accounts {
            self.catch_up_journal(account, &mut problems)?;
            for (id, dir) in session_dirs(&self.root, account)? {
                self.catch_up_dir(&id, &dir, &mut problems)?;
            }
        }
        Ok(problems)
    }

    /// 只补账号 `account` 的会话 `id`（`session_usage` 查这个会话之前）。交回没读完的。
    ///
    /// # Errors
    ///
    /// 读写库出错。
    pub fn catch_up_session(
        &self,
        account: &AccountId,
        id: &SessionId,
    ) -> Result<Vec<Problem>, UsageError> {
        let mut problems = Vec::new();
        let dir = self.root.session_dir(account, id);
        self.catch_up_dir(id, &dir, &mut problems)?;
        Ok(problems)
    }

    /// 一次性调用（施工 8-15，「怎么走」第九条第 4 条）：账号 `owner` 在 `at` 发出去的一次 `call`，先追加进他的账号日志
    /// （`usage.oneshot`），再写一行，编号是那一条的序号。
    ///
    /// # Errors
    ///
    /// 写不进账号日志（这时不写行：真相没有，汇总也不该有）；写不进库。
    pub fn one_shot(
        &self,
        owner: &AccountId,
        at: Timestamp,
        call: &OneShotCall,
    ) -> Result<(), UsageError> {
        let body = serde_json::to_string(call)
            .and_then(|text| serde_json::from_str::<RawJson>(&text))
            .map_err(|error| UsageError::Bad(error.to_string()))?;
        let entry = Entry {
            at,
            by: By::Kernel,
            cause: None,
            kind: kind(ONESHOT)?,
            body,
        };
        let path = self.root.account_dir(owner).join(journal::FILE);
        let seq = journal::append(&path, entry)?;
        let row = Spent::one_shot(owner, seq.get(), at, call);
        if let Some(db) = self.lock().as_ref() {
            insert(db, &[row])?;
        }
        Ok(())
    }

    /// 补账号 `account` 的日志：从记到的字节往后读，对不上的从头读。
    fn catch_up_journal(
        &self,
        account: &AccountId,
        problems: &mut Vec<Problem>,
    ) -> Result<(), UsageError> {
        let source = journal_source(account.as_str());
        let path = self.root.account_dir(account).join(journal::FILE);
        let marked = match self.lock().as_ref() {
            Some(db) => marked(db, &source)?,
            None => return Ok(()),
        };
        let from = marked.as_ref().map_or(0, |marked| marked.mark.bytes);
        let (entries, end, from) = match journal::read_from(&path, from)? {
            Some((entries, end)) => (entries, end, from),
            None => match journal::read_from(&path, 0)? {
                Some((entries, end)) => (entries, end, 0),
                None => return Ok(()),
            },
        };
        if marked.is_some() && entries.is_empty() && from == end {
            return Ok(());
        }
        let mut db = self.lock();
        let Some(db) = db.as_mut() else {
            return Ok(());
        };
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for entry in &entries {
            if let Err(why) = take(&tx, account, entry) {
                problems.push(Problem {
                    source: source.clone(),
                    error: format!("entry {}: {why}", entry.seq),
                });
            }
        }
        // 账号日志只记读到第几个字节；场所、父会话、段都没有。
        tx.execute(
            "INSERT OR REPLACE INTO marks (source, owner, venue, parent, segment, bytes, next, purged) \
             VALUES (?1, ?2, NULL, NULL, 0, ?3, 1, 0)",
            params![source, account.as_str(), integer(end, "mark")?],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// 补会话 `id`（日志在 `dir`）：见模块的说明。
    fn catch_up_dir(
        &self,
        id: &SessionId,
        dir: &Path,
        problems: &mut Vec<Problem>,
    ) -> Result<(), UsageError> {
        let marked = match self.lock().as_ref() {
            Some(db) => marked(db, id.as_str())?,
            None => return Ok(()),
        };
        if marked.as_ref().is_some_and(|marked| marked.purged) {
            return Ok(());
        }
        let mut events = Vec::new();
        let mut read = |from: Option<&Mark>| {
            events.clear();
            read_marked(dir, from, |segment| {
                events.extend(segment);
                true
            })
        };
        let mut whole = marked.is_none();
        let mut result = read(marked.as_ref().map(|marked| &marked.mark));
        if matches!(result, Ok(None)) {
            whole = true;
            result = read(None);
        }
        let end = match result {
            Ok(Some(end)) => Some(end),
            Ok(None) => None,
            // 目录里一段日志都没有：没有用量，不用记。
            Err(OpenError::Missing(_)) => return Ok(()),
            Err(error) => {
                problems.push(Problem {
                    source: id.as_str().to_string(),
                    error: error.to_string(),
                });
                None
            }
        };
        let who = match (&marked, whole) {
            (Some(marked), false) => marked.who.clone(),
            _ => match Who::first(&events) {
                Some(who) => who,
                None => {
                    if !events.is_empty() {
                        problems.push(Problem {
                            source: id.as_str().to_string(),
                            error: "the log does not start with session.created".to_string(),
                        });
                    }
                    return Ok(());
                }
            },
        };
        if !whole && end.as_ref() == marked.as_ref().map(|marked| &marked.mark) {
            return Ok(());
        }
        let rows: Vec<Spent> = events
            .iter()
            .filter_map(|event| Spent::called(id, &who, event))
            .collect();
        let mut db = self.lock();
        let Some(db) = db.as_mut() else {
            return Ok(());
        };
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // 读的时候账号日志里的 `usage.purged` 已经换过它的：不再写，免得加两遍。
        if self::marked(&tx, id.as_str())?.is_some_and(|now| now.purged) {
            return Ok(());
        }
        insert(&tx, &rows)?;
        if let Some(end) = end {
            match &marked {
                None => put_mark(&tx, id.as_str(), &who, &end, None)?,
                Some(was) => put_mark(&tx, id.as_str(), &who, &end, Some(&was.mark))?,
            }
        }
        tx.commit()?;
        Ok(())
    }
}

/// 账号日志里的一条：`usage.oneshot` 写一行，`usage.purged` 换掉那个会话的；别的种类不看。
fn take(db: &Connection, account: &AccountId, entry: &Event) -> Result<(), String> {
    let Body::Unknown { kind, body } = &entry.body else {
        return Ok(());
    };
    match kind.as_str() {
        ONESHOT => {
            let call: OneShotCall =
                serde_json::from_str(body.get()).map_err(|error| error.to_string())?;
            let row = Spent::one_shot(account, entry.seq.get(), entry.at, &call);
            insert(db, &[row]).map_err(|error| error.to_string())
        }
        PURGED => {
            let purged: Purged =
                serde_json::from_str(body.get()).map_err(|error| error.to_string())?;
            replace(db, &purged).map_err(|error| error.to_string())
        }
        _ => Ok(()),
    }
}

/// 删掉的会话：它的单次行、以前的合计都删掉，换成这一份；`marks` 标上 `purged`，以后补的时候不再读它的目录。
fn replace(db: &Connection, purged: &Purged) -> Result<(), UsageError> {
    let session = purged.session.as_str();
    db.execute("DELETE FROM spent WHERE session = ?1", [session])?;
    insert(db, &purged.rows())?;
    let empty = Mark {
        segment: 0,
        bytes: 0,
        next: Seq::FIRST,
    };
    write_mark(db, session, &purged.who(), &empty, true)
}

/// 记下的一份；没有的没有。
fn marked(db: &Connection, source: &str) -> Result<Option<Marked>, UsageError> {
    let found = db
        .query_row(
            "SELECT owner, venue, parent, segment, bytes, next, purged FROM marks WHERE source = ?1",
            [source],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, bool>(6)?,
                ))
            },
        )
        .optional()?;
    let Some((owner, venue, parent, segment, bytes, next, purged)) = found else {
        return Ok(None);
    };
    let bad = |what: &str| UsageError::Bad(format!("mark {source}: {what}"));
    let number = |n: i64| u64::try_from(n).map_err(|_| bad("mark"));
    let who = Who {
        owner: AccountId::parse(&owner).map_err(|_| bad("owner"))?,
        venue: VenueId::parse(venue.as_deref().unwrap_or("local")).map_err(|_| bad("venue"))?,
        parent: parent
            .map(|parent| SessionId::parse(&parent).map_err(|_| bad("parent")))
            .transpose()?,
    };
    let mark = Mark {
        segment: number(segment)?,
        bytes: number(bytes)?,
        next: Seq::new(number(next)?).ok_or_else(|| bad("mark"))?,
    };
    Ok(Some(Marked { who, mark, purged }))
}

/// 记下会话 `source` 读到了 `mark`：`was` 是空的，没记过才写；不是空的，记的还是它才换。
fn put_mark(
    db: &Connection,
    source: &str,
    who: &Who,
    mark: &Mark,
    was: Option<&Mark>,
) -> Result<(), UsageError> {
    match was {
        None => {
            if marked(db, source)?.is_none() {
                write_mark(db, source, who, mark, false)?;
            }
            Ok(())
        }
        Some(was) => move_mark(db, source, was, mark),
    }
}

/// 记的正好是 `before` 的才挪到 `after`；别的（没记过、落后了、标了 `purged`）不动。
fn move_mark(db: &Connection, source: &str, before: &Mark, after: &Mark) -> Result<(), UsageError> {
    db.execute(
        "UPDATE marks SET segment = ?2, bytes = ?3, next = ?4 \
         WHERE source = ?1 AND segment = ?5 AND bytes = ?6 AND next = ?7 AND purged = 0",
        params![
            source,
            integer(after.segment, "mark")?,
            integer(after.bytes, "mark")?,
            integer(after.next.get(), "mark")?,
            integer(before.segment, "mark")?,
            integer(before.bytes, "mark")?,
            integer(before.next.get(), "mark")?,
        ],
    )?;
    Ok(())
}

/// 整个写一份，有的盖掉。
fn write_mark(
    db: &Connection,
    source: &str,
    who: &Who,
    mark: &Mark,
    purged: bool,
) -> Result<(), UsageError> {
    db.execute(
        "INSERT OR REPLACE INTO marks (source, owner, venue, parent, segment, bytes, next, purged) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            source,
            who.owner.as_str(),
            who.venue.as_str(),
            who.parent.as_ref().map(SessionId::as_str),
            integer(mark.segment, "mark")?,
            integer(mark.bytes, "mark")?,
            integer(mark.next.get(), "mark")?,
            purged,
        ],
    )?;
    Ok(())
}

/// 账号日志的种类。
fn kind(name: &str) -> Result<EventKind, UsageError> {
    EventKind::parse(name).map_err(|error| UsageError::Bad(error.to_string()))
}

/// 账号 `account` 的会话和回收处里的会话：编号和目录。名字不合会话编号写法的不算。
fn session_dirs(
    root: &DataRoot,
    account: &AccountId,
) -> Result<Vec<(SessionId, std::path::PathBuf)>, UsageError> {
    let mut dirs: Vec<_> = root
        .sessions(account)?
        .into_iter()
        .map(|id| {
            let dir = root.session_dir(account, &id);
            (id, dir)
        })
        .collect();
    let trash = root.trashed_sessions(account);
    match std::fs::read_dir(&trash) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry?;
                if let Some(id) = entry
                    .file_name()
                    .to_str()
                    .and_then(|name| SessionId::parse(name).ok())
                {
                    dirs.push((id, entry.path()));
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(dirs)
}
