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
//! 5. 没有模型：`models.chat` 没配、引用解析不出，交 [`NoModel`]，原话照「出错」那张表。引用指到一个模型还是一个池，在
//!    [`crate::reference::resolve`]（施工 8-8）。
//!
//! 地址可能是写死的，也可能是一个环境变量的引用（施工 8-6b，[`miyu_config::Address`]）：这里只带着引用走，不解出地址
//! 本身——对目录、本机的服务这两处用得到字面地址的，查不到的就当没有；真要连供应商的那一刻才经 [`resolve_base_url`]
//! 解出来（`route.rs`、`route/lists.rs`），地址因此不会被这一层的任何输出（`model.list`、`config.get`）带出去。
//!
//! 模型的资料照 [`crate::facts`]。
//!
//! 一家里的模型可以各走各的驱动（施工 8-14，`drivers/openai-chat.md`「接 opencode Zen」）：真发的那个模型照
//! [`Provider::for_model`] 换成它的样子——驱动照「手写的供应商 `driver` > 第 1、2 层对上的模型的 `npm` > 档案 > 目录里那一家的
//! `npm`」，`openai-chat` 的思考回传照目录的 `interleaved`（档案写了 `reasoning` 的照档案）。另配的头照档案，值是模板
//! （[`crate::headers`]）。

use miyu_config::secret::{Reference as KeyRef, Secret};
use miyu_config::{Address, Values};
use std::collections::BTreeMap;

use miyu_drivers::openai_chat::{Compat, ReasoningField, ReasoningReplay};
use miyu_drivers::{Anthropic, DriverTexts, OpenAiChat, OpenAiResponses};

use crate::facts::Wire;
use crate::headers;
use crate::knowledge::Knowledge;
use crate::matching::{Recognized, recognize};
use crate::profile::ImageTokens;
use crate::settings::{ProviderSettings, UseSettings};

/// 认得的驱动。8-6 有 OpenAI 兼容的对话接口，8-12 加 Anthropic 的消息接口，8-13 加 OpenAI 的 Responses 接口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    /// `openai-chat`。
    OpenAiChat,
    /// `anthropic`（施工 8-12）。
    Anthropic,
    /// `openai-responses`（施工 8-13）。
    OpenAiResponses,
}

impl Driver {
    /// 配置、档案里驱动的写法认成现在有的哪一种（施工 8-11 从 [`provider`] 里拿出来，`provider.catalog` 的 `supported` 也照
    /// 它）；不认识的（档案里写了别的、目录的包名换出来的还没有的）是空的。
    pub fn parse(name: &str) -> Option<Driver> {
        match name {
            "openai-chat" => Some(Driver::OpenAiChat),
            "anthropic" => Some(Driver::Anthropic),
            "openai-responses" => Some(Driver::OpenAiResponses),
            _ => None,
        }
    }

    /// 配置、档案里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Driver::OpenAiChat => "openai-chat",
            Driver::Anthropic => "anthropic",
            Driver::OpenAiResponses => "openai-responses",
        }
    }

    /// 造这种驱动（施工 8-12）：`openai-chat` 照开关 `compat`，`anthropic`、`openai-responses` 没有开关；占位是 `texts`。
    pub fn build(self, compat: Compat, texts: DriverTexts) -> Box<dyn miyu_drivers::Driver> {
        match self {
            Driver::OpenAiChat => Box::new(OpenAiChat::new(compat, texts)),
            Driver::Anthropic => Box::new(Anthropic::new(texts)),
            Driver::OpenAiResponses => Box::new(OpenAiResponses::new(texts)),
        }
    }

    /// 一定要写输出上限（`models.md`「驱动要守的约定」第 11 条，施工 8-12）：`anthropic` 是，路由替它照模型资料填。
    pub fn needs_max_output(self) -> bool {
        self == Driver::Anthropic
    }
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
    /// 驱动是配置里手写的（施工 8-14）：压过目录里模型的 `npm`。
    pub driver_written: bool,
    /// 档案写了思考怎么回传（施工 8-14）：压过目录的 `interleaved`。
    pub reasoning_written: bool,
    /// 档案另配的头：名字 → 模板（施工 8-14，[`crate::headers`]）。
    pub headers: BTreeMap<String, String>,
    /// 档案点名的占位工具（施工 8-14 补）：工具面里缺这几件时补同名的占位声明（`models.md`「八、opencode Zen」第 2 条）。
    pub placeholders: Vec<String>,
}

impl Provider {
    /// 能不能照开关关思考（「怎么走」第十一条第 1 条，施工 8-12）：`openai-chat` 照档案写没写开关（`compat.toggle`），
    /// `anthropic` 的开关是接口自带的（`thinking` 写 `disabled`）；`openai-responses` 没有开关，`off` 只从目录的 `none` 来（8-13）。
    pub fn switchable(&self) -> bool {
        match self.driver {
            Driver::OpenAiChat => self.compat.toggle.is_some(),
            Driver::Anthropic => true,
            Driver::OpenAiResponses => false,
        }
    }

