//! 主题的测试夹具。

use std::sync::{Mutex, MutexGuard};

static LOCK: Mutex<()> = Mutex::new(());

/// 换主题的测试、数「换过几次主题」的测试都先拿这把锁：主题只有一份，并行跑时一个换了，另一个数的就不准。
pub fn hold() -> MutexGuard<'static, ()> {
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
