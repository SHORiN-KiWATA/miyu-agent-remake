//! 领任务编号（`docs/blueprint/kernel/ids.md`「任务编号」，施工 7-5）：一个会话 actor 一份，后台命令和子代理共用一串，
//! 从日志里用过的最大编号往下数。几次调用一起跑的，各领各的，不重不漏。
//!
//! 编号不回收（`kernel/history.md`）：领了没派成的（造子会话失败、调用被掐掉）也不再发；载入以后照日志里记下的数，没记下
//! 的那几个号没人用过，照样可以再发。

use std::sync::atomic::{AtomicU64, Ordering};

use miyu_kernel::id::JobId;

/// 一个会话的任务编号。
#[derive(Debug)]
pub(crate) struct JobIds(AtomicU64);

impl JobIds {
    /// 用过的最大编号是 `used`（内核的 `last_job_number`，没有是 0），下一个从它加一数起。
    pub(crate) fn starting_after(used: u64) -> JobIds {
        JobIds(AtomicU64::new(used))
    }

    /// 领下一个编号。
    ///
    /// # Panics
    ///
    /// 实际不会：领到 `u64` 的尽头要派十八亿亿次。
    pub(crate) fn next(&self) -> JobId {
        let n = self.0.fetch_add(1, Ordering::Relaxed) + 1;
        JobId::new(n).unwrap_or_else(|| unreachable!("加过一，从 1 数起"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn numbers_follow_the_last_one_used() {
        let fresh = JobIds::starting_after(0);
        assert_eq!(fresh.next().to_string(), "j1");
        assert_eq!(fresh.next().to_string(), "j2");
        let loaded = JobIds::starting_after(7);
        assert_eq!(loaded.next().to_string(), "j8");
    }

    #[test]
    fn calls_running_together_never_share_a_number() {
        let ids = Arc::new(JobIds::starting_after(3));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let ids = Arc::clone(&ids);
                std::thread::spawn(move || (0..100).map(|_| ids.next().get()).collect::<Vec<_>>())
            })
            .collect();
        let mut all: Vec<u64> = threads
            .into_iter()
            .flat_map(|thread| thread.join().unwrap())
            .collect();
        all.sort_unstable();
        assert_eq!(all, (4..=803).collect::<Vec<_>>());
    }
}
