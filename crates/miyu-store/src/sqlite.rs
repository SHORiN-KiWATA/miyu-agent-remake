//! 派生数据的 SQLite 库怎么开（`docs/designs/07-存储.md` 第六节、S3）：会话列表的索引（施工 3-8 七补，[`crate::index`]）和
//! 用量汇总（施工 8-15，[`crate::usage`]）共用。库是派生的：日志才是真相，读不了、坏了、版本不对的删掉建一份空的，空的照
//! 日志补，就是重建；结构变了也是删掉重建，不写迁移。
//!
//! - 版本记在 SQLite 的 `user_version` 里；0 是新文件，建表、记下版本；对得上的跑一次 `quick_check`。
//! - 日志模式 WAL、`synchronous` 是 `NORMAL`：提交时不同步，断电最多丢最后几次更新，库不会坏，丢了的照日志补。
//! - 一个库一个进程只开一个连接，一直开着：打开再关上同一个库文件会丢掉 SQLite 在这个进程里的文件锁（07 第六节，旧版
//!   把库弄坏过）。删的时候连同 `-wal`、`-shm` 三个一起删。

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::durable::create_dir;

/// 打开一个库时是什么情形：调的一方照它记运行日志。
#[derive(Debug)]
pub enum Opened {
    /// 原来就有，版本对、查过没坏。
    Kept,
    /// 原来没有（或者是个空文件），新建了一份空的。
    Created,
    /// 原来的读不了、坏了、版本不对（为什么），删掉换了一份空的。
    Rebuilt(DbError),
    /// 删掉重建也打不开（为什么）：这一回用不了。
    Unusable(DbError),
}

/// 库读写出错。
#[derive(Debug)]
pub enum DbError {
    /// SQLite 说的。
    Sql(rusqlite::Error),
    /// 建目录、删文件出错。
    Io(io::Error),
    /// 库里的东西不对：版本、查坏了、一行读不懂。
    Bad(String),
}

/// 打开 `path` 这一份库，没有就照 `schema` 建，版本是 `version`（目录一起建，Unix 上 0700）。读不了、坏了、版本不对的，
/// 连同 `-wal`、`-shm` 删掉，建一份空的。交回开着的连接（删了重建也打不开的没有）和是什么情形。
pub(crate) fn open(path: &Path, schema: &str, version: i64) -> (Option<Connection>, Opened) {
    match connect(path, schema, version) {
        Ok((db, true)) => (Some(db), Opened::Created),
        Ok((db, false)) => (Some(db), Opened::Kept),
        Err(why) => match remove(path).and_then(|()| connect(path, schema, version)) {
            Ok((db, _)) => (Some(db), Opened::Rebuilt(why)),
            Err(error) => (None, Opened::Unusable(error)),
        },
    }
}

/// 打开或者新建，交回连接、是不是新建的：新的建表、记下版本；有的查版本、查坏没坏（`quick_check`）。
pub(crate) fn connect(
    path: &Path,
    schema: &str,
    version: i64,
) -> Result<(Connection, bool), DbError> {
    if let Some(dir) = path.parent() {
        create_dir(dir)?;
    }
    let db = Connection::open(path)?;
    let found: i64 = db.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match found {
        0 => {
            db.execute_batch(&format!(
                "BEGIN; {schema}; PRAGMA user_version = {version}; COMMIT;"
            ))?;
        }
        found if found == version => {
            let checked: String = db.pragma_query_value(None, "quick_check", |row| row.get(0))?;
            if checked != "ok" {
                return Err(DbError::Bad(format!("quick_check: {checked}")));
            }
        }
        other => return Err(DbError::Bad(format!("version {other}, not {version}"))),
    }
    let mode: String = db.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(DbError::Bad(format!("journal mode {mode}")));
    }
    db.pragma_update(None, "synchronous", "NORMAL")?;
    Ok((db, found == 0))
}

/// 删掉库文件，连同 SQLite 的 `-wal`、`-shm`。没有的不要紧。
pub(crate) fn remove(path: &Path) -> Result<(), DbError> {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        match fs::remove_file(PathBuf::from(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// 一个数写进 SQLite 的整数：超过的说是哪一样太大。
pub(crate) fn integer(n: u64, what: &str) -> Result<i64, DbError> {
    i64::try_from(n).map_err(|_| DbError::Bad(format!("{what} too large")))
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::Sql(error) => error.fmt(f),
            DbError::Io(error) => error.fmt(f),
            DbError::Bad(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for DbError {}

impl From<rusqlite::Error> for DbError {
    fn from(error: rusqlite::Error) -> DbError {
        DbError::Sql(error)
    }
}

impl From<io::Error> for DbError {
    fn from(error: io::Error) -> DbError {
        DbError::Io(error)
    }
}
