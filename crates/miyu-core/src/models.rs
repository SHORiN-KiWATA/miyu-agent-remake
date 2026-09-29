//! 模型（`docs/designs/15-模型与供应商.md` 第七节，施工 3-9 上）：配置系统做出来之前，只认环境变量
//! `DEEPSEEK_API_KEY`（2026-09-27 项目主人定），设了就接 DeepSeek 官方的 `deepseek-flash`；没设的，每次
//! 请求都当场回「没有可用的模型」。key 只在内存里，不落盘、不写配置、不进日志。
//!
//! 开发用的 `MIYU_DEV_BASE_URL`、`MIYU_DEV_MODEL` 替换地址和模型（施工 3-9 再补，2026-09-29 项目主人定），
//! `MIYU_DEV_WINDOW` 给窗口（施工 6-3 上，同一天定），配置系统做好以后删掉。
//!
//! 模型的窗口、最大输出照资源目录里的模型资料查（[`ModelTable`]）；一张图怎么算跟着驱动的写法走，DeepSeek 的写法交官方
//! 计算器的算法。

mod table;

pub use table::{ModelFacts, ModelTable};

use std::sync::Arc;

use miyu_drivers::openai_chat::Compat;
use miyu_drivers::{Call, DeepSeekImages, Inputs};
use miyu_http::{Endpoint, Proxy, client};
use miyu_kernel::event::{CallError, ErrorClass};
use miyu_kernel::id::{ModelName, ProviderId, Seq};
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_session::{Cancel, ForSession, HttpModels, ModelPort, Models, Reports};

use crate::TARGET;

/// DeepSeek 官方的地址。
const BASE_URL: &str = "https://api.deepseek.com";

/// 端点的编号：记进 `model.called`。
const PROVIDER: &str = "deepseek";

/// 换了地址的端点编号：看得出这一次没走 DeepSeek 官方。
const DEV_PROVIDER: &str = "dev";

/// 起来时从环境变量读到的：key，和开发用的地址、模型。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelEnv {
    /// `DEEPSEEK_API_KEY`。
    pub key: Option<String>,
    /// `MIYU_DEV_BASE_URL`。
    pub base_url: Option<String>,
    /// `MIYU_DEV_MODEL`。
    pub model: Option<String>,
    /// `MIYU_DEV_WINDOW`。
    pub window: Option<String>,
}

/// 挑定的端点：地址、编号、模型、key。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Chosen {
    base_url: String,
    provider: &'static str,
    model: ModelName,
    key: String,
    /// `MIYU_DEV_WINDOW` 给的窗口。
    window: Option<u64>,
}

