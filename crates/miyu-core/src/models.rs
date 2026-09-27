//! 模型（`docs/designs/15-模型与供应商.md` 第七节，施工 3-9 上）：配置系统做出来之前，只认环境变量
//! `DEEPSEEK_API_KEY`（2026-09-27 项目主人定），设了就接 DeepSeek 官方的 `deepseek-flash`；没设的，每次
//! 请求都当场回「没有可用的模型」。key 只在内存里，不落盘、不写配置、不进日志。

use std::sync::Arc;

use miyu_drivers::openai_chat::Compat;
use miyu_drivers::{Call, Inputs};
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

/// 模型名，照 DeepSeek 那边的叫法。
const MODEL: &str = "deepseek-flash";

/// 照 `key` 造给会话请求模型的端口：有 key 的接 DeepSeek 官方，没有的回「没有可用的模型」。
///
/// # Errors
///
/// HTTP 客户端造不出来（系统的证书读不了之类），交回原因。
pub fn from_env(key: Option<String>) -> Result<Arc<dyn Models>, String> {
    let Some(key) = key.filter(|key| !key.trim().is_empty()) else {
        tracing::warn!(target: TARGET, "DEEPSEEK_API_KEY not set, no model");
        return Ok(Arc::new(Unavailable));
    };
    let client = client(Proxy::FromEnvironment).map_err(|error| error.to_string())?;
    Ok(Arc::new(HttpModels {
        client,
        provider: provider(PROVIDER),
        endpoint: Endpoint::new(BASE_URL, key.trim()),
        compat: Compat::deepseek(),
        call: Call {
            model: model(MODEL),
            max_output: None,
            inputs: Inputs::default(),
        },
        idle: miyu_session::IDLE,
    }))
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
            }),
            None,
        );
    }
}

fn provider(name: &str) -> ProviderId {
    ProviderId::parse(name).unwrap_or_else(|e| unreachable!("「{name}」合端点编号的写法：{e}"))
}

fn model(name: &str) -> ModelName {
    ModelName::parse(name).unwrap_or_else(|e| unreachable!("「{name}」合模型名的写法：{e}"))
}
