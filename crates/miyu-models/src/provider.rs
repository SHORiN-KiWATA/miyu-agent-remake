//! 一家供应商照这一轮的配置和档案合出来的样子（`docs/blueprint/models.md`「怎么走」第一条，施工 8-6、8-7），和一个会话这一次
//! 请求发给谁（第四条 8-6 那一半）：
//!
//! 1. 驱动、地址：先看手写的，没写的看档案（`[providers.<目录里的编号>]`：写了 `catalog` 的是它，没写的是这一家自己的
//!    编号）。档案也没有的，看它对上的目录里那一家（[`crate::matching::recognize`]）的 `api` 和 `npm`，`npm` 照档案的
//!    `[npm]` 表换成驱动（8-7）。都没有的，这一家用不了，别的照常。
//! 2. 开关：档案的，没有的用驱动的默认（手写的 `compat` 随用到它的那一步）。
//! 3. key：照写的先后。取不到值的不当候选（由执行器取，这里只排先后，[`crate::keys`]）。
//! 4. 本机的服务：手写的 `local`，没写的照地址在不在本机（第二条第 12 条，8-7）。
//! 5. 没有模型：`models.chat` 没配、引用解析不出，交 [`NoModel`]，原话照「出错」那张表。
//!
//! 模型的资料照 [`crate::facts`]。

use miyu_config::Values;
use miyu_config::secret::Reference as KeyRef;
use miyu_drivers::openai_chat::Compat;

use crate::knowledge::Knowledge;
use crate::matching::{Recognized, recognize};
use crate::profile::ImageTokens;
use crate::reference::{Place, Reference};
use crate::settings::{ProviderSettings, UseSettings};

/// 认得的驱动。8-6 只有 OpenAI 兼容的对话接口；另两种随 8-12、8-13。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    /// `openai-chat`。
    OpenAiChat,
}

/// 一家供应商这一轮的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    /// 编号：记进 `model.called` 的 `endpoint`。
    pub id: String,
    /// 驱动。
    pub driver: Driver,
    /// 地址。
    pub base_url: String,
    /// `openai-chat` 的开关。
    pub compat: Compat,
    /// 几个 key，照写的先后。空的不带认证头。
    pub keys: Vec<KeyRef>,
    /// 一张图怎么算。
    pub images: Option<ImageTokens>,
    /// 查档案时照哪一家：写了 `catalog` 的是它，没写的是编号。
    pub catalog: String,
    /// 它在目录里是哪一家（第二条第 4 条第 2 层，8-7）：没认出来、还没有目录的没有。
    pub recognized: Option<Recognized>,
    /// 本机的模型服务：价格当 0（8-7）。
    pub local: bool,
}

/// 一次请求发给谁：哪一家、哪个模型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// 那一家。
    pub provider: Provider,
    /// 模型名，照供应商那边的叫法。
    pub model: String,
}

/// 没有能用的模型：原话（英文，进 `model.called`、运行日志）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoModel(pub String);

/// `models.chat` 没配时的原话（`models.md`「怎么走」第一条第 7 条）。
pub const NOT_CONFIGURED: &str = "no model configured: set models.chat";

/// 这一轮的 `models.chat`：没配的是空的。
pub fn chat(values: &Values) -> Option<String> {
    UseSettings::from(values).chat
}

/// 编号 `id` 这一家这一轮的样子，照手头的资料 `knowledge`（档案、目录）推。
///
/// # Errors
///
/// 配置里没有这一家；推不出驱动、地址；驱动还没有。
pub fn provider(values: &Values, knowledge: &Knowledge<'_>, id: &str) -> Result<Provider, NoModel> {
    let configured = miyu_config::key::names(values.keys(), "providers.<id>", &[]);
    if !configured.iter().any(|name| name == id) {
        return Err(NoModel(format!("no provider {id:?}")));
    }
    let settings = ProviderSettings::at(values, &[id]);
    let catalog = settings.catalog.clone().unwrap_or_else(|| id.to_string());
    let profile = knowledge
        .profiles
        .providers
        .get(&catalog)
        .cloned()
        .unwrap_or_default();
    let written_url = settings.base_url.clone().or(profile.base_url.clone());
    let recognized = knowledge.catalog.and_then(|loaded| {
        recognize(
            &loaded.catalog,
            id,
            written_url.as_deref(),
            settings.catalog.as_deref(),
        )
    });
    let listed = recognized.as_ref().and_then(|recognized| {
        knowledge
            .catalog
            .and_then(|loaded| loaded.catalog.provider(&recognized.provider))
    });
    let from_npm = listed
        .and_then(|listed| listed.npm.as_ref())
        .and_then(|npm| knowledge.profiles.npm.get(npm).cloned());
    let (Some(driver), Some(base_url)) = (
        settings.driver.or(profile.driver.clone()).or(from_npm),
        written_url.or_else(|| listed.and_then(|listed| listed.api.clone())),
    ) else {
        return Err(NoModel(format!(
            "provider {id:?} needs driver and base_url: it matches nothing in the catalog"
        )));
    };
    let driver = match driver.as_str() {
        "openai-chat" => Driver::OpenAiChat,
        other => {
            return Err(NoModel(format!(
                "driver {other:?} of provider {id:?} is not available yet"
            )));
        }
    };
    let local = settings.local.unwrap_or_else(|| on_this_machine(&base_url));
    Ok(Provider {
        id: id.to_string(),
        driver,
        base_url,
        compat: profile
            .compat
            .as_ref()
            .map(|compat| compat.compat())
            .unwrap_or_default(),
        keys: settings.keys,
        images: profile.image_tokens,
        catalog,
        recognized,
        local,
    })
}

/// 地址在本机：主机名是 `127.0.0.1`、`localhost`、`::1`（写成 `[::1]`），不分大小写。
fn on_this_machine(base_url: &str) -> bool {
    let rest = base_url
        .split_once("://")
        .map_or(base_url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = match authority.strip_prefix('[') {
        Some(inside) => inside.split(']').next().unwrap_or_default(),
        None => authority.split(':').next().unwrap_or_default(),
    };
    ["127.0.0.1", "localhost", "::1"]
        .iter()
        .any(|local| host.eq_ignore_ascii_case(local))
}

/// 引用 `text` 这一轮发给谁。挡位 8-6 都没配，退回 `models.chat`（再退一次还是挡位的，算解析不出）。
///
/// # Errors
///
/// 读不成；引用的池（随 8-8）、供应商没有；那一家用不了。
pub fn target(values: &Values, knowledge: &Knowledge<'_>, text: &str) -> Result<Target, NoModel> {
    let reference =
        Reference::parse_at(text, Place::Session).map_err(|bad| NoModel(bad.to_string()))?;
    let reference = match reference {
        Reference::Tier(_) => {
            let chat = chat(values).ok_or_else(|| NoModel(NOT_CONFIGURED.to_string()))?;
            Reference::parse_at(&chat, Place::Use).map_err(|bad| NoModel(bad.to_string()))?
        }
        other => other,
    };
    match reference {
        Reference::Model {
            provider: id,
            model,
        } => Ok(Target {
            provider: provider(values, knowledge, &id)?,
            model,
        }),
        Reference::Pool(pool) => Err(NoModel(format!("no pool {pool:?}"))),
        Reference::Tier(tier) => Err(NoModel(format!("a tier cannot be used here: {tier:?}"))),
    }
}

#[cfg(test)]
mod tests;