/// 照环境变量挑端点：没有 key 的是 `None`；开发用的两个，去掉前后空白不是空的才算设了。
fn choose(env: &ModelEnv) -> Result<Option<Chosen>, String> {
    let set = |value: &Option<String>| {
        value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    let Some(key) = set(&env.key) else {
        return Ok(None);
    };
    let model = match set(&env.model) {
        Some(name) => ModelName::parse(&name)
            .map_err(|error| format!("MIYU_DEV_MODEL {name:?} is not a model name: {error}"))?,
        None => model(MODEL),
    };
    let (base_url, provider) = match set(&env.base_url) {
        Some(url) => (url, DEV_PROVIDER),
        None => (BASE_URL.to_string(), PROVIDER),
    };
    let window = match set(&env.window) {
        Some(text) => Some(
            text.parse::<u64>()
                .ok()
                .filter(|window| *window > 0)
                .ok_or_else(|| {
                    format!("MIYU_DEV_WINDOW {text:?} is not a positive whole number")
                })?,
        ),
        None => None,
    };
    Ok(Some(Chosen {
        base_url,
        provider,
        model,
        key,
        window,
    }))
}

/// 这个模型的资料：驱动用的是 DeepSeek 的写法，开发端点也照 `deepseek` 那一家查；`MIYU_DEV_WINDOW` 压过查到的窗口。
fn facts(chosen: &Chosen, table: &ModelTable) -> ModelFacts {
    let found = table.find(PROVIDER, chosen.model.as_str());
    ModelFacts {
        window: chosen.window.or(found.window),
        max_output: found.max_output,
    }
}

/// 模型名，照 DeepSeek 那边的叫法。
const MODEL: &str = "deepseek-flash";

/// 照环境变量造给会话请求模型的端口：有 key 的接 DeepSeek 官方，设了开发用的地址、模型的换成它们；没有 key 的回「没有
/// 可用的模型」。窗口、最大输出照 `table` 查。
///
/// # Errors
///
/// `MIYU_DEV_MODEL` 不合模型名的写法、`MIYU_DEV_WINDOW` 不是正整数；HTTP 客户端造不出来（系统的证书读不了之类）。
/// 交回原因。
pub fn from_env(env: &ModelEnv, table: &ModelTable) -> Result<Arc<dyn Models>, String> {
    let Some(chosen) = choose(env)? else {
        tracing::warn!(target: TARGET, "DEEPSEEK_API_KEY not set, no model");
        return Ok(Arc::new(Unavailable));
    };
    if chosen.base_url != BASE_URL || chosen.model.as_str() != MODEL {
        tracing::info!(
            target: TARGET,
            "dev endpoint base_url={} model={}",
            chosen.base_url,
            chosen.model
        );
    }
    let facts = facts(&chosen, table);
    tracing::info!(
        target: TARGET,
        "model limits model={} window={} max_output={}",
        chosen.model,
        or_none(facts.window),
        or_none(facts.max_output)
    );
    let client = client(Proxy::FromEnvironment).map_err(|error| error.to_string())?;
    Ok(Arc::new(HttpModels {
        client,
        provider: provider(chosen.provider),
        endpoint: Endpoint::new(&chosen.base_url, &chosen.key),
        compat: Compat::deepseek(),
        call: call(chosen.model),
        idle: miyu_session::IDLE,
        window: facts.window,
        max_output: facts.max_output,
        images: Some(Arc::new(DeepSeekImages)),
    }))
}

/// 日志里的数：没有的写 `none`。
fn or_none(value: Option<u64>) -> String {
    value.map_or_else(|| "none".to_string(), |value| value.to_string())
}

/// 没有可用的模型。
struct Unavailable;

impl Models for Unavailable {
    fn port(&self, _: ForSession) -> Arc<dyn ModelPort> {
        Arc::new(NoModel(Model {
            endpoint: provider("none"),
            model: model("none"),
        }))
    }
}

/// 每次请求都当场说完，没发出去；分类是认证失败（没有 key），内核不重试。
struct NoModel(Model);

impl ModelPort for NoModel {
    fn model(&self) -> &Model {
        &self.0
    }

    fn call(&self, _: Seq, _: Request, reports: Reports, _: Cancel) {
        reports.ended(
            None,
            Some(CallError {
                class: ErrorClass::Auth,
                message: "no model: set DEEPSEEK_API_KEY".to_string(),
                status: None,
            }),
            None,
            None,
        );
    }
}

/// 每次请求 DeepSeek 定的：模型名，不写输出上限；收图，不收 PDF。DeepSeek 从 2026-08-21 起收图，只收 user 消息里的，
/// 工具结果里的图由驱动挪过去（施工 4-13）。
fn call(model: ModelName) -> Call {
    Call {
        model,
        max_output: None,
        inputs: Inputs {
            images: true,
            pdf: false,
        },
    }
}

fn provider(name: &str) -> ProviderId {
    ProviderId::parse(name).unwrap_or_else(|e| unreachable!("「{name}」合端点编号的写法：{e}"))
}

fn model(name: &str) -> ModelName {
    ModelName::parse(name).unwrap_or_else(|e| unreachable!("「{name}」合模型名的写法：{e}"))
}

#[cfg(test)]
mod tests;
