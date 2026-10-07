//! 检索库（施工 R-1，`docs/blueprint/recall.md`「怎么走」第二条）：一个 SQLite 文件，`items` 一条一行，contentless 的 FTS5
//! `terms` 只存倒排索引，rowid 就是 `items.id`。照关键词找，bm25 最相关的在前。
//!
//! 库是派生的：真相在会话日志、记忆日志、知识库的文件里，库随时能删掉重建（`docs/designs/07-存储.md` S1、S3）。怎么开、坏了
//! 和版本不对怎么删掉重建，和会话列表的索引、用量汇总共用（[`crate::sqlite`]）。一个库一个核心开一个连接、一直开着，拿锁
//! 护着：同一个进程里开了又关同一个库文件会丢掉 SQLite 的文件锁（07 第六节）。
//!
//! 一段字怎么切成词、查询怎么拼，是纯逻辑，在 `miyu-recall`。

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, OptionalExtension, params};

use miyu_kernel::time::Timestamp;

use crate::sqlite::{self, connect, integer, remove};

pub use crate::sqlite::{DbError, Opened};

/// 表的结构的版本，记在 SQLite 的 `user_version` 里：结构一变就加一，对不上的删掉重建，不写迁移。
const VERSION: i64 = 1;

/// 建表（`recall.md`「库的结构」）。`terms` 是 contentless 的：词只进倒排索引，原文在 `items.text`；`contentless_delete`
/// 让它能照 rowid 删（SQLite 3.43 起，`bundled` 带的是 3.53）。`at` 是毫秒，给以后的排名用。
const SCHEMA: &str = "CREATE TABLE items (
    id INTEGER PRIMARY KEY,
    key TEXT NOT NULL UNIQUE,
    text TEXT NOT NULL,
    at INTEGER NOT NULL
);
CREATE VIRTUAL TABLE terms USING fts5(words, content='', contentless_delete=1, tokenize='unicode61')";

/// 一个检索库：键是调的一方起的字符串，库不解读。
#[derive(Debug)]
pub struct RecallIndex {
    /// 库文件在哪。
    path: PathBuf,
    /// 开着的连接：用不了的（删了重建也打不开）是空的，这时找不到任何一条、写什么都不写。
    db: Mutex<Option<Connection>>,
}

/// 找到的一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// 放进来时的键。
    pub key: String,
    /// 第几名，从 0 起：以后几路照名次合并（`recall.md` 第三条）。
    pub rank: usize,
}

impl RecallIndex {
    /// 打开 `path` 这一份库，没有就建（目录一起建，Unix 上 0700）。读不了、坏了、版本不对的，连同 SQLite 的 `-wal`、
    /// `-shm` 删掉，建一份空的，调的一方照真相补。删了重建也打不开的，这一回用不了（[`Opened::Unusable`]）。
    pub fn open(path: &Path) -> (RecallIndex, Opened) {
        let (db, opened) = sqlite::open(path, SCHEMA, VERSION);
        let index = RecallIndex {
            path: path.to_path_buf(),
            db: Mutex::new(db),
        };
        (index, opened)
    }

    /// 删掉重建：用着用着读出坏了的，关上连接，连同 `-wal`、`-shm` 删掉，建一份空的。
    ///
    /// # Errors
    ///
    /// 删不掉、建不成：这之后用不了这个库。
    pub fn reset(&self) -> Result<(), DbError> {
        let mut db = self.lock();
        drop(db.take());
        remove(&self.path)?;
        *db = Some(connect(&self.path, SCHEMA, VERSION)?.0);
        Ok(())
    }

    /// 放进一条：`key` 已经有的整条换掉（字、时刻、词都换），在一个事务里。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn put(&self, key: &str, text: &str, at: Timestamp) -> Result<(), DbError> {
        let mut db = self.lock();
        let Some(db) = db.as_mut() else {
            return Ok(());
        };
        let at = at.unix_millis();
        let terms = miyu_recall::index_terms(text);
        let tx = db.transaction()?;
        let old: Option<i64> = tx
            .query_row("SELECT id FROM items WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?;
        let id = match old {
            Some(id) => {
                tx.execute("DELETE FROM terms WHERE rowid = ?1", [id])?;
                tx.execute(
                    "UPDATE items SET text = ?2, at = ?3 WHERE id = ?1",
                    params![id, text, at],
                )?;
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO items (key, text, at) VALUES (?1, ?2, ?3)",
                    params![key, text, at],
                )?;
                tx.last_insert_rowid()
            }
        };
        tx.execute(
            "INSERT INTO terms (rowid, words) VALUES (?1, ?2)",
            params![id, terms],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// 拿掉一条，没有的不要紧，在一个事务里。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn remove(&self, key: &str) -> Result<(), DbError> {
        let mut db = self.lock();
        let Some(db) = db.as_mut() else {
            return Ok(());
        };
        let tx = db.transaction()?;
        let old: Option<i64> = tx
            .query_row("SELECT id FROM items WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?;
        if let Some(id) = old {
            tx.execute("DELETE FROM terms WHERE rowid = ?1", [id])?;
            tx.execute("DELETE FROM items WHERE id = ?1", [id])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// 照关键词找 `text`，最多 `limit` 条，bm25 最相关的在前。切不出词的（只有标点、空白）找不到任何一条。
    ///
    /// # Errors
    ///
    /// 读不了。`MATCH` 的写法是 `miyu-recall` 拼好的，SQLite 照样报错的也如实交回。
    pub fn search(&self, text: &str, limit: usize) -> Result<Vec<Hit>, DbError> {
        let db = self.lock();
        let (Some(db), Some(query)) = (db.as_ref(), miyu_recall::query(text)) else {
            return Ok(Vec::new());
        };
        let limit = integer(limit as u64, "limit")?;
        let mut select = db.prepare(
            "SELECT items.key FROM terms JOIN items ON items.id = terms.rowid
             WHERE terms MATCH ?1 ORDER BY bm25(terms) LIMIT ?2",
        )?;
        let keys = select.query_map(params![query, limit], |row| row.get::<_, String>(0))?;
        let mut hits = Vec::new();
        for (rank, key) in keys.enumerate() {
            hits.push(Hit { key: key?, rank });
        }
        Ok(hits)
    }

    /// 库里有哪些键，照键排：重建时和真相比，多的拿掉。
    ///
    /// # Errors
    ///
    /// 读不了。
    pub fn keys(&self) -> Result<Vec<String>, DbError> {
        let db = self.lock();
        let Some(db) = db.as_ref() else {
            return Ok(Vec::new());
        };
        let mut select = db.prepare("SELECT key FROM items ORDER BY key")?;
        let keys = select.query_map([], |row| row.get::<_, String>(0))?;
        Ok(keys.collect::<Result<_, _>>()?)
    }

    /// 拿锁。别的线程拿着锁崩了，库还是好的（写都在事务里），照常用。
    fn lock(&self) -> MutexGuard<'_, Option<Connection>> {
        self.db
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
