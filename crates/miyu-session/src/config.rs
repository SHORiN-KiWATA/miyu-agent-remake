//! 回合的配置（`docs/blueprint/config.md`「怎么走」第八条第 3 条，G7，施工 8-4）：会话 actor 在记下 `turn.started` 以后、
//! 跑回合开始的挂接点那一刻，从 [`Configs`] 取当前的配置，照会话这时的目录读一次项目配置、合上，得到这一轮的配置。这一轮
//! 的每一次请求（出错再来、压缩的摘要请求也算）都照它；回合之间改的，下一轮才用上。
//!
//! 配置服务在端点（更高一层），会话不反过来引用它：端点每换一次最终值，往 `tokio::sync::watch` 里放一份
//! [`ConfigSource`]，会话从里面取。不开回合的请求（手动压缩、清空单开的那一轮、回顾、起标题）照上一轮的；造会话、载入时
//! 先取一份。
//!
//! 这一轮的配置连同取密钥的办法一起冻结（[`Turn`]，施工 8-6）：路由照它挑供应商、照 `{ secret }`、`{ env }` 取 key，回合
//! 之间换了的 key 下一轮才用上（`config.md` 第九条第 8 条）。

use std::sync::Arc;

use tokio::sync::watch;

use miyu_config::Layer;
use miyu_config::merge::Resolved;
use miyu_config::secret::{Reference, Secret};

use crate::blocking::blocking;

/// 一份配置：不算项目配置的最终值，和照一个目录带上项目配置再合一次的办法。端点的配置服务实现它。
pub trait ConfigSource: Send + Sync + std::fmt::Debug {
    /// 带上目录 `dir`（头报的写法）的项目配置合出来的最终值：信任着的才算，没有的和不算项目配置的一样。要读磁盘，会话
    /// actor 在阻塞线程里调。
    fn with_project(&self, dir: &str) -> Resolved;

    /// 照引用取一个密钥（施工 8-6）：`{ secret }` 照密钥文件，`{ env }` 照核心的环境。没设的、设成空的是空的。
    fn secret(&self, reference: &Reference) -> Option<Secret>;

    /// 一层的配置是哪份文件，数据根里的相对路径（施工 8-15）：手写的价格记进 `cost.source` 时照它写
    /// （`config:<文件>:<行>`），和 `model.list` 的来源同一份。不认数据根的（测试里不变的配置）写层的名字。
    fn file(&self, layer: Layer) -> String {
        layer.as_str().to_string()
    }
}

/// 当前的配置：配置服务换一次，这里就是新的一份。
pub type Configs = watch::Receiver<Arc<dyn ConfigSource>>;

/// 一轮的配置：回合开始时冻结的那一份。
pub type TurnConfig = Arc<Turn>;

/// 冻结的一轮：最终值，和取密钥的那一份配置（配置服务换上的每一份都是不变的，拿着它就是冻结了）。
#[derive(Debug)]
pub struct Turn {
    /// 带上项目配置合出来的最终值。
    pub resolved: Resolved,
    /// 取密钥的那一份。
    source: Arc<dyn ConfigSource>,
}

impl Turn {
    /// 冻结一份：最终值 `resolved`，key 照 `source` 取（施工 8-20：一次性调用照这一刻不算项目配置的最终值，端点交进来）。
    pub fn new(resolved: Resolved, source: Arc<dyn ConfigSource>) -> Turn {
        Turn { resolved, source }
    }

    /// 照引用取一个密钥：这一轮开始时的那一份。
    pub fn secret(&self, reference: &Reference) -> Option<Secret> {
        self.source.secret(reference)
    }

    /// 一层的配置是哪份文件（施工 8-15，[`ConfigSource::file`]）。
    pub fn file(&self, layer: Layer) -> String {
        self.source.file(layer)
    }
}

/// 一份不变的配置：没有配置服务的时候（测试里、自己造的会话），全是 `resolved`，一个密钥都取不到。
pub fn fixed(resolved: Resolved) -> Configs {
    fixed_with(resolved, Vec::new())
}

/// 同 [`fixed`]，另带几个取得到的密钥：引用和它的值（测试里配供应商的 key 用）。
pub fn fixed_with(resolved: Resolved, secrets: Vec<(Reference, Secret)>) -> Configs {
    let source: Arc<dyn ConfigSource> = Arc::new(Fixed(resolved, secrets));
    watch::channel(source).1
}

/// 不变的一份：不看目录。
struct Fixed(Resolved, Vec<(Reference, Secret)>);

impl std::fmt::Debug for Fixed {
    /// 密钥不印。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Fixed")
            .field(&self.0)
            .finish_non_exhaustive()
    }
}

impl ConfigSource for Fixed {
    fn with_project(&self, _: &str) -> Resolved {
        self.0.clone()
    }

    fn secret(&self, reference: &Reference) -> Option<Secret> {
        self.1
            .iter()
            .find(|(written, _)| written == reference)
            .map(|(_, secret)| secret.clone())
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
    blocking(move || {
        let resolved = source.with_project(&dir);
        Arc::new(Turn { resolved, source })
    })
    .await
}
