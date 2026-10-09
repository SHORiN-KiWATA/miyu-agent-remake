//! 装卸软件包以后换配置清单（施工 F-5 补，`docs/blueprint/config.md`「怎么走」第一条第 1 款，`packages.md`「配置项」
//! 第 2 条）：换上新的清单，系统配置、个人设置照手里的字重新认（不重读盘：盘上的手改照监视那一条走），合出新的最终值交给
//! 会话、核心。哪一层认得的项、问题变了，推 `config.changed`（`via: package`，不带 `by`），记一条 `INFO config changed`；
//! 不记配置日志：文件没人改。

use miyu_config::Item;

use super::file::File;
use super::hub::Pushing;
use super::observe::differences;
use super::push::Via;
use super::{Config, TARGET};
use crate::Core;

impl Config {
    /// 换上清单 `items`：两层照手里的字重新认，最终值跟着重算。交回换以前的两层。带 `env` 的只有核心自己的项（`log.level`），
    /// 换了清单也还在：起来时取的环境变量照旧。
    fn refit(&mut self, items: Vec<Item>) -> [File; 2] {
        self.items = items;
        let system = self.system.refit(&self.items);
        let personal = self.personal.refit(&self.items);
        let old = [
            std::mem::replace(&mut self.system, system),
            std::mem::replace(&mut self.personal, personal),
        ];
        self.resolved = self.merged(None);
        old
    }
}

/// 核心照新的配置清单 `items` 换（[`Core::reload_packages`] 调）：和手里的一样的什么都不做。
pub(crate) fn refit(core: &Core, items: Vec<Item>) {
    let mut config = core.config();
    if config.items() == items.as_slice() {
        return;
    }
    let old = config.refit(items);
    let mut pushed = false;
    for old in &old {
        let new = config.file(old.layer);
        let changes = differences(old, new);
        if changes.is_empty() && old.problems().eq(new.problems()) {
            continue;
        }
        let keys: Vec<String> = changes.into_iter().map(|(key, _, _)| key).collect();
        tracing::info!(
            target: TARGET,
            layer = %old.layer.as_str(),
            via = %Via::Package.as_str(),
            keys = %keys.join(","),
            "config changed"
        );
        let pushing = Pushing {
            layer: old.layer,
            via: Via::Package,
            by: None,
            keys,
        };
        core.hub.publish(&config, Some(pushing));
        pushed = true;
    }
    if !pushed {
        core.hub.publish(&config, None);
    }
}
