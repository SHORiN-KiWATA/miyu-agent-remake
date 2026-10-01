//! 找文件的清单记几份（施工 W-2，`web-module.md`「三、列文件、找文件」第 5、6 条）：最多
//! [`miyu_fs::MAX_INDEXES`] 个目录的清单，多了丢最久没用的；`fresh` 时清单建好 [`miyu_fs::FRESH_SECS`] 秒以上
//! 的才重建；同一个目录同时来两次，第二次拿到第一次那一份（锁把两次请求排开，第二次来的时候第一次已经登记
//! 上了）。

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use miyu_fs::{Boundary, CAP, Index, MAX_INDEXES};

/// 一份清单，连着它什么时候建的、什么时候最后用过。
struct Slot {
    cwd: PathBuf,
    index: Index,
    built: Instant,
    used: Instant,
}

/// 找文件的清单记几份：核心一份，各个连接共用。
#[derive(Default)]
pub(crate) struct Cache {
    slots: Mutex<Vec<Slot>>,
}

impl Cache {
    /// 拿 `cwd` 这个目录的清单：没有的新建一份；`fresh` 且建好 `fresh_after` 以上的也重建（出厂值
    /// [`miyu_fs::FRESH_SECS`] 秒，核心照 `Core::with_files_fresh` 能改，测试用短的）；多了丢最久没用的腾地方。
    /// 读不了一层目录的，记一条运行日志（`dir=cwd error=…`）。
    pub(crate) fn get(
        &self,
        cwd: &Path,
        boundary: &Boundary,
        fresh: bool,
        fresh_after: Duration,
    ) -> Index {
        let mut slots = self
            .slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        if let Some(position) = slots.iter().position(|slot| slot.cwd == cwd) {
            let stale = fresh && now.duration_since(slots[position].built) >= fresh_after;
            if !stale {
                slots[position].used = now;
                return slots[position].index.clone();
            }
            slots[position] = slot(cwd, boundary, now);
            return slots[position].index.clone();
        }
        if slots.len() >= MAX_INDEXES
            && let Some(oldest) = slots
                .iter()
                .enumerate()
                .min_by_key(|(_, slot)| slot.used)
                .map(|(at, _)| at)
        {
            slots.remove(oldest);
        }
        let slot = slot(cwd, boundary, now);
        let index = slot.index.clone();
        slots.push(slot);
        index
    }
}

/// 新起一份清单：读不了一层目录的（没有权限这类），记一条运行日志。
fn slot(cwd: &Path, boundary: &Boundary, now: Instant) -> Slot {
    let dir = cwd.to_path_buf();
    let index = Index::start(cwd.to_path_buf(), boundary.clone(), CAP, move |error| {
        tracing::warn!(target: super::TARGET, dir = %dir.display(), error = %error, "files index failed");
    });
    Slot {
        cwd: cwd.to_path_buf(),
        index,
        built: now,
        used: now,
    }
}
