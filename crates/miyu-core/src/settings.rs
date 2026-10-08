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
use miyu_config::package::Manifest;
use miyu_config::{Item, Layer, Missing, Values, Words, reference, schema};
use miyu_endpoint::config::{Config, Environment};
use miyu_endpoint::settings::{
    EXTERNAL_BINDINGS, PermissionSettings, PersonaSettings, PresetSettings, UiSettings,
};
use miyu_kernel::id::AccountId;
use miyu_log::settings::LogSettings;
use miyu_log::{Guard, Level, Levels};
use miyu_models::settings::{
    AuthCooldown, CatalogSettings, ModelSettings, PoolSettings, PriceSettings, ProviderSettings,
    RateLimitedCooldown, RetryableCooldown, UsageSettings, UseSettings,
};
use miyu_session::settings::CompactionSettings;
use miyu_store::generated;
use miyu_store::human::Human;
use miyu_store::packages::Found;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

/// 运行日志的目标（`config.md`「出错」）。
const TARGET: &str = "miyu::config";

/// 登记的模块，照这个先后，一个模块里照声明的先后。加一个模块只加一行。设置页的页照第一次出现的先后排：通用、界面、
/// 权限、模型、高级（施工 8-2、8-3、8-6）；模型那一页先「用途」、再「供应商」、再「目录」（施工 8-7），「池」排在「用途」
/// 后面（施工 8-8；「挡位」8-8 补去掉了），「冷却」排在「目录」后面（施工 8-9）；高级那一页先「压缩」再「运行日志」（施工
/// 6-11 上）。通讯平台的桥的几项排在最后，高级页里「运行日志」后面（施工 O-8；O-16 加端口，O-17 加自己人）。
const MODULES: [&[Item]; 18] = [
    UiSettings::ITEMS,
    PersonaSettings::ITEMS,
    PresetSettings::ITEMS,
    UsageSettings::ITEMS,
    PermissionSettings::ITEMS,
    EXTERNAL_BINDINGS,
    UseSettings::ITEMS,
    PoolSettings::ITEMS,
    ProviderSettings::ITEMS,
    ModelSettings::ITEMS,
    PriceSettings::ITEMS,
    CatalogSettings::ITEMS,
    RateLimitedCooldown::ITEMS,
    RetryableCooldown::ITEMS,
    AuthCooldown::ITEMS,
    CompactionSettings::ITEMS,
    LogSettings::ITEMS,
    OnebotSettings::ITEMS,
];

miyu_config::settings! {
    /// 通讯平台的桥 `miyu-onebot` 的配置（施工 O-8，`docs/blueprint/onebot.md` 第一条）。权宜：照 `tui.startup`（施工 8-3）的
    /// 先例，软件包的清单还不能声明配置项，先由核心替桥声明、登记；桥自己读系统配置，核心不管它：起来时读一次，之后在 NapCat
    /// 的令牌对不上时、桥的 WebUI 问的时候重读（施工 O-16 补二）。施工 9-1 有了软件包的清单以后挪进桥自己的清单。
    pub struct OnebotSettings in "onebot" {
        /// NapCat 反连进来的端口，只听本机 `127.0.0.1`。8301 挨着网页软件的 8300（`onebot.md`「施工时定的」第 2 条）。桥起来时
        /// 读；在桥的 WebUI 上改、保存的当场换（`/apply`，O-16 补二），命令行改的桥下次起来时生效。
        listen: Option<i64> = 8301 {
            kind: int [1024, 65535],
            layers: [System],
            applies: head_start,
            ui: { page: "advanced", group: "onebot", control: number },
        },
        /// 桥自己的 WebUI 的端口，只听本机 `127.0.0.1`。8302 挨着网页软件的 8300、NapCat 的 8301（施工 O-16，`onebot.md`
        /// 第二条「对外的样子」）。生效同 `listen`。
        web: Option<i64> = 8302 {
            kind: int [1024, 65535],
            layers: [System],
            applies: head_start,
            ui: { page: "advanced", group: "onebot", control: number },
        },
        /// NapCat 连进来时出示的访问令牌：`{ secret = … }` 或 `{ env = … }`。没设、取不到的，桥照样起来，NapCat 连进来一律
        /// 401（「施工时定的」第 3 条）。改了当场生效：NapCat 下一次连进来，桥对不上就重读，照新的比（O-16 补二，`onebot.md`
        /// 第一条「怎么走」第 2 条）。
        token: Option<miyu_config::secret::Reference> = none {
            kind: secret,
            layers: [System],
            applies: now,
            ui: { page: "advanced", group: "onebot", control: text },
        },
        /// 自己人：平台身份的列表（`qq:20017`），私聊里能叫她、不限流、睡着时私聊也放行（施工 O-17，`chat.md`「发的人是谁」，
        /// `onebot.md` 第二条「怎么走」第 4 条）。在桥的 WebUI「主人与自己人」页上整张写回。元素最多 128 个字，和主人对应表的
        /// `<external>` 一样长；没有默认值，不写的就是没有自己人（「施工时定的」第 36 条）。现在桥还不读，桥接群、算「发的人
        /// 是谁」时读，到时照 O-16 的办法当场重读，所以生效时机写 `now`。以后桥的配置整体挪进软件包清单的 `[settings]` 时，
        /// 写成 `type = "list", element = "text", layers = ["system"]`，同一个提交删掉这一项。
        trusted: Option<Vec<String>> = none {
            kind: texts [128],
            layers: [System],
            applies: now,
            ui: { page: "advanced", group: "onebot", control: list },
        },
    }
}

/// 生成的三份放在状态区的这个目录里：`state/config/`。
const DIR: &str = "config";

/// 软件包的配置项和读成了的清单（施工 9-1 下，`packages.md`「配置项」）：核心起来时照清单拼好一次，读配置、生成 Schema
/// 和参考文件时并进去。
#[derive(Debug, Clone, Default)]
pub struct Packaged {
    /// 包的配置项，接在登记的后面。
    pub items: Vec<Item>,
    /// 读成了的清单：编号和样子，配置项的名字、说明照它。
    pub manifests: Vec<(String, Manifest)>,
}

impl Packaged {
    /// 照两层清单拼：编号撞了核心自己的模块的，那一份改报 `settings_taken`（[`miyu_endpoint::packages::settle`]）。
    pub fn of(found: &mut [Found]) -> Packaged {
        let items = miyu_endpoint::packages::settle(found, &items());
        let manifests = found
            .iter()
            .filter_map(|one| match &one.read {
                Ok(manifest) => Some((one.id.clone(), manifest.clone())),
                Err(_) => None,
            })
            .collect();
        Packaged { items, manifests }
    }

    /// 登记的全部配置项，接上包的。
    pub fn all(&self) -> Vec<Item> {
        items()
            .into_iter()
            .chain(self.items.iter().cloned())
            .collect()
    }
}

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
pub fn read(
    root: &DataRoot,
    admin: &AccountId,
    home: Option<&Path>,
    packaged: &Packaged,
) -> Config {
    Config::load(root, admin, home, packaged.all(), Environment::process())
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
pub fn generate(
    root: &DataRoot,
    resources: &ResourceRoot,
    locale: Option<&str>,
    values: &Values,
    packaged: &Packaged,
) {
    let items = packaged.all();
    let ui = UiSettings::from(values);
    let language = ui.language_for(locale);
    let manifests = packaged
        .manifests
        .iter()
        .map(|(id, manifest)| (id.as_str(), manifest));
    let texts = match Human::load(resources, language) {
        Ok(words) => render(&items, &words.with_packages(manifests, language))
            .map(|text| text.map_err(|error| error.to_string())),
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
