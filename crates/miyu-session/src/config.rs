//! 回合的配置（`docs/blueprint/config.md`「怎么走」第八条第 3 条，G7，施工 8-4）：会话 actor 在记下 `turn.started` 以后、
//! 跑回合开始的挂接点那一刻，从 [`Configs`] 取当前的配置，照会话这时的目录读一次项目配置、合上，得到这一轮的配置。这一轮
//! 的每一次请求（出错再来、压缩的摘要请求也算）都照它；回合之间改的，下一轮才用上。
//!
//! 配置服务在端点（更高一层），会话不反过来引用它：端点每换一次最终值，往 `tokio::sync::watch` 里放一份
//! [`ConfigSource`]，会话从里面取。不开回合的请求（手动压缩、清空单开的那一轮、回顾、起标题）照上一轮的；造会话、载入时
//! 先取一份。

use std::sync::Arc;

use tokio::sync::watch;

use miyu_config::merge::Resolved;

use crate::blocking::blocking;

/// 一份配置：不算项目配置的最终值，和照一个目录带上项目配置再合一次的办法。端点的配置服务实现它。
pub trait ConfigSource: Send + Sync + std::fmt::Debug {
    /// 带上目录 `dir`（头报的写法）的项目配置合出来的最终值：信任着的才算，没有的和不算项目配置的一样。要读磁盘，会话
    /// actor 在阻塞线程里调。
    fn with_project(&self, dir: &str) -> Resolved;
}

/// 当前的配置：配置服务换一次，这里就是新的一份。
pub type Configs = watch::Receiver<Arc<dyn ConfigSource>>;

/// 一轮的配置：回合开始时冻结的那一份。
pub type TurnConfig = Arc<Resolved>;

/// 一份不变的配置：没有配置服务的时候（测试里、自己造的会话），全是 `resolved`。
pub fn fixed(resolved: Resolved) -> Configs {
    let source: Arc<dyn ConfigSource> = Arc::new(Fixed(resolved));
    watch::channel(source).1
}

/// 不变的一份：不看目录。
#[derive(Debug)]
struct Fixed(Resolved);

impl ConfigSource for Fixed {
    fn with_project(&self, _: &str) -> Resolved {
        self.0.clone()
    }
}

/// 一个会话手里的配置：从哪取，和这一轮的那一份。
pub(crate) struct Turning {
    configs: Configs,
    current: TurnConfig,
}

impl Turning {
    /// 造会话、载入时先照目录 `dir` 取一份：回合开始以前的请求（手动压缩、回顾）也有配置用。
    pub(crate) async fn start(configs: Configs, dir: String) -> Turning {
        let current = take(&configs, dir).await;
        Turning { configs, current }
    }

    /// 这一轮的配置。
    pub(crate) fn current(&self) -> &TurnConfig {
        &self.current
    }

    /// 回合开始了：照会话这时的目录 `dir` 换一份新的。
    pub(crate) async fn turn(&mut self, dir: String) {
        self.current = take(&self.configs, dir).await;
    }
}

/// 从 `configs` 取当前的一份，照目录 `dir` 带上项目配置：读磁盘的那一步在阻塞线程里做。
async fn take(configs: &Configs, dir: String) -> TurnConfig {
    let source = Arc::clone(&*configs.borrow());
    blocking(move || Arc::new(source.with_project(&dir))).await
}
