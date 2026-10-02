//! 会话列表的索引（施工 3-8 七补，`docs/designs/07-存储.md` 第六节、S3；`docs/blueprint/store/index.md`）：一个账号一份
//! SQLite，`home/<账号>/index/sessions.db`，一个会话一行（[`Row`]）。它是派生的：日志才是真相，索引随时可以删掉照日志重建，
//! 结构变了也是删掉重建，不写迁移（07 第六节）。
//!
//! 每一行记着照到日志的哪里（[`Mark`]）。会话写日志时一批批往上盖（[`SessionIndex::advance`]），只盖照到的正好是这一批之前
//! 的那一行：中间哪一批没盖上（更新失败、崩了），这一行就停在那里，等列会话照日志补上多出来的那一截（[`SessionIndex::put`]
//! 也只换照到的没变过的那一行）。所以索引落后了不要紧，只是列的时候多读一截；不会有照到的位置对、内容错的一行。
//!
//! 一个核心一个连接，拿锁护着：打开再关上同一个库文件会丢掉 SQLite 在这个进程里的文件锁（07 第六节，旧版把库弄坏过）。
//! 怎么开、坏了怎么删掉重建，和用量汇总共用（[`crate::sqlite`]，施工 8-15 挪出去的）。

mod row;

pub use row::{Row, cwd};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params_from_iter};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{Seq, SessionId};

use crate::log::Mark;
use crate::sqlite::{self, connect, remove};

pub use crate::sqlite::{DbError as IndexError, Opened};

/// 索引的文件名，在账号的 `index/` 下（[`crate::root::DataRoot::index`]）。
pub const FILE: &str = "sessions.db";

/// 表的结构的版本，记在 SQLite 的 `user_version` 里：结构一变就加一，对不上的删掉重建。
const VERSION: i64 = 1;

/// 表里的列，照这个先后读写（[`Row::from_sql`]、[`Row::to_sql`]）。
const COLUMNS: &str =
    "id, owner, parent, oneshot, title, pinned, cwd, created, last_active, segment, bytes, next";

/// 建表。`segment`、`bytes`、`next` 是这一行照到日志的哪里（[`Mark`]）；时刻存成毫秒。
const SCHEMA: &str = "CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    owner TEXT NOT NULL,
    parent TEXT,
    oneshot INTEGER NOT NULL,
    title TEXT NOT NULL,
    pinned INTEGER NOT NULL,
    cwd TEXT,
    created INTEGER NOT NULL,
    last_active INTEGER NOT NULL,
    segment INTEGER NOT NULL,
    bytes INTEGER NOT NULL,
    next INTEGER NOT NULL
) WITHOUT ROWID";

/// 一个账号的会话列表的索引。
#[derive(Debug)]
pub struct SessionIndex {
    /// 库文件在哪。
    path: PathBuf,
    /// 开着的连接：用不了的（删了重建也打不开）是空的，这时读出来没有一行、写什么都不写。
    db: Mutex<Option<Connection>>,
}

impl SessionIndex {
    /// 打开 `path` 这一份索引，没有就建（目录一起建，Unix 上 0700）。读不了、坏了、版本不对的，连同 SQLite 的
    /// `-wal`、`-shm` 删掉，建一份空的：一行都没有的索引照日志补（07 第六节「结构变了就删掉重建」）。
    pub fn open(path: &Path) -> (SessionIndex, Opened) {
        let (db, opened) = sqlite::open(path, SCHEMA, VERSION);
        let index = SessionIndex {
            path: path.to_path_buf(),
            db: Mutex::new(db),
        };
        (index, opened)
    }

    /// 删掉重建（施工 3-8 七补）：用着用着读出坏了的，关上连接，连同 `-wal`、`-shm` 删掉，建一份空的。
    ///
    /// # Errors
    ///
    /// 删不掉、建不成：这之后用不了索引。
    pub fn reset(&self) -> Result<(), IndexError> {
        let mut db = self.lock();
        drop(db.take());
        remove(&self.path)?;
        *db = Some(connect(&self.path, SCHEMA, VERSION)?.0);
        Ok(())
    }

