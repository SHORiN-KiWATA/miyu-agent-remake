//! 一家供应商照这一轮的配置和档案合出来的样子（`docs/blueprint/models.md`「怎么走」第一条，施工 8-6），和一个会话这一次
//! 请求发给谁（第四条 8-6 那一半）：
//!
//! 1. 驱动、地址：先看手写的，没写的看档案（`[providers.<目录里的编号>]`：写了 `catalog` 的是它，没写的是这一家自己的
//!    编号；8-6 还没有目录，只认编号一样的）。都没有的，这一家用不了，别的照常。
//! 2. 开关：档案的，没有的用驱动的默认（手写的 `compat` 随用到它的那一步）。
//! 3. key：照写的先后。取不到值的不当候选（由执行器取，这里只排先后，[`crate::keys`]）。
//! 4. 窗口：手写的，没有的照模型资料（[`crate::ModelTable`]，照对应目录里的那一家查）。
//! 5. 没有模型：`models.chat` 没配、引用解析不出，交 [`NoModel`]，原话照「出错」那张表。

use miyu_config::Values;
use miyu_config::secret::Reference as KeyRef;
use miyu_drivers::Inputs;
use miyu_drivers::openai_chat::Compat;

use crate::profile::{ImageTokens, Profiles};
use crate::reference::{Place, Reference};
use crate::settings::{ModelSettings, ProviderSettings, UseSettings};
use crate::table::{ModelFacts, ModelTable};

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
    /// 能收哪些输入。
    pub inputs: Inputs,
    /// 一张图怎么算。
    pub images: Option<ImageTokens>,
    /// 查模型资料、档案时照哪一家：写了 `catalog` 的是它，没写的是编号。
    pub catalog: String,
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

/// 编号 `id` 这一家这一轮的样子。
///
/// # Errors
///
/// 配置里没有这一家；推不出驱动、地址；驱动还没有。
pub fn provider(values: &Values, profiles: &Profiles, id: &str) -> Result<Provider, NoModel> {
    let configured = miyu_config::key::names(values.keys(), "providers.<id>", &[]);
    if !configured.iter().any(|name| name == id) {
        return Err(NoModel(format!("no provider {id:?}")));
    }
    let settings = ProviderSettings::at(values, &[id]);
    let catalog = settings.catalog.clone().unwrap_or_else(|| id.to_string());
    let profile = profiles
        .providers
        .get(&catalog)
        .cloned()
        .unwrap_or_default();
    let (Some(driver), Some(base_url)) = (
        settings.driver.or(profile.driver.clone()),
        settings.base_url.or(profile.base_url.clone()),
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
        inputs: profile.inputs(),
        images: profile.image_tokens,
        catalog,
    })
}

/// 引用 `text` 这一轮发给谁。挡位 8-6 都没配，退回 `models.chat`（再退一次还是挡位的，算解析不出）。
///
/// # Errors
///
/// 读不成；引用的池（随 8-8）、供应商没有；那一家用不了。
pub fn target(values: &Values, profiles: &Profiles, text: &str) -> Result<Target, NoModel> {
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
            provider: provider(values, profiles, &id)?,
            model,
        }),
        Reference::Pool(pool) => Err(NoModel(format!("no pool {pool:?}"))),
        Reference::Tier(tier) => Err(NoModel(format!("a tier cannot be used here: {tier:?}"))),
    }
}

/// 这个模型的窗口、最大输出：手写的窗口压过模型资料的；模型资料照对应目录里的那一家查。
pub fn facts(values: &Values, table: &ModelTable, target: &Target) -> ModelFacts {
    let found = table.find(&target.provider.catalog, &target.model);
    let written = ModelSettings::at(values, &[&target.provider.id, &target.model]).window;
    ModelFacts {
        window: written
            .and_then(|window| u64::try_from(window).ok())
            .or(found.window),
        max_output: found.max_output,
    }
}

#[cfg(test)]
mod tests;
