//! 核心起来时清一次回收处（施工 3-8 三补，`docs/blueprint/core.md`「起来的先后」第 14 条）：管理员的回收处里删了满
//! [`KEEP`] 的会话真删掉（`store.md` 第 12 条）。写了 `ready` 以后在阻塞线程里清，不耽误头连上来；核心走之前等它清完。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::task::JoinHandle;

use miyu_kernel::id::AccountId;
use miyu_kernel::time::Timestamp;
use miyu_store::root::DataRoot;
use miyu_store::trash;

use crate::TARGET;

/// 删了的会话在回收处留多久：7 天（2026-09-30 项目主人定）。以后放进配置。
pub(crate) const KEEP: Duration = Duration::from_secs(7 * 24 * 3600);

/// 在阻塞线程里清一次账号 `account` 的回收处，照现在的钟。交回等它清完的那一头。
pub(crate) fn purge(root: DataRoot, account: AccountId) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || match trash::purge(&root, &account, now(), KEEP) {
        Ok(purged) => {
            if purged.removed > 0 {
                tracing::info!(target: TARGET, removed = purged.removed, "trash purged");
            }
            for (session, error) in purged.failed {
                tracing::warn!(target: TARGET, session = session.as_str(), error = %error, "trash entry kept");
            }
        }
        Err(error) => tracing::warn!(target: TARGET, error = %error, "trash not read"),
    })
}

/// 现在，到毫秒。读不出、出了范围的当 1970 年：那样什么都不满时限，一个都不删。
fn now() -> Timestamp {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|since| i64::try_from(since.as_millis()).ok())
        .unwrap_or(0);
    Timestamp::from_unix_millis(millis)
        .or_else(|| Timestamp::from_unix_millis(0))
        .unwrap_or_else(|| unreachable!("1970 年在时刻的范围里"))
}