    /// 全部的行，照会话编号。
    ///
    /// # Errors
    ///
    /// 读不了；有一行读不懂。
    pub fn rows(&self) -> Result<BTreeMap<SessionId, Row>, IndexError> {
        let db = self.lock();
        let Some(db) = db.as_ref() else {
            return Ok(BTreeMap::new());
        };
        let mut select = db.prepare(&format!("SELECT {COLUMNS} FROM sessions"))?;
        let mut rows = select.query([])?;
        let mut found = BTreeMap::new();
        while let Some(row) = rows.next()? {
            let row = Row::from_sql(row)?;
            found.insert(row.id.clone(), row);
        }
        Ok(found)
    }

    /// 照日志补好的一行写回去（列会话时）：`was` 是空的，表里还没有这一行才写；不是空的，表里那一行照到的还是它才换。
    /// 交回写了没有：没写的是会话自己同时往前盖过了，照它的。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn put(&self, row: &Row, was: Option<&Mark>) -> Result<bool, IndexError> {
        let db = self.lock();
        let Some(db) = db.as_ref() else {
            return Ok(false);
        };
        let values = row.to_sql()?;
        let changed = match was {
            None => db.execute(
                &format!("INSERT OR IGNORE INTO sessions ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"),
                params_from_iter(values),
            )?,
            Some(was) => {
                let [segment, bytes, next] = mark_values(was)?;
                db.execute(
                    &format!("{REPLACE} WHERE id = ?1 AND segment = ?13 AND bytes = ?14 AND next = ?15"),
                    params_from_iter(values.into_iter().chain([segment, bytes, next])),
                )?
            }
        };
        Ok(changed > 0)
    }

    /// 会话 `id` 落了盘的一批 `events`（施工 3-8 七补，`session/actor.md` 第 5 条）：写之前日志在 `before`，写完在 `after`。
    /// 这一批以 `session.created` 开头、写在日志的最前面的，照它新起一行；别的，表里那一行照到的正好是 `before` 才一条条
    /// 盖上去、照到 `after`，对不上的（中间哪一批没盖上、没有这一行）不动，等列会话照日志补。交回写了没有。
    ///
    /// # Errors
    ///
    /// 读不了、写不进。
    pub fn advance(
        &self,
        id: &SessionId,
        before: &Mark,
        events: &[Event],
        after: &Mark,
    ) -> Result<bool, IndexError> {
        let mut db = self.lock();
        let Some(db) = db.as_mut() else {
            return Ok(false);
        };
        let first = events.first();
        let created = first.filter(|event| {
            before.next == Seq::FIRST && matches!(event.body, Body::SessionCreated(_))
        });
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row = match created {
            Some(first) => Row::new(id.clone(), first),
            None => tx
                .query_row(
                    &format!("SELECT {COLUMNS} FROM sessions WHERE id = ?1"),
                    [id.as_str()],
                    |row| Ok(Row::from_sql(row)),
                )
                .optional()?
                .transpose()?
                .filter(|row| row.mark == *before),
        };
        let Some(mut row) = row else {
            return Ok(false);
        };
        for event in events {
            row.see(event);
        }
        row.mark = *after;
        tx.execute(
            &format!("INSERT OR REPLACE INTO sessions ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"),
            params_from_iter(row.to_sql()?),
        )?;
        tx.commit()?;
        Ok(true)
    }

    /// 删掉会话 `id` 那一行（删会话时）。没有这一行的也算成。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn remove(&self, id: &SessionId) -> Result<(), IndexError> {
        if let Some(db) = self.lock().as_ref() {
            db.execute("DELETE FROM sessions WHERE id = ?1", [id.as_str()])?;
        }
        Ok(())
    }

    /// 拿着连接。拿着锁的线程 panic 了也照样用：连接本身没坏，坏了的下次读出来再重建。
    fn lock(&self) -> MutexGuard<'_, Option<Connection>> {
        self.db
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// 换掉一整行，`WHERE` 由调的一方接上。
const REPLACE: &str = "UPDATE sessions SET id = ?1, owner = ?2, parent = ?3, oneshot = ?4, title = ?5, pinned = ?6, \
     cwd = ?7, created = ?8, last_active = ?9, segment = ?10, bytes = ?11, next = ?12";

/// 照到的位置写成三格。
fn mark_values(mark: &Mark) -> Result<[rusqlite::types::Value; 3], IndexError> {
    let number = |n: u64| {
        i64::try_from(n)
            .map(rusqlite::types::Value::Integer)
            .map_err(|_| IndexError::Bad("mark too large".to_string()))
    };
    Ok([
        number(mark.segment)?,
        number(mark.bytes)?,
        number(mark.next.get())?,
    ])
}

#[cfg(test)]
mod tests;
