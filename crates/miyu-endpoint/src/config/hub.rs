//! 配置换了交给谁（`docs/blueprint/config.md`「怎么走」第八条，施工 8-4）：配置服务每换上一份新的，
//!
//! - 交给会话：往 `tokio::sync::watch` 里放一份，会话在下一个回合开始时从里面取（[`Hub::configs`]）；
//! - 交给核心：同一个 `watch`，核心照它当场换运行日志的级别、重写生成的文件（[`Hub::current`]）；
//! - 有要推的（系统配置、个人设置变了），推给订阅着配置的连接（[`Hub::subscribe`]）。读得慢的掉队，推 `resync`。

use std::sync::Arc;

use tokio::sync::{broadcast, watch};

use miyu_session::{ConfigSource, Configs};

use super::Config;
use super::push::Changed;

/// 推送的队列：一个连接最多攒这么多条还没写出去的，再多就掉队（`04-核心协议.md` 第七节）。配置很少改，攒不了几条。
pub(crate) const PUSH_QUEUE: usize = 16;

/// 配置换了交给谁。
#[derive(Debug)]
pub(crate) struct Hub {
    /// 推给订阅着配置的连接。
    pushes: broadcast::Sender<Arc<Changed>>,
    /// 当前的一份：会话、核心取。
    current: watch::Sender<Arc<Config>>,
    /// 同一份，写成会话认的样子。
    sessions: watch::Sender<Arc<dyn ConfigSource>>,
}

impl ConfigSource for Config {
    fn with_project(&self, dir: &str) -> miyu_config::merge::Resolved {
        Config::with_project(self, dir).0
    }
}

impl Hub {
    /// 从 `config` 开始。
    pub(crate) fn new(config: &Config) -> Hub {
        let now = Arc::new(config.clone());
        let source: Arc<dyn ConfigSource> = now.clone();
        Hub {
            pushes: broadcast::channel(PUSH_QUEUE).0,
            current: watch::channel(now).0,
            sessions: watch::channel(source).0,
        }
    }

    /// 会话取配置的那一头：造会话、载入时交给它。
    pub(crate) fn configs(&self) -> Configs {
        self.sessions.subscribe()
    }

    /// 核心取配置的那一头。
    pub(crate) fn current(&self) -> watch::Receiver<Arc<Config>> {
        self.current.subscribe()
    }

    /// 订阅推送。
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<Arc<Changed>> {
        self.pushes.subscribe()
    }

    /// 配置服务换上了 `config`：交给会话、核心；`changed` 有的（系统配置、个人设置变了）推出去，推的带着这一份。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "没有订阅者就没人收，照常往下走"
    )]
    pub(crate) fn publish(&self, config: &Config, changed: Option<Pushing>) {
        let now = Arc::new(config.clone());
        let source: Arc<dyn ConfigSource> = now.clone();
        self.sessions.send_replace(source);
        self.current.send_replace(Arc::clone(&now));
        if let Some(pushing) = changed {
            let _ = self.pushes.send(Arc::new(Changed {
                config: now,
                layer: pushing.layer,
                via: pushing.via,
                by: pushing.by,
                keys: pushing.keys,
            }));
        }
    }
}

/// 要推的一次：哪一层、怎么改的、谁改的、变了的项。
#[derive(Debug)]
pub(crate) struct Pushing {
    /// 哪一层。
    pub(crate) layer: miyu_config::Layer,
    /// 怎么改的。
    pub(crate) via: super::push::Via,
    /// 谁改的：`via` 是 `set`、`edit` 才有。
    pub(crate) by: Option<miyu_kernel::origin::By>,
    /// 这一层变了的项，照键名排。
    pub(crate) keys: Vec<&'static str>,
}
