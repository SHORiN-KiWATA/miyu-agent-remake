//! 配置清单（`docs/blueprint/config.md`「怎么走」第一、二条，施工 8-1、8-2）：各模块在自己的 crate 里声明自己的几项，
//! 这里登记成一张表；核心起来时读配置（[`read`]），照最终值定运行日志的级别（[`log_level`]），再生成两份 JSON Schema
//! 和参考文件，放在 `state/config/`（[`generate`]）。
//!
//! 生成的三份是派生的：一样的不重写，写不成的记一条 `WARN config schema not written`，照样起来，缺了只是编辑器没有
//! 补全。字照管理员的 `ui.language` 的最终值，`auto` 的照核心所在系统的语言。
//!
//! 运行中配置换了（施工 8-4，[`follow()`]）：`log.level` 变了当场换级别，`ui.language` 变了照新的语言重写这三份。

use std::path::Path;

use miyu_config::merge::Origin;
use miyu_config::{Item, Layer, Missing, Values, Words, reference, schema};
use miyu_endpoint::config::{Config, Environment};
use miyu_endpoint::settings::{PermissionSettings, UiSettings};
use miyu_kernel::id::AccountId;
use miyu_log::settings::LogSettings;
use miyu_log::{Guard, Level, Levels};
use miyu_store::generated;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

/// 运行日志的目标（`config.md`「出错」）。
const TARGET: &str = "miyu::config";

/// 登记的模块，照这个先后，一个模块里照声明的先后。加一个模块只加一行。设置页的页照第一次出现的先后排：通用、界面、
/// 权限、高级（施工 8-2、8-3）。
const MODULES: [&[Item]; 4] = [
    UiSettings::ITEMS,
    TuiSettings::ITEMS,
    PermissionSettings::ITEMS,
    LogSettings::ITEMS,
];

miyu_config::settings! {
    /// 终端界面的配置（施工 8-3，2026-10-01 项目主人要）。终端界面还没进工作区（M9），它的几项先在这里替它声明、登记：
    /// 头自己经 `config.get` 读，核心不管它。终端界面并进来以后挪到它自己的 crate 里。
    pub struct TuiSettings in "tui" {
        /// 终端界面启动时开哪个会话：`new` 开一个新的，`recent` 接着最近的那一个。
        startup: String = "new" {
            kind: option ["new", "recent"],
            layers: [System, Personal],
            applies: head_start,
            ui: { page: "interface", group: "tui", control: select },
        },
    }
}

/// 生成的三份放在状态区的这个目录里：`state/config/`。
const DIR: &str = "config";

/// 登记的全部配置项。核心起来时合成一次，之后不变。
pub fn items() -> Vec<Item> {
    MODULES
        .iter()
        .flat_map(|items| items.iter().cloned())
        .collect()
}

/// 生成的三份的文件名，和 [`render`] 交回的先后一样：系统配置的 Schema、个人设置的 Schema、参考文件。
pub const FILES: [&str; 3] = [
    "config.schema.json",
    "settings.schema.json",
    "reference.toml",
];

/// 照清单 `items`、字 `words` 生成的三份，先后照 [`FILES`]。系统配置的 Schema 只有能放进系统配置的项，个人设置的
/// 同理，参考文件是全部。
pub fn render(items: &[Item], words: &dyn Words) -> [Result<String, Missing>; 3] {
    [
        schema::render(items, Layer::System, words),
        schema::render(items, Layer::Personal, words),
        reference::render(items, words),
    ]
}

/// 核心起来时读配置（第二条第 1 条）：系统配置、管理员 `admin` 的个人设置、信任的记录、密钥文件（施工 8-5），照登记的
/// 全部清单认，带 `env` 的项、`{ env = … }` 照进程的环境变量。`home` 是系统的家目录。读不进来不影响起不起得来：有问题的每份记一条 `WARN`。
pub fn read(root: &DataRoot, admin: &AccountId, home: Option<&Path>) -> Config {
    Config::load(root, admin, home, items(), Environment::process())
}

/// 读完配置，照 `log.level` 的最终值换运行日志的级别（第二条第 7 条，`log.md`）：`MIYU_LOG` 设了、读得懂的照它（装日志时
/// 就照它了），读不懂的记一条 `WARN MIYU_LOG not understood, using config`，照配置。再记一条 `INFO log level`：级别和
/// 从哪来（`env`、`config`、`default`）。`from_env` 是装日志时读的 `MIYU_LOG`。
pub fn log_level(config: &Config, from_env: &Level, log: &Guard) {
    if let Some(unknown) = &from_env.unknown {
        tracing::warn!(
            target: TARGET,
            value = %unknown,
            "MIYU_LOG not understood, using config"
        );
    }
    set_level(config, &log.levels());
}

/// 照 `config` 里 `log.level` 的最终值换级别，记一条 `INFO log level`：级别和从哪来。
fn set_level(config: &Config, levels: &Levels) {
    let resolved = config.resolved();
    let settings = LogSettings::from(&resolved.values());
    let from = match resolved.get("log.level").map(|(_, origin)| origin) {
        Some(Origin::Env(_)) => "env",
        Some(Origin::File { .. }) => "config",
        _ => "default",
    };
    levels.set(miyu_log::level(Some(&settings.level)).filter);
    tracing::info!(target: TARGET, level = %settings.level, from, "log level");
}

/// 核心起来时写生成的三份：字照 `ui.language` 的最终值 `values`，`auto` 的照系统的语言 `locale`。写不成的一份记一条
/// `WARN`，不影响起不起得来。
pub fn generate(root: &DataRoot, resources: &ResourceRoot, locale: Option<&str>, values: &Values) {
    let items = items();
    let ui = UiSettings::from(values);
    let texts = match Human::load(resources, ui.language_for(locale)) {
        Ok(words) => render(&items, &words).map(|text| text.map_err(|error| error.to_string())),
        Err(error) => FILES.map(|_| Err(error.to_string())),
    };
    let dir = root.state().join(DIR);
    for (name, text) in FILES.into_iter().zip(texts) {
        let written = text.and_then(|text| {
            generated::write(&dir.join(name), text.as_bytes()).map_err(|error| error.to_string())
        });
        if let Err(error) = written {
            tracing::warn!(
                target: TARGET,
                file = %format!("state/{DIR}/{name}"),
                error = %error,
                "config schema not written"
            );
        }
    }
}

mod follow;

pub use follow::follow;

#[cfg(test)]
mod tests;
