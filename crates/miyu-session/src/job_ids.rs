//! 领任务编号（`docs/blueprint/kernel/ids.md`「任务编号」，施工 7-5）：一个会话 actor 一份，后台命令和子代理共用一串，
//! 从日志里用过的最大编号往下数。几次调用一起跑的，各领各的，不重不漏。
//!
//! 子会话领的号前面带上它自己在父会话里的编号（施工 7-1 补，`session/tools.md`「派子代理」第 1 条）：`j2` 这个子会话领的是
//! `j2.1`、`j2.2`，主会话没有前缀。前缀由造会话、载入的那一头照 `session.created` 的 `cause` 读回来交进来
//! （[`crate::agents::job_in`]）。
//!
//! 编号不回收（`kernel/history.md`）：领了没派成的（造子会话失败、调用被掐掉）也不再发；载入以后照日志里记下的数，没记下
//! 的那几个号没人用过，照样可以再发。

use std::sync::atomic::{AtomicU64, Ordering};

use miyu_kernel::id::JobId;

/// 一个会话的任务编号。
#[derive(Debug)]
pub(crate) struct JobIds {
    /// 这个会话在父会话里的编号：领的号接在它后面。主会话、读不出编号的子会话没有。
    prefix: Option<JobId>,
    /// 最后一段用到了几。
    used: AtomicU64,
}

impl JobIds {
    /// 前缀是 `prefix`，用过的最后一段最大是 `used`（内核的 `last_job_number`，没有是 0），下一个从它加一数起。以前的日志里
    /// 子会话派的 `j1` 照样占着 1：它和前缀接上的 `j2.1` 不是同一个，可是接着数的照最后一段，领的是 `j2.2`，看着不撞。
    pub(crate) fn starting_after(prefix: Option<JobId>, used: u64) -> JobIds {
        JobIds {
            prefix,
            used: AtomicU64::new(used),
        }
    }

    /// 领下一个编号。
    ///
    /// # Panics
    ///
    /// 实际不会：领到 `u64` 的尽头要派十八亿亿次。
    pub(crate) fn next(&self) -> JobId {
        let n = self.used.fetch_add(1, Ordering::Relaxed) + 1;
        match &self.prefix {
            Some(prefix) => prefix.under(n),
            None => JobId::new(n),
        }
        .unwrap_or_else(|| unreachable!("加过一，从 1 数起"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn numbers_follow_the_last_one_used() {
        let fresh = JobIds::starting_after(None, 0);
        assert_eq!(fresh.next().to_string(), "j1");
        assert_eq!(fresh.next().to_string(), "j2");
        let loaded = JobIds::starting_after(None, 7);
        assert_eq!(loaded.next().to_string(), "j8");
    }

    /// 子会话领的号带上它在父会话里的编号，孙会话的三段（施工 7-1 补）；以前的日志里子会话派过 `j1` 的，接着领 `j2.2`。
    #[test]
    fn a_child_session_numbers_under_its_own_job() {
        let job = |text| JobId::parse(text).ok();
        let child = JobIds::starting_after(job("j2"), 0);
        assert_eq!(child.next().to_string(), "j2.1");
        assert_eq!(child.next().to_string(), "j2.2");
        let grandchild = JobIds::starting_after(job("j2.1"), 0);
        assert_eq!(grandchild.next().to_string(), "j2.1.1");
        let loaded = JobIds::starting_after(job("j2.1"), 4);
        assert_eq!(loaded.next().to_string(), "j2.1.5");
        let old = JobIds::starting_after(job("j2"), 1);
        assert_eq!(old.next().to_string(), "j2.2", "旧日志里的 j1 占着 1");
    }

    #[test]
    fn calls_running_together_never_share_a_number() {
        let ids = Arc::new(JobIds::starting_after(JobId::new(5), 3));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let ids = Arc::clone(&ids);
                std::thread::spawn(move || (0..100).map(|_| ids.next()).collect::<Vec<_>>())
            })
            .collect();
        let mut all: Vec<JobId> = threads
            .into_iter()
            .flat_map(|thread| thread.join().unwrap())
            .collect();
        all.sort_unstable();
        let expected: Vec<JobId> = (4..=803)
            .map(|n| JobId::new(5).and_then(|prefix| prefix.under(n)).unwrap())
            .collect();
        assert_eq!(all, expected);
    }
}
