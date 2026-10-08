//! 删掉的人格（施工 P-3 下，`docs/blueprint/personas.md`「改」）：家目录里的人格目录整个挪进
//! `home/<账号>/trash/personas/<编号>.<删的时刻，毫秒>/`，里面多一个 `deleted_at`（同会话的写法）；核心起来时和会话一起清，
//! 删了满时限的真删（同会话，7 天）。同一个编号删几次各是各的。挪是一次改名：要么挪了、要么没挪。

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

use miyu_kernel::id::AccountId;
use miyu_kernel::time::Timestamp;

use super::{DELETED_AT, deleted_at};
use crate::durable::{create_dir, sync_dir};
use crate::root::DataRoot;

/// 把人格目录 `from`（编号 `id`）挪进账号 `account` 的回收处，删的时刻是 `at`：先在目录里写下 `deleted_at`、同步，再改名，
/// 同步两头的上一层。回收处没有的先建。
///
/// # Errors
///
/// 目录不在；写不了 `deleted_at`；建不了回收处；改不了名；同步不了。
pub fn discard(
    root: &DataRoot,
    account: &AccountId,
    from: &Path,
    id: &str,
    at: Timestamp,
) -> io::Result<()> {
    let trash = root.trashed_personas(account);
    let mut stamp = File::create(from.join(DELETED_AT))?;
    stamp.write_all(format!("{at}\n").as_bytes())?;
    stamp.sync_all()?;
    drop(stamp);
    create_dir(&trash)?;
    fs::rename(from, trash.join(format!("{id}.{}", at.unix_millis())))?;
    if let Some(personas) = from.parent() {
        sync_dir(personas)?;
    }
    sync_dir(&trash)
}

/// 清了一次人格的回收处。
#[derive(Debug, Default)]
pub struct Purged {
    /// 真删掉了几个。
    pub removed: usize,
    /// 留下了、却不是因为没满时限的：读不出删的时刻的，删不掉的。哪一个（回收处里的名字），为什么（系统的原话，英文）。
    pub failed: Vec<(String, String)>,
}

/// 清一次账号 `account` 的人格回收处：删的时刻离 `now` 满了 `keep` 的连目录整个删掉；没满的、时钟往回拨过的留着；读不出
/// 删的时刻的留着，记进 [`Purged::failed`]。回收处还没有的什么都不做。
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
    let entries = match fs::read_dir(root.trashed_personas(account)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(purged),
        Err(error) => return Err(error),
    };
    let keep = i64::try_from(keep.as_millis()).unwrap_or(i64::MAX);
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let deleted = match deleted_at(&dir) {
            Ok(deleted) => deleted,
            Err(why) => {
                purged.failed.push((name, why));
                continue;
            }
        };
        if now.unix_millis().saturating_sub(deleted.unix_millis()) < keep {
            continue;
        }
        match fs::remove_dir_all(&dir) {
            Ok(()) => purged.removed += 1,
            Err(error) => purged.failed.push((name, error.to_string())),
        }
    }
    Ok(purged)
}

#[cfg(test)]
mod tests;
