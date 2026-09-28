//! 在阻塞线程里做完磁盘上的事（`02-内核.md` 第七节「会话 actor 怎么跑」）：读写、同步可能要几十毫秒，
//! 不占异步线程。

/// 在阻塞线程里做完，交回结果。阻塞的那一头 panic 了，照原样接着 panic：那是 bug。阻塞线程带着派活时的 span，
/// 那边发的行也有会话编号（施工 4-9 再补四上）。
pub(crate) async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let span = tracing::Span::current();
    match tokio::task::spawn_blocking(move || span.in_scope(work)).await {
        Ok(value) => value,
        Err(error) => std::panic::resume_unwind(error.into_panic()),
    }
}
