//! 抓过的记在核心的内存里（`net.md`「怎么走」第 9 条，照桥）：抓到了的记得久（页面的 og 标签基本不动）；
//! `no_preview` 记一会儿，这种结论不会自己变；`unreachable` 只记很短：会自己变好，记久了等于把一条本来能出卡片的
//! 链接按死（旧版 09-09 那条 bilibili 就是这么变成纯文字的）。满了整个清空：几百字节一条的表，清空的代价只是
//! 重抓一次，不值得做 LRU。
//!
//! 什么时候都由调的一方给（`now`）：测试不用真等 6 个小时。

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::{Card, Why};

/// 三种结果各记多久、最多记几条（`link_preview.json` 的 `remember`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Keep {
    /// 抓到了的。
    pub(crate) found: Duration,
    /// `no_preview`。
    pub(crate) no_preview: Duration,
    /// `unreachable`。
    pub(crate) unreachable: Duration,
    /// 最多几条。
    pub(crate) entries: usize,
}

/// 记着的一条：结果和什么时候记的。
type Entry = (Result<Card, Why>, Instant);

/// 记着的：地址 → 一条。
pub(crate) struct Remember {
    keep: Keep,
    entries: Mutex<HashMap<String, Entry>>,
}

impl Remember {
    /// 空的。
    pub(crate) fn new(keep: Keep) -> Remember {
        Remember {
            keep,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// `key` 记着的、到 `now` 还没过期的结果。
    pub(crate) fn get(&self, key: &str, now: Instant) -> Option<Result<Card, Why>> {
        let entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let (value, at) = entries.get(key)?;
        let ttl = match value {
            Ok(_) => self.keep.found,
            Err(Why::NoPreview) => self.keep.no_preview,
            // 读不成地址的、不是 http 的不记（第 2 条），走不到这里；照最短的算。
            Err(_) => self.keep.unreachable,
        };
        (now.saturating_duration_since(*at) < ttl).then(|| value.clone())
    }

    /// 记一条，`now` 是记的时候；满了先整个清空。
    pub(crate) fn put(&self, key: String, value: Result<Card, Why>, now: Instant) {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        if entries.len() >= self.keep.entries && !entries.contains_key(&key) {
            entries.clear();
        }
        entries.insert(key, (value, now));
    }
}

#[cfg(test)]
mod tests;
