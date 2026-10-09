//! 检索库（施工 R-1，`docs/blueprint/recall.md`「怎么走」第二条）：一个 SQLite 文件，`items` 一条一行，contentless 的 FTS5
//! `terms` 只存倒排索引，rowid 就是 `items.id`。照关键词找，bm25 最相关的在前。
//!
//! 库是派生的：真相在会话日志、记忆日志、知识库的文件里，库随时能删掉重建（`docs/designs/07-存储.md` S1、S3）。怎么开、坏了
//! 和版本不对怎么删掉重建，和会话列表的索引、用量汇总共用（[`crate::sqlite`]）。一个库一个核心开一个连接、一直开着，拿锁
//! 护着：同一个进程里开了又关同一个库文件会丢掉 SQLite 的文件锁（07 第六节）。
//!
//! 一段字怎么切成词、查询怎么拼，是纯逻辑，在 `miyu-recall`。
//!
//! 每个来源（例如一个会话）照到了哪个序号记在 `marks` 里，和那一批放进、拿掉在同一个事务里写（[`RecallIndex::apply`]，
//! 施工 R-2 上）：没往前挪的，下次照真相补。键以 `来源/` 开头，拿掉一个来源照它（[`RecallIndex::forget`]）。

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use miyu_kernel::id::Seq;
use miyu_kernel::time::Timestamp;

use crate::sqlite::{self, connect, integer, remove};

pub use crate::sqlite::{DbError, Opened};
pub use indexes::{Opener, RecallIndexes};
pub use room::Room;

mod indexes;
mod room;
mod vectors;

pub use vectors::Near;

/// 表的结构的版本，记在 SQLite 的 `user_version` 里：结构一变就加一，对不上的删掉重建，不写迁移。
const VERSION: i64 = 3;

/// 建表（`recall.md`「库的结构」）。`terms` 是 contentless 的：词只进倒排索引，原文在 `items.text`；`contentless_delete`
/// 让它能照 rowid 删（SQLite 3.43 起，`bundled` 带的是 3.53）。`at` 是毫秒，给以后的排名用。`marks` 是每个来源照到了哪个
/// 序号（施工 R-2 上，版本 2）。`buried` 是墓碑：撤销了的回合、删掉了的会话，记忆的出处照它判活不活（施工 R-3 上，版本 3）。
/// `vectors` 是每一条照模型的向量（施工 R-5 下，`vectors.rs`）：不换版本，每次开库、重建以后照 `vectors::ADD` 补上这张表，
/// 以前的库也就有了。换版本就要删掉重建，删掉的会话那几块墓碑是删会话时写的，重建补不回来。
const SCHEMA: &str = "CREATE TABLE buried (
    key TEXT PRIMARY KEY NOT NULL
) WITHOUT ROWID;
CREATE TABLE marks (
    source TEXT PRIMARY KEY NOT NULL,
    upto INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TABLE items (
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

/// 一批里的一处改动（[`RecallIndex::apply`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// 放进一条，键已经有的整条换掉。
    Put {
        /// 键：`来源/…`。
        key: String,
        /// 字。
        text: String,
        /// 时刻。
        at: Timestamp,
    },
    /// 拿掉一条，没有的不要紧。
    Remove {
        /// 键。
        key: String,
    },
    /// 埋一块墓碑（施工 R-3 上）：埋过的不要紧。
    Bury {
        /// 键：`会话/回合`，或者整个会话 `会话/`。
        key: String,
    },
    /// 揭掉一块墓碑，没埋的不要紧。
    Unbury {
        /// 键。
        key: String,
    },
}

/// 找到的一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// 放进来时的键。
    pub key: String,
    /// 第几名，从 0 起：以后几路照名次合并（`recall.md` 第三条）。
    pub rank: usize,
    /// 放进来时的字（施工 R-3 中：搜以前的对话要给她看）。
    pub text: String,
    /// 放进来时的时刻。
    pub at: Timestamp,
}

