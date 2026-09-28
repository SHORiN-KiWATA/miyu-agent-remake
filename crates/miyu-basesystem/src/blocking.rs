//! 在阻塞线程里干碰文件的活（施工 4-4 下）：不占着跑异步任务的线程。
//!
//! 阻塞线程里已经在跑的活自己不会停：`glob`、`grep` 从 `/` 往下走，能走很久；`write`、`edit`、`trash` 改到一半的
//! 不能半途而废。所以活拿着这次调用的旗（[`Stop`]，施工 4-9 再补一）：执行器「叫它停」时举起来，future 被丢掉
//! （「掐掉」）时也举起来。走目录、搜内容的每一步看一眼，改文件的真正改之前看一眼。

use miyu_tool::Stop;

/// 丢掉它就举旗。
struct Raise(Stop);

impl Drop for Raise {
    fn drop(&mut self) {
        self.0.raise();
    }
}

/// 在阻塞线程里跑 `work`，交回它的结果。`work` 拿到的是这次调用的旗 `stop`；这个 future 被丢掉时旗也举起来。
/// `work` 里 panic 了，这里照样 panic，执行器认得出工具崩了。阻塞线程带着派活时的 span，那边发的行也有会话编号
/// （施工 4-9 再补四上）。
pub(crate) async fn blocking<T: Send + 'static>(
    stop: Stop,
    work: impl FnOnce(&Stop) -> T + Send + 'static,
) -> T {
    let _raise = Raise(stop.clone());
    let span = tracing::Span::current();
    match tokio::task::spawn_blocking(move || span.in_scope(|| work(&stop))).await {
        Ok(value) => value,
        Err(error) => std::panic::resume_unwind(error.into_panic()),
    }
}
