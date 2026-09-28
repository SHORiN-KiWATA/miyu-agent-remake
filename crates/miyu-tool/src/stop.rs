//! 叫停的旗（`docs/blueprint/tools/interface.md`「叫停」，施工 4-9 再补一）：执行器「叫它停」时举起来，工具的 future
//! 被丢掉时也举起来。在阻塞线程里干活的工具自己看旗：走目录、搜内容的每一步看一眼；改文件的真正改之前看一眼。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// 一次调用的旗，交给工具的 [`crate::Call`] 带着；克隆出来的是同一面。
#[derive(Debug, Clone, Default)]
pub struct Stop(Arc<AtomicBool>);

impl Stop {
    /// 举起来：叫这次调用停。
    pub fn raise(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// 举起来了没有。
    pub fn stopped(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// 两面旗比的是举没举起来：[`crate::Call`] 照格子比较时用。
impl PartialEq for Stop {
    fn eq(&self, other: &Stop) -> bool {
        self.stopped() == other.stopped()
    }
}

impl Eq for Stop {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clone_is_the_same_flag() {
        let stop = Stop::default();
        let seen_by_the_tool = stop.clone();
        assert!(!seen_by_the_tool.stopped());
        stop.raise();
        assert!(seen_by_the_tool.stopped());
    }
}
