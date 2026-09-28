//! 运行日志（`docs/designs/28-运行日志.md`，施工 3-7 上）：核心和子进程在干什么，一行一条的英文，
//! 像 dmesg；写进数据根的 `state/logs/`，满 10 MB 换一份；默认记到 `INFO`，`MIYU_LOG` 改这一次启动的
//! 级别。不写对话的内容，不写密钥（LG3）：发日志的地方只给编号、长度、状态。
//!
//! 各 crate 照 `tracing` 这个门面发，目标一律写成 `miyu::<来源>`，例如 `miyu::http`；怎么写成一行、
//! 写到哪，只有这里管：
//!
//! - [`install`]：程序入口装一次，交回 [`Guard`]；
//! - [`level()`]：`MIYU_LOG` 的值怎么读；
//! - [`RotatingFile`]：按大小轮换的文件；
//! - [`LineLayer`]：把一条事件写成一行，交给 [`Sink`]。测试里拿 [`Memory`] 接住。

mod home;
mod layer;
mod level;
mod line;
mod rotate;

pub use layer::{LineLayer, Memory, Sink};
pub use level::{Level, level};
pub use line::utc_offset;
pub use rotate::RotatingFile;
pub use tracing_subscriber::filter::LevelFilter;

use std::io;
use std::path::Path;
use std::sync::Arc;

use tracing::subscriber::SetGlobalDefaultError;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;

/// 一份的上限：满了换下一份（`28-运行日志.md` 第一节）。
pub const LIMIT: u64 = 10 * 1024 * 1024;

/// 正在写的之外留几份：`core.log.1` 到 `core.log.5`。
pub const KEEP: usize = 5;

/// 装上了：留着它，日志就一直写，退出前丢掉它。现在每一行直接交给系统，没有攒着没写的；以后换成
/// 后台写（施工单 3-7 上「风险」），丢掉它时等后台写完。
#[derive(Debug)]
pub struct Guard {
    file: Arc<RotatingFile>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.file.flush();
    }
}

/// 装不上。
#[derive(Debug)]
pub enum InstallError {
    /// 建不了日志的目录，或者打不开文件。
    Io(io::Error),
    /// 这个进程已经装过一次了。
    Twice(SetGlobalDefaultError),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::Io(error) => write!(f, "运行日志写不了：{error}"),
            InstallError::Twice(error) => write!(f, "运行日志已经装过了：{error}"),
        }
    }
}

impl std::error::Error for InstallError {}

/// 装上运行日志：写进 `dir` 下的 `<name>.log`（核心是 `core`），照 `MIYU_LOG` 的值 `value` 定级别。
/// 值读不懂的，照 `INFO` 记，再记一条 `WARN` 写明读不懂的是什么。路径里的家目录 `home` 写成 `~`。
///
/// # Errors
///
/// 建不了目录、打不开文件；这个进程已经装过了。
pub fn install(
    dir: &Path,
    name: &str,
    value: Option<&str>,
    home: Option<&Path>,
) -> Result<Guard, InstallError> {
    let file = Arc::new(RotatingFile::open(dir, name, LIMIT, KEEP).map_err(InstallError::Io)?);
    let chosen = level(value);
    tracing::subscriber::set_global_default(subscriber(file.clone(), chosen.filter, home))
        .map_err(InstallError::Twice)?;
    if let Some(unknown) = chosen.unknown {
        tracing::warn!(target: "miyu::log", value = %unknown, "MIYU_LOG not understood, using info");
    }
    Ok(Guard { file })
}

/// 一个订阅者：自己的（`miyu::` 开头的目标）照 `filter` 记，别人家的（`hyper`、`reqwest` 这些）最多记
/// 到 `WARN`，免得调到 `DEBUG` 时被它们刷屏；`filter` 比 `WARN` 还严的，别人家的也照它（`off` 就是
/// 什么都不记）。路径里的家目录 `home` 写成 `~`，没有的不换。
pub fn subscriber(
    sink: Arc<dyn Sink>,
    filter: LevelFilter,
    home: Option<&Path>,
) -> impl tracing::Subscriber + Send + Sync {
    with_clock(sink, filter, line::now, home)
}

/// 同 [`subscriber`]，时刻照 `clock` 给的：测试里定住它。
pub(crate) fn with_clock(
    sink: Arc<dyn Sink>,
    filter: LevelFilter,
    clock: fn() -> String,
    home: Option<&Path>,
) -> impl tracing::Subscriber + Send + Sync {
    let targets = Targets::new()
        .with_target("miyu", filter)
        .with_default(filter.min(LevelFilter::WARN));
    tracing_subscriber::registry()
        .with(targets)
        .with(LineLayer::with_clock(sink, clock).home(home))
}
