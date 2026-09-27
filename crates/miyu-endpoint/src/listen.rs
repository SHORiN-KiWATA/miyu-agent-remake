//! 在监听器上一个个接连接，每个连接交给 [`serve`]（施工 3-8 下）。

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use miyu_ipc::Listener;
use tokio::task::JoinSet;

use crate::{Core, serve};

/// 接不了连接以后歇多久再接：打开的文件太多这类错一时好不了，不歇会空转。
const PAUSE: Duration = Duration::from_millis(100);

/// 在 `listener` 上一个个接连接，每个交给 [`serve`]，一直接下去。不会自己停：要停就丢掉它，监听器、
/// 锁、各个连接跟着一起没了。
pub async fn run(mut listener: Listener, core: Arc<Core>) -> Infallible {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(connection) => {
                    tracing::debug!(target: "miyu::endpoint", "connected");
                    connections.spawn(serve(connection, Arc::clone(&core)));
                }
                Err(error) => {
                    tracing::warn!(target: "miyu::endpoint", error = %error, "accept failed");
                    tokio::time::sleep(PAUSE).await;
                }
            },
            Some(done) = connections.join_next() => {
                if let Err(error) = done {
                    tracing::error!(target: "miyu::endpoint", error = %error, "connection task failed");
                }
            }
        }
    }
}
