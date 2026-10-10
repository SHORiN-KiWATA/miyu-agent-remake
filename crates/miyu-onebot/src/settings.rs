//! 桥用的配置（`onebot.md` 第一条「对外的样子」「怎么走」第 1、2 条，施工 O-20）：核心拉起桥时，握手的回应里交来这个包自己
//! 的配置（`config`：`onebot.` 开头的键，最终值，密钥是真值，没设、取不到的那一键不放），之后变了推 `extension.config`
//! （`{"keys": {键: 新值或 null}}`，只放变了的键；`extensions.md`「配置」）。桥不读系统配置、密钥文件，不依赖核心的 crate
//! （「施工时定的」第 6 条）。
//!
//! - NapCat 的端口 `onebot.listen`：0 到 65535 的整数照它（核心只收 1024 到 65535；0 是测试让系统挑）；没有的、`null` 的、
//!   不是的，照清单 `[settings]` 的默认值（[`Defaults`]，「施工时定的」第 38 条）。原来桥自己的网页的端口 `onebot.web` 随施工
//!   O-28 下去掉：交来了也不认（第 170 条）。
//! - 令牌 `onebot.token`：是字的照它，去掉前后空白（[`Secret::new`]）；没有的、`null`、空的、不是字的就是没有：桥照样起来，
//!   NapCat 连进来一律 401（第 1、2 条）。核心不交取不到的引用，桥分不出「没写引用」和「取不到」（「施工时定的」第 41 条）。
//! - 白名单成员 `onebot.whitelist`（施工 O-23；O-27 从 `onebot.trusted` 改名，旧键不认）：跟核心的那一头照 [`whitelist`] 读
//!   一份、推来的换（「群里怎么叫她」第 3 条），不进 [`Settings`]：NapCat 的监听、后台页的方法用不上它。别的键不认。
//!
//! 键是 `<包的编号>.<名字>`（`packages.md`「配置项」第 1 条）：名字在这里写一次，清单的默认值、握手和推送的键都照它。

use serde_json::{Map, Value};

use miyu_config::secret::Secret;
use miyu_store::packages::{Issue, Packages};
use miyu_store::resources::ResourceRoot;

use crate::PACKAGE;

/// NapCat 反连进来的端口：清单里的名字。
const LISTEN: &str = "listen";

/// NapCat 要出示的令牌：清单里的名字。
const TOKEN: &str = "token";

/// 白名单成员：清单里的名字（施工 O-23；O-27 从 `trusted` 改名，旧键不认）。
const WHITELIST: &str = "whitelist";

/// 桥用的两项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// NapCat 反连进来的端口，只听 `127.0.0.1`；`0` 是让系统挑一个（测试用）。
    pub port: u16,
    /// NapCat 连进来时要出示的访问令牌；没有的是空的。`Debug` 只印 `Secret(…)`。
    pub token: Option<Secret>,
}

/// 清单 `[settings]` 里 NapCat 端口的默认值：握手没交、推来 `null` 的照它（「施工时定的」第 38 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Defaults {
    /// `onebot.listen` 的默认值（出厂 8301）。
    pub listen: u16,
}

impl Defaults {
    /// 读资源目录 `resources` 里这个包的清单（出厂那一层的 [`PACKAGE`]，`miyu_store::packages`），取 NapCat 端口的默认值。
    ///
    /// # Errors
    ///
    /// 清单不在、读不成、写错了，端口没声明、没写默认值、默认值不是端口：原话里说是哪个文件。
    pub fn load(resources: &ResourceRoot) -> Result<Defaults, String> {
        let found = Packages::shipped(resources)
            .read()
            .into_iter()
            .find(|found| found.id == PACKAGE)
            .ok_or_else(|| {
                format!(
                    "no {PACKAGE} package manifest in {}",
                    resources.path().display()
                )
            })?;
        let bad = |why: String| format!("{} not readable: {why}", found.path.display());
        let manifest = match &found.read {
            Ok(manifest) => manifest,
            Err(Issue::Wrong(problem)) => return Err(bad(problem.message.clone())),
            Err(Issue::Unreadable(error)) => return Err(bad(error.to_string())),
        };
        let listen = manifest
            .settings
            .iter()
            .find(|setting| setting.name == LISTEN)
            .and_then(|setting| match setting.default {
                Some(miyu_config::Value::Int(port)) => u16::try_from(port).ok(),
                _ => None,
            })
            .ok_or_else(|| bad(format!("no port default for {PACKAGE}.{LISTEN}")))?;
        Ok(Defaults { listen })
    }
}

impl Settings {
    /// 握手的回应交来的 `config`（第 1 条）：没交的、读不出来的端口照 `defaults`，没交令牌的没有令牌。`config` 不是对象的
    /// （没带这一格）当什么都没交。
    pub fn handed(config: &Value, defaults: &Defaults) -> Settings {
        let mut settings = Settings {
            port: defaults.listen,
            token: None,
        };
        if let Some(config) = config.as_object() {
            settings.change(config, defaults);
        }
        settings
    }

    /// 推来的变化 `keys` 合进来（`extension.config` 的 `keys`）：带了的键照新值，`null` 的当没有（端口照 `defaults`，令牌
    /// 没了）；没带的不动；别的键不认。
    pub fn change(&mut self, keys: &Map<String, Value>, defaults: &Defaults) {
        for (key, value) in keys {
            let name = key
                .strip_prefix(PACKAGE)
                .and_then(|rest| rest.strip_prefix('.'));
            match name {
                Some(LISTEN) => self.port = port(value).unwrap_or(defaults.listen),
                Some(TOKEN) => {
                    self.token = value.as_str().and_then(|token| Secret::new(token).ok())
                }
                _ => {}
            }
        }
    }
}

/// 白名单成员那一键（`onebot.whitelist`）：握手交来的 `config`、推来的 `keys` 里照它取（施工 O-23）。
pub fn whitelist_key() -> String {
    format!("{PACKAGE}.{WHITELIST}")
}

/// 白名单成员的平台身份（`onebot.whitelist` 的值，施工 O-23，「群里怎么叫她」第 3 条）：字的列表照收；没有、`null`、不是列表的是
/// 空的（核心照清单查过类型，照说不会），列表里不是字的不要。
pub fn whitelist(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// 一个端口：0 到 65535 的整数；别的是空的。
fn port(value: &Value) -> Option<u16> {
    value.as_u64().and_then(|port| u16::try_from(port).ok())
}