    /// 照这一家的驱动和开关造一个驱动，占位是 `texts`（施工 8-12）。
    pub fn build(&self, texts: DriverTexts) -> Box<dyn miyu_drivers::Driver> {
        self.driver.build(self.compat.clone(), texts)
    }

    /// 发给模型 `model` 时这一家的样子（施工 8-14）：`wire` 是这个模型照目录怎么说话（只取第 1、2 层对上的，
    /// [`crate::facts::Facts::wire`]），`npm` 是档案的 `[npm]` 表。
    ///
    /// - 驱动：手写的照手写的；不然模型写了自己的包名的照它换，换不出现在有的驱动的，这个模型用不了；都没有的照这一家的。
    /// - 开关：走 `openai-chat`、档案没写 `reasoning` 的，目录写了交错思考的照它回传（`always` 是真的），认不出的字段照旧。
    ///
    /// # Errors
    ///
    /// 模型的包名换不出现在有的驱动：`model "<供应商>/<模型>" needs driver "<它>", which is not available yet`（「它」是 `[npm]`
    /// 换出来的名字，表里没有的是包名本身）。
    pub fn for_model(
        &self,
        model: &str,
        wire: &Wire,
        npm: &BTreeMap<String, String>,
    ) -> Result<Provider, NoModel> {
        let mut speaking = self.clone();
        if let Some(package) = wire.npm.as_deref().filter(|_| !self.driver_written) {
            let name = npm.get(package).map_or(package, String::as_str);
            speaking.driver = Driver::parse(name).ok_or_else(|| {
                NoModel(format!(
                    "model \"{}/{model}\" needs driver {name:?}, which is not available yet",
                    self.id
                ))
            })?;
        }
        let field = match wire.interleaved.as_deref() {
            Some("reasoning_content") => Some(ReasoningField::ReasoningContent),
            Some("reasoning") => Some(ReasoningField::Reasoning),
            _ => None,
        };
        if let Some(field) =
            field.filter(|_| speaking.driver == Driver::OpenAiChat && !self.reasoning_written)
        {
            speaking.compat.reasoning = ReasoningReplay::Replay {
                field,
                always: true,
            };
        }
        Ok(speaking)
    }

    /// 发给模型 `model` 时真走的驱动（施工 8-22，温度的上限照它）：照 [`Provider::for_model`]，这个模型用不了的照这一家的
    /// （发的时候当场 `no_model`）。
    pub fn driver_for(&self, model: &str, wire: &Wire, npm: &BTreeMap<String, String>) -> Driver {
        self.for_model(model, wire, npm)
            .map_or(self.driver, |speaking| speaking.driver)
    }

    /// 另配的头，照种子 `seed` 换好模板（施工 8-14，[`crate::headers`]）：会话的是会话编号，一次性的是用途，`provider.test`
    /// 是 [`headers::PROBE_SEED`]。照名字排。
    pub fn headers(&self, seed: &str) -> Vec<(String, String)> {
        self.headers
            .iter()
            .map(|(name, template)| (name.clone(), headers::fill(template, seed)))
            .collect()
    }

    /// 占位工具（施工 8-14 补）：档案点名的名字，说明 `text` 是资源目录里那一句（`resources/core/drivers/placeholder-tool.txt`）。
    /// 说明是空的（测试、老数据根）不补；造请求的一方（`route/placeholder.rs`）照它填。
    pub fn placeholder_specs(&self, text: &str) -> Vec<(String, String)> {
        match text.is_empty() {
            true => Vec::new(),
            false => self
                .placeholders
                .iter()
                .map(|name| (name.clone(), text.to_string()))
                .collect(),
        }
    }
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
    let driver_written = settings.driver.is_some();
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
    let Some(driver_kind) = Driver::parse(&driver) else {
        return Err(NoModel(format!(
            "driver {driver:?} of provider {id:?} is not available yet"
        )));
    };
    // 本机的服务：是环境变量的引用时查不出来，照不在本机算（第四条，施工 8-6b）。
    let local = settings
        .local
        .unwrap_or_else(|| literal(&base_url).is_some_and(on_this_machine));
    Ok(Provider {
        id: id.to_string(),
        driver: driver_kind,
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
        driver_written,
        reasoning_written: profile
            .compat
            .as_ref()
            .is_some_and(|compat| compat.reasoning.is_some()),
        headers: profile.headers,
        placeholders: profile.placeholder_tools,
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

/// 地址在本机：主机名是 `127.0.0.1`、`localhost`、`::1`（写成 `[::1]`），不分大小写。第一次接入探哪几家也照它（施工 8-11）。
pub(crate) fn on_this_machine(base_url: &str) -> bool {
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
