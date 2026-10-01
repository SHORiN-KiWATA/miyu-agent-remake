//! 一家供应商照这一轮的配置和档案合出来的样子（`docs/blueprint/models.md`「怎么走」第一条，施工 8-6、8-7），和一个会话这一次
//! 请求发给谁（第四条 8-6 那一半）：
//!
//! 1. 驱动、地址：先看手写的，没写的看档案（`[providers.<目录里的编号>]`：写了 `catalog` 的是它，没写的是这一家自己的
//!    编号）。档案也没有的，看它对上的目录里那一家（[`crate::matching::recognize`]）的 `api` 和 `npm`，`npm` 照档案的
//!    `[npm]` 表换成驱动（8-7）。都没有的，这一家用不了，别的照常。
//! 2. 开关：档案的，没有的用驱动的默认（手写的 `compat` 随用到它的那一步）。
//! 3. key：照写的先后。取不到值的不当候选（由执行器取，这里只排先后，[`crate::keys`]）。
//! 4. 本机的服务：手写的 `local`，没写的照地址在不在本机（第二条第 12 条，8-7）。手写的地址是环境变量的引用时查不出来，
//!    照不在本机算，想算本机的自己写 `local = true`（施工 8-6b）。
//! 5. 没有模型：`models.chat` 没配、引用解析不出，交 [`NoModel`]，原话照「出错」那张表。引用指到一个模型还是一个池、
//!    挡位换成什么，在 [`crate::reference::resolve`]（施工 8-8）。
//!
//! 地址可能是写死的，也可能是一个环境变量的引用（施工 8-6b，[`miyu_config::Address`]）：这里只带着引用走，不解出地址
//! 本身——对目录、本机的服务这两处用得到字面地址的，查不到的就当没有；真要连供应商的那一刻才经 [`resolve_base_url`]
//! 解出来（`route.rs`、`route/lists.rs`），地址因此不会被这一层的任何输出（`model.list`、`config.get`）带出去。
//!
//! 模型的资料照 [`crate::facts`]。

use miyu_config::secret::{Reference as KeyRef, Secret};
use miyu_config::{Address, Values};
use miyu_drivers::openai_chat::Compat;

use crate::knowledge::Knowledge;
use crate::matching::{Recognized, recognize};
use crate::profile::ImageTokens;
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
    /// 地址：写死的，或者一个环境变量的引用（施工 8-6b）。真要连供应商时经 [`resolve_base_url`] 解出来。
    pub base_url: Address,
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

/// 配置里有哪几家供应商：照编号排。引用的模型 `p/m`、池的成员照它认 `p` 在不在（施工 8-8）。
pub fn configured(values: &Values) -> Vec<String> {
    miyu_config::key::names(values.keys(), "providers.<id>", &[])
}

/// 编号 `id` 这一家这一轮的样子，照手头的资料 `knowledge`（档案、目录）推。
///
/// # Errors
///
/// 配置里没有这一家；推不出驱动、地址；驱动还没有。
pub fn provider(values: &Values, knowledge: &Knowledge<'_>, id: &str) -> Result<Provider, NoModel> {
    if !configured(values).iter().any(|name| name == id) {
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
    let written_url = settings
        .base_url
        .clone()
        .or_else(|| profile.base_url.clone().map(Address::Literal));
    // 对目录只认得出字面地址：是环境变量的引用时查不出来，当没有这一格（第四条第 2 条第 2 层）。
    let written_text = written_url.as_ref().and_then(literal);
    let recognized = knowledge.catalog.and_then(|loaded| {
        recognize(
            &loaded.catalog,
            id,
            written_text,
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
        written_url.or_else(|| {
            listed
                .and_then(|listed| listed.api.clone())
                .map(Address::Literal)
        }),
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
    // 本机的服务：是环境变量的引用时查不出来，照不在本机算（第四条，施工 8-6b）。
    let local = settings
        .local
        .unwrap_or_else(|| literal(&base_url).is_some_and(on_this_machine));
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

/// 字面地址：写死的就是它，环境变量的引用查不出来（施工 8-6b）。
fn literal(address: &Address) -> Option<&str> {
    match address {
        Address::Literal(text) => Some(text.as_str()),
        Address::Env(_) => None,
    }
}

/// 照 `secret` 取这一家的地址（施工 8-6b）：写死的直接用；是环境变量的引用的照取，`secret` 和取 key 的办法一样（`Reference`
/// 不分密钥、网址）。没设、设成空的：这一家没有地址，`NoModel`，原话照「有 key 取不到」的样子（`route.rs`、
/// `route/lists.rs` 真要连供应商时调）。地址不会经这个函数之外的任何路径流出去。
///
/// # Errors
///
/// 环境变量没设、设成空的（[`NoModel`]）。
pub fn resolve_base_url(
    provider: &Provider,
    secret: &dyn Fn(&KeyRef) -> Option<Secret>,
) -> Result<String, NoModel> {
    match &provider.base_url {
        Address::Literal(text) => Ok(text.clone()),
        Address::Env(name) => secret(&KeyRef::Env(name.clone()))
            .map(|secret| secret.expose().to_string())
            .ok_or_else(|| NoModel(format!("provider {:?} has no usable base_url", provider.id))),
    }
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

#[cfg(test)]
mod tests;
