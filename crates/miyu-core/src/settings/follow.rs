//! 配置换了当场生效的几样（`docs/blueprint/config.md`「怎么走」第八条第 1 条，施工 8-4）：配置服务每换上一份新的，
//!
//! - `log.level` 的最终值变了：经运行日志的重载把手换级别，记一条 `INFO log level`。`MIYU_LOG` 设了、读得懂的，最终值
//!   一直是它，配置怎么改都不换（`log.md` 第 3 条）。
//! - `ui.language` 照核心这边的系统语言算出的那一种变了：照新的语言重写生成的三份（第一条第 12 条）。`auto` 换成系统本来
//!   就是的那一种，字一样，不重写。
//!
//! 别的 `now` 项由用它的地方自己跟：连接每说一句照这时的 `ui.language` 重算（端点）。

use std::sync::Arc;

use tokio::sync::watch;

use miyu_endpoint::config::Config;
use miyu_endpoint::settings::UiSettings;
use miyu_log::Levels;
use miyu_log::settings::LogSettings;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use super::{generate, set_level};

/// 当场生效的那几样现在是什么。
#[derive(Debug, PartialEq, Eq)]
struct Applied {
    /// `log.level` 的最终值。
    level: String,
    /// 生成的文件用哪种语言。
    language: String,
}

impl Applied {
    fn of(config: &Config, locale: Option<&str>) -> Applied {
        let values = config.resolved().values();
        Applied {
            level: LogSettings::from(&values).level,
            language: UiSettings::from(&values).language_for(locale).to_string(),
        }
    }
}

/// 跟着配置服务 `now`，直到它没了：级别换在 `levels` 上，生成的文件写进 `root` 的状态区，字从 `resources` 读，`auto` 的照
/// 核心这边的系统语言 `locale`。
///
/// 现在是什么在调的这一刻就记下（不等交回的任务跑起来）：任务起来之前换的也看得到。
pub fn follow(
    mut now: watch::Receiver<Arc<Config>>,
    levels: Levels,
    root: DataRoot,
    resources: ResourceRoot,
    locale: Option<String>,
) -> impl Future<Output = ()> + Send {
    let last = Applied::of(&now.borrow_and_update(), locale.as_deref());
    watching(now, last, levels, root, resources, locale)
}

/// 等配置换，照 [`follow()`] 说的办。
async fn watching(
    mut now: watch::Receiver<Arc<Config>>,
    mut last: Applied,
    levels: Levels,
    root: DataRoot,
    resources: ResourceRoot,
    locale: Option<String>,
) {
    while now.changed().await.is_ok() {
        let config = Arc::clone(&now.borrow_and_update());
        let next = Applied::of(&config, locale.as_deref());
        if next.level != last.level {
            set_level(&config, &levels);
        }
        if next.language != last.language {
            let (root, resources, locale) = (root.clone(), resources.clone(), locale.clone());
            let values = config.resolved().values();
            let written = tokio::task::spawn_blocking(move || {
                generate(&root, &resources, locale.as_deref(), &values);
            });
            if let Err(error) = written.await {
                tracing::error!(target: super::TARGET, error = %error, "config schema writer panicked");
            }
        }
        last = next;
    }
}
