//! 在阻塞线程里干碰文件的活（施工 4-4 下）：不占着跑异步任务的线程。
//!
//! 叫停一次调用，就是丢掉它的 future（施工 4-2），可阻塞线程里已经在跑的活自己不会停：`glob`、`grep` 从 `/`
//! 往下走，能走很久。所以交给活一面旗 [`Stop`]，future 被丢掉时旗就举起来，走目录、搜内容的每一步看一眼。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// 这次调用叫停了没有。
#[derive(Clone, Default)]
pub(crate) struct Stop(Arc<AtomicBool>);

impl Stop {
    /// 叫停了：活可以不干完就交回，交回的也没人要了。
    pub(crate) fn stopped(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// 丢掉它就举旗。
struct Raise(Arc<AtomicBool>);

impl Drop for Raise {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// 在阻塞线程里跑 `work`，交回它的结果。这次调用被叫停（这个 future 被丢掉）时，`work` 拿到的旗举起来。
/// `work` 里 panic 了，这里照样 panic，执行器认得出工具崩了。
pub(crate) async fn blocking<T: Send + 'static>(
    work: impl FnOnce(&Stop) -> T + Send + 'static,
) -> T {
    let stop = Stop::default();
    let _raise = Raise(Arc::clone(&stop.0));
    match tokio::task::spawn_blocking(move || work(&stop)).await {
        Ok(value) => value,
        Err(error) => std::panic::resume_unwind(error.into_panic()),
    }
}
