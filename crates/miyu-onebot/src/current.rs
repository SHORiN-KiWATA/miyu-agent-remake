//! 桥手里最新的配置（`onebot.md` 第一条「怎么走」第 1、2 条，施工 O-20）：握手交来的那一份，核心推来 `extension.config` 就照它
//! 换上。NapCat 的监听照它的令牌比，后台页的方法照它答，换端口照它的端口，都不读盘。
//!
//! 换了令牌，已经连着的 NapCat 不断：只有连进来的那一下比令牌。端口变了由 `serve` 另起一个任务换（`crate::running::rebind`，
//! [`Current::change`] 交回变没变）。

use std::sync::{Mutex, MutexGuard, PoisonError};

use serde_json::{Map, Value};

use miyu_config::secret::Secret;

use crate::TARGET;
use crate::settings::{Defaults, Settings};

/// 桥手里最新的配置。NapCat 的监听、后台页的方法、换端口共用一份。
pub(crate) struct Current {
    /// 推来 `null` 的端口照它（清单的默认值）。
    defaults: Defaults,
    /// 最新的那一份。
    settings: Mutex<Settings>,
}

impl Current {
    /// 手里先拿着握手交来的 `settings`；推来 `null` 的端口照 `defaults`。
    pub(crate) fn new(settings: Settings, defaults: Defaults) -> Current {
        Current {
            defaults,
            settings: Mutex::new(settings),
        }
    }

    /// 最新的那一份。
    pub(crate) fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    /// 最新的令牌；没有的是空的。
    pub(crate) fn token(&self) -> Option<Secret> {
        lock(&self.settings).token.clone()
    }

    /// 推来的变化 `keys` 合进来（[`Settings::change`]）。令牌换了记一行，不记值。交回端口变了没有：变了的由调的一方换
    /// （`crate::running::rebind`）。
    pub(crate) fn change(&self, keys: &Map<String, Value>) -> bool {
        let mut settings = lock(&self.settings);
        let before = settings.clone();
        settings.change(keys, &self.defaults);
        if settings.token != before.token {
            tracing::info!(target: TARGET, "token changed");
        }
        settings.port != before.port
    }
}

/// 锁里不 `await`、不会崩；真崩了，里面的照样能用。
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