impl RecallIndex {
    /// 打开 `path` 这一份库，没有就建（目录一起建，Unix 上 0700）。读不了、坏了、版本不对的，连同 SQLite 的 `-wal`、
    /// `-shm` 删掉，建一份空的，调的一方照真相补。删了重建也打不开的，这一回用不了（[`Opened::Unusable`]）。
    pub fn open(path: &Path) -> (RecallIndex, Opened) {
        let (db, opened) = sqlite::open(path, SCHEMA, VERSION);
        // 以前的库（同一个版本、没有向量表）补上这张表；补不上的这一回用不了。
        let (db, opened) = match db.map(|db| db.execute_batch(vectors::ADD).map(|()| db)) {
            Some(Ok(db)) => (Some(db), opened),
            Some(Err(error)) => (None, Opened::Unusable(error.into())),
            None => (None, opened),
        };
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
        let fresh = connect(&self.path, SCHEMA, VERSION)?.0;
        fresh.execute_batch(vectors::ADD)?;
        *db = Some(fresh);
        Ok(())
    }

    /// 放进一条：`key` 已经有的整条换掉（字、时刻、词都换），在一个事务里。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn put(&self, key: &str, text: &str, at: Timestamp) -> Result<(), DbError> {
        self.write(|tx| put(tx, key, text, at))
    }

    /// 拿掉一条，没有的不要紧，在一个事务里。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn remove(&self, key: &str) -> Result<(), DbError> {
        self.write(|tx| remove_key(tx, key))
    }

    /// 来源 `source` 的一批改动，照先后做，连同它照到了 `upto`，在一个事务里写（施工 R-2 上）：要么都写上，要么都没写、
    /// 照到的位置也不动，下次照真相补。这个来源照到的位置已经在 `upto` 以后的、整个来源埋了墓碑 `来源/` 的（删掉了），整批
    /// 不写（施工 R-2 下）：后台补旧会话读日志在前、写在后，会话自己这时可能已经往前写了、可能刚被删掉，落后的一批不该把
    /// 撤销了的、删掉的又放回去。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn apply(&self, source: &str, edits: &[Edit], upto: Seq) -> Result<(), DbError> {
        let upto = integer(upto.get(), "upto")?;
        self.write(|tx| {
            let got: Option<i64> = tx
                .query_row(
                    "SELECT upto FROM marks WHERE source = ?1",
                    [source],
                    |row| row.get(0),
                )
                .optional()?;
            if got.is_some_and(|got| got > upto) {
                return Ok(());
            }
            // 整个来源埋了墓碑的（会话删掉了）不再写：补的一批读日志在前，删会话可能夹在中间。
            let tomb = format!("{source}/");
            let buried = tx
                .query_row("SELECT 1 FROM buried WHERE key = ?1", [&tomb], |_| Ok(()))
                .optional()?
                .is_some();
            if buried {
                return Ok(());
            }
            for edit in edits {
                match edit {
                    Edit::Put { key, text, at } => put(tx, key, text, *at)?,
                    Edit::Remove { key } => remove_key(tx, key)?,
                    Edit::Bury { key } => {
                        tx.execute("INSERT OR IGNORE INTO buried (key) VALUES (?1)", [key])?;
                    }
                    Edit::Unbury { key } => {
                        tx.execute("DELETE FROM buried WHERE key = ?1", [key])?;
                    }
                }
            }
            tx.execute(
                "INSERT OR REPLACE INTO marks (source, upto) VALUES (?1, ?2)",
                params![source, upto],
            )?;
            Ok(())
        })
    }

    /// 来源 `source` 照到了哪个序号；没照过的没有（施工 R-2 上）。库用不了的也没有：调的一方照真相整份补，写也写不进。
    ///
    /// # Errors
    ///
    /// 读不了；记着的不是一个序号。
    pub fn mark(&self, source: &str) -> Result<Option<Seq>, DbError> {
        let db = self.lock();
        let Some(db) = db.as_ref() else {
            return Ok(None);
        };
        let upto: Option<i64> = db
            .query_row(
                "SELECT upto FROM marks WHERE source = ?1",
                [source],
                |row| row.get(0),
            )
            .optional()?;
        upto.map(|upto| {
            u64::try_from(upto)
                .ok()
                .and_then(Seq::new)
                .ok_or_else(|| DbError::Bad(format!("mark {upto}")))
        })
        .transpose()
    }

    /// 埋一块墓碑，不碰照到哪（施工 R-3 上：删会话时埋整个会话 `会话/`）。埋过的不要紧。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn bury(&self, key: &str) -> Result<(), DbError> {
        self.write(|tx| {
            tx.execute("INSERT OR IGNORE INTO buried (key) VALUES (?1)", [key])?;
            Ok(())
        })
    }

    /// 键 `key` 埋了墓碑没有（施工 R-3 上）。库用不了的当没埋：记忆的出处照活的算，宁可多想起来，不吞掉人说过的。
    ///
    /// # Errors
    ///
    /// 读不了。
    pub fn is_buried(&self, key: &str) -> Result<bool, DbError> {
        let db = self.lock();
        let Some(db) = db.as_ref() else {
            return Ok(false);
        };
        Ok(db
            .query_row("SELECT 1 FROM buried WHERE key = ?1", [key], |_| Ok(()))
            .optional()?
            .is_some())
    }

    /// 拿掉来源 `source` 的全部（键以 `source/` 开头的）和它照到了哪，在一个事务里（施工 R-2 上：删会话）。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn forget(&self, source: &str) -> Result<(), DbError> {
        let prefix = format!("{source}/");
        let length = integer(prefix.chars().count() as u64, "prefix")?;
        self.write(|tx| {
            tx.execute(
                "DELETE FROM terms WHERE rowid IN (SELECT id FROM items WHERE substr(key, 1, ?2) = ?1)",
                params![prefix, length],
            )?;
            tx.execute(
                "DELETE FROM items WHERE substr(key, 1, ?2) = ?1",
                params![prefix, length],
            )?;
            tx.execute(
                "DELETE FROM vectors WHERE substr(key, 1, ?2) = ?1",
                params![prefix, length],
            )?;
            tx.execute("DELETE FROM marks WHERE source = ?1", [source])?;
            Ok(())
        })
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
            "SELECT items.key, items.text, items.at FROM terms JOIN items ON items.id = terms.rowid
             WHERE terms MATCH ?1 ORDER BY bm25(terms) LIMIT ?2",
        )?;
        let rows = select.query_map(params![query, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        let mut hits = Vec::new();
        for (rank, row) in rows.enumerate() {
            let (key, text, at) = row?;
            let at = Timestamp::from_unix_millis(at)
                .ok_or_else(|| DbError::Bad(format!("time {at}")))?;
            hits.push(Hit {
                key,
                rank,
                text,
                at,
            });
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

    /// 在一个事务里写；库用不了的什么都不做。
    fn write(
        &self,
        body: impl FnOnce(&Transaction<'_>) -> Result<(), DbError>,
    ) -> Result<(), DbError> {
        let mut db = self.lock();
        let Some(db) = db.as_mut() else {
            return Ok(());
        };
        let tx = db.transaction()?;
        body(&tx)?;
        tx.commit()?;
        Ok(())
    }

    /// 拿锁。别的线程拿着锁崩了，库还是好的（写都在事务里），照常用。
    fn lock(&self) -> MutexGuard<'_, Option<Connection>> {
        self.db
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// 放进一条：键有旧的先从 `terms` 删掉旧的那一行、再换 `items` 那一行，然后写进新的词。
fn put(tx: &Transaction<'_>, key: &str, text: &str, at: Timestamp) -> Result<(), DbError> {
    let at = at.unix_millis();
    let words = miyu_recall::index_terms(text);
    let id = match id_of(tx, key)? {
        Some(id) => {
            // 字换了的，以前的向量作废（施工 R-5 下）；一样的照留，不重算。
            tx.execute(
                "DELETE FROM vectors WHERE key = ?1 AND (SELECT text FROM items WHERE id = ?2) <> ?3",
                params![key, id, text],
            )?;
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
        params![id, words],
    )?;
    Ok(())
}

/// 拿掉一条，没有的不要紧。
fn remove_key(tx: &Transaction<'_>, key: &str) -> Result<(), DbError> {
    if let Some(id) = id_of(tx, key)? {
        tx.execute("DELETE FROM terms WHERE rowid = ?1", [id])?;
        tx.execute("DELETE FROM items WHERE id = ?1", [id])?;
    }
    tx.execute("DELETE FROM vectors WHERE key = ?1", [key])?;
    Ok(())
}

/// 键是 `key` 的那一行的 `id`。
fn id_of(tx: &Transaction<'_>, key: &str) -> Result<Option<i64>, DbError> {
    Ok(tx
        .query_row("SELECT id FROM items WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()?)
}
