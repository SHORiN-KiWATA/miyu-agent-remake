//! 桥手里的令牌（`onebot.md` 第一条「怎么走」第 2 条、第二条「施工时定的」第 6、18 条，施工 O-16 补二）：起来时读到的那个；
//! 之后每次重读配置都照读到的换上，令牌改了不用重启。
//!
//! 谁来重读：WebUI 的 `/status`、`/token`、`/apply` 每次都读；NapCat 出示的令牌对不上（或者手里还没有）时也读，但有节流
//! （[`Current::due`]）：离上一次这样重读不到 `bridge.json` 的 `reload_seconds` 的不读，照手里的比，乱连的不能把读配置变成
//! 负担。配置读不出来（端口坏了，走不到）的，手里的不动。
//!
//! 换了令牌，已经连着的 NapCat 不断：只有连进来的那一下比令牌。

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::TARGET;
use crate::settings::{Reload, Settings, Token};

/// 桥手里的令牌，和怎么重读。NapCat 的监听、WebUI 共用一份。
pub(crate) struct Current {
    /// 重读配置。
    reload: Reload,
    /// 手里的令牌。
    token: Mutex<Token>,
    /// NapCat 对不上时，两次重读至少隔多久。
    every: Duration,
    /// NapCat 对不上时上一次重读的时刻；还没读过的是空的。
    last: Mutex<Option<Instant>>,
}

impl Current {
    /// 手里先拿着起来时读到的 `token`；NapCat 对不上时至少隔 `every` 才重读。
    pub(crate) fn new(reload: Reload, token: Token, every: Duration) -> Current {
        Current {
            reload,
            token: Mutex::new(token),
            every,
            last: Mutex::new(None),
        }
    }

    /// 手里的令牌。
    pub(crate) fn token(&self) -> Token {
        lock(&self.token).clone()
    }

    /// 重读一次配置（在阻塞线程里读文件）：读得出来的，手里的令牌换成读到的，交回读到的；读不出来的手里的不动，记一行运行
    /// 日志，交回空的。令牌换了记一行，不记值。
    pub(crate) async fn reload(&self) -> Option<Settings> {
        let reload = Arc::clone(&self.reload);
        let read = match tokio::task::spawn_blocking(move || reload()).await {
            Ok(Ok(settings)) => settings,
            Ok(Err(unready)) => {
                tracing::warn!(target: TARGET, unready = ?unready, "config not readable");
                return None;
            }
            Err(error) => {
                tracing::error!(target: TARGET, error = %error, "config reload panicked");
                return None;
            }
        };
        let mut token = lock(&self.token);
        if *token != read.token {
            tracing::info!(target: TARGET, "token changed");
            *token = read.token.clone();
        }
        Some(read)
    }

    /// NapCat 对不上，现在（`now`）该不该重读：离上一次这样重读够了 `every` 的该读，记下这一次。
    pub(crate) fn due(&self, now: Instant) -> bool {
        let mut last = lock(&self.last);
        if last.is_some_and(|last| now.saturating_duration_since(last) < self.every) {
            return false;
        }
        *last = Some(now);
        true
    }
}

/// 锁里不 `await`、不会崩；真崩了，里面的照样能用。
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
