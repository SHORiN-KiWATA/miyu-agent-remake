//! 一个个接连接，空闲够久了、收到停的信号就停（`docs/designs/12-进程形态与分发.md` 第二节「按需运行」，
//! 施工 3-9 上）。

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::time::MissedTickBehavior;

use miyu_endpoint::Core;
use miyu_ipc::Listener;

use crate::TARGET;

/// 为什么停了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    /// 没有连接、也没有在跑的回合，空闲够久了。
    Idle,
    /// 收到停的信号：在跑的会话先有计划地停下了。
    Signal,
}

impl Stopped {
    /// 记进运行日志的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Stopped::Idle => "idle",
            Stopped::Signal => "signal",
        }
    }
}

/// 接连接，直到空闲够久了（没有连接、也没有在跑的回合，连续 `idle` 这么久），或者 `stop` 来了。`stop`
/// 来了的，先让在跑的会话有计划地停下：跑到一半的回合记成「重启了」，下次载入接着干。停之前请扩展退出（施工 9-4 上）。
///
/// 停之前记一条「停了」：那时锁还在手里，下一个核心起来之前，这一份日志还是它一个人写。交回以后，
/// 监听器、锁、各个连接都没了。
pub async fn serve(
    listener: Listener,
    core: Arc<Core>,
    idle: Duration,
    stop: impl Future<Output = ()>,
) -> Stopped {
    let running = miyu_endpoint::run(listener, Arc::clone(&core));
    tokio::pin!(running, stop);
    let mut ticks = tokio::time::interval(check(idle));
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut quiet: Option<Instant> = None;
    let stopped = loop {
        tokio::select! {
            never = &mut running => match never {},
            () = &mut stop => {
                core.stop_sessions().await;
                break Stopped::Signal;
            }
            _ = ticks.tick() => {
                if !core.idle().await {
                    quiet = None;
                    continue;
                }
                let since = *quiet.get_or_insert_with(Instant::now);
                if since.elapsed() >= idle {
                    break Stopped::Idle;
                }
            }
        }
    };
    // 扩展随核心退出（施工 9-4 上）：关它们的标准输入，一起等，没退的杀掉。
    core.stop_extensions().await;
    tracing::info!(target: TARGET, reason = stopped.as_str(), "stopped");
    stopped
}

/// 多久看一次空不空闲：空闲时限的四分之一，最多 30 秒，最少 100 毫秒。不另起忙循环。
fn check(idle: Duration) -> Duration {
    (idle / 4).clamp(Duration::from_millis(100), Duration::from_secs(30))
}

/// 停的信号：Ctrl+C（SIGINT），Unix 上还有 SIGTERM。拉起的核心自成一个进程组，终端里按 Ctrl+C 打不到它，
/// 要停它得明着发。装不上的当不会来：Ctrl+C 装不上时照样等 SIGTERM（施工 4-9 再补三上，原来当成收到了，马上
/// 停）。
pub(crate) async fn signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    () = interrupt() => {}
                    _ = terminate.recv() => {}
                }
            }
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "SIGTERM not watched");
                interrupt().await;
            }
        }
    }
    #[cfg(not(unix))]
    interrupt().await;
}

/// 等 Ctrl+C。装不上的一直等下去。
async fn interrupt() {
    watched(tokio::signal::ctrl_c()).await;
}

/// 等 Ctrl+C：`installed` 是装上它的 future。装不上（交回出错）的，记一条 `WARN`，当它不会来，一直等下去。
async fn watched(installed: impl Future<Output = std::io::Result<()>>) {
    if let Err(error) = installed.await {
        tracing::warn!(target: TARGET, error = %error, "Ctrl+C not watched");
        std::future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests;
