//! 回收处（施工 3-8 三补，`docs/blueprint/store.md` 第 12 条）：删掉的会话，整个会话目录挪进
//! `home/<账号>/trash/sessions/<会话编号>/`，里面多一个 `deleted_at`，写着删的时刻；核心起来时清一次，满了留的时限的
//! 真删（2026-09-30 项目主人定：删了的进回收处，留 7 天）。
//!
//! 挪是一次改名：要么挪了、要么没挪，不会留半个会话。`deleted_at` 先写进还在原处的会话目录、落了盘，再改名：回收处里
//! 的每一个都带着它；写了没来得及挪的，会话照旧在原处，日志只认 12 位数字的段，不碍着它。blob 不动：回收随存储的回收
//! 那一步（`07-存储.md` 第五节）。

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::time::Timestamp;

use crate::durable::{create_dir, sync_dir};
use crate::root::DataRoot;

/// 回收处里每个会话目录下写着删的时刻的那个文件：一行，事件的时刻写法，例如 `2026-09-30T12:00:00.000Z`，加换行。
pub const DELETED_AT: &str = "deleted_at";

/// 把账号 `account` 的会话 `session` 挪进回收处，删的时刻是 `at`：先在会话目录里写下 `deleted_at`、同步，再把整个目录
/// 改名成回收处里的那一个，同步两头的上一层。回收处没有的先建（Unix 上 0700）。调的一方先停下这个会话：开着的文件
/// Windows 上挪不走。
///
/// # Errors
///
/// 会话目录不在；写不了 `deleted_at`；建不了回收处；改不了名（回收处里已经有同名的也算）；同步不了。
pub fn discard(
    root: &DataRoot,
    account: &AccountId,
    session: &SessionId,
    at: Timestamp,
) -> io::Result<()> {
    let from = root.session_dir(account, session);
    let trash = root.trashed_sessions(account);
    let mut stamp = File::create(from.join(DELETED_AT))?;
    stamp.write_all(format!("{at}\n").as_bytes())?;
    stamp.sync_all()?;
    drop(stamp);
    create_dir(&trash)?;
    fs::rename(&from, trash.join(session.as_str()))?;
    if let Some(sessions) = from.parent() {
        sync_dir(sessions)?;
    }
    sync_dir(&trash)
}

/// 清了一次回收处。
#[derive(Debug, Default)]
pub struct Purged {
    /// 真删掉了几个。
    pub removed: usize,
    /// 留下了、却不是因为没满时限的：读不出删的时刻的，删不掉的。哪一个会话，为什么（系统的原话，英文）。
    pub failed: Vec<(SessionId, String)>,
}

/// 清一次账号 `account` 的回收处：删的时刻离 `now` 满了 `keep` 的，连目录整个删掉；没满的、删的时刻比 `now` 还晚的
/// （时钟往回拨过）留着；读不出删的时刻的也留着，记进 [`Purged::failed`]：说不清它删了多久，不猜。名字不合会话编号写法的
/// 不是这里放的，不看。回收处还没有的，什么都不做。
///
/// # Errors
///
/// 回收处读不了。
pub fn purge(
    root: &DataRoot,
    account: &AccountId,
    now: Timestamp,
    keep: Duration,
) -> io::Result<Purged> {
    let mut purged = Purged::default();
    let entries = match fs::read_dir(root.trashed_sessions(account)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(purged),
        Err(error) => return Err(error),
    };
    let keep = i64::try_from(keep.as_millis()).unwrap_or(i64::MAX);
    for entry in entries {
        let entry = entry?;
        let Some(session) = entry
            .file_name()
            .to_str()
            .and_then(|name| SessionId::parse(name).ok())
        else {
            continue;
        };
        let dir = entry.path();
        let deleted = match deleted_at(&dir) {
            Ok(deleted) => deleted,
            Err(why) => {
                purged.failed.push((session, why));
                continue;
            }
        };
        if now.unix_millis().saturating_sub(deleted.unix_millis()) < keep {
            continue;
        }
        match fs::remove_dir_all(&dir) {
            Ok(()) => purged.removed += 1,
            Err(error) => purged.failed.push((session, error.to_string())),
        }
    }
    Ok(purged)
}

/// 回收处里的会话目录 `dir` 写着的删的时刻。读不了、写法不对的，交回为什么。
fn deleted_at(dir: &Path) -> Result<Timestamp, String> {
    let text = fs::read_to_string(dir.join(DELETED_AT)).map_err(|error| error.to_string())?;
    Timestamp::parse(text.trim_end()).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
