//! 每个会话的路由（`docs/blueprint/models.md`「怎么走」第一条、第四条 8-6 那一半，`session/actor.md` 第 8 条，施工 8-6）：
//! 请求模型的端口的真实现，取代原来照环境变量接一个端点的 `HttpModels`。
//!
//! - 造端口时（造会话、载入）照那一刻的配置记下这个会话用的引用：`models.chat`。限额（窗口、最大输出、一张图怎么算）照它
//!   定，交给内核（会变随 8-10）。
//! - 每一次请求照这一轮冻结的配置（[`crate::TurnConfig`]）重新解析：钉着的引用解析得出就用它；解析不出的退回这一轮的
//!   `models.chat`，退得回去的以后就钉在它上面；都不行的当场说完，分类 `no_model`，不发（第一条第 7 条）。
//! - key：这一家写了几个，会话钉在照会话编号算出的那一个上（`miyu_models::keys`），取不到值的跳过，照写的先后取下一个；
//!   一个都取不到的也是 `no_model`。出错换 key、换端点随 8-9。没写 key 的不带认证头（本机的服务）。
//! - 发：照驱动编码、经 HTTP 执行器发、流式读回来（[`send`]），和原来一样。

mod send;

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use miyu_drivers::DriverTexts;
use miyu_drivers::{Call, DeepSeekImages, OpenAiChat};
use miyu_http::{Client, Endpoint};
use miyu_kernel::estimate::ImagePrice;
use miyu_kernel::event::{CallError, ErrorClass};
use miyu_kernel::id::{ModelName, ProviderId, Seq, SessionId};
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_kernel::session::Limits;
use miyu_models::profile::{ImageTokens, Profiles};
use miyu_models::provider::{self, NOT_CONFIGURED, NoModel, Target};
use miyu_models::{ModelTable, keys};
use miyu_store::blob::Blobs;

use crate::TARGET;
use crate::config::TurnConfig;
use crate::port::{Cancel, ForSession, ModelPort, Models, Reports};

/// 空闲超时的初值：多久没收到新的字节就算断了（`05-内核接口.md` 第七节，以后按思考强度放大）。
pub const IDLE: Duration = Duration::from_secs(180);

/// 核心一份的：HTTP 客户端、档案、模型资料、空闲超时。给每个会话造一个路由。
#[derive(Clone)]
pub struct Routes {
    /// HTTP 客户端：一个核心一个，连接跨请求复用。
    pub client: Client,
    /// 认得出的供应商的档案（资源目录的 `models/profiles.toml`）。
    pub profiles: Arc<Profiles>,
    /// 模型资料（资源目录的 `models/models-dev.json`）。
    pub table: Arc<ModelTable>,
    /// 空闲超时，平时是 [`IDLE`]。
    pub idle: Duration,
}

impl Models for Routes {
    fn port(&self, session: ForSession) -> Arc<dyn ModelPort> {
        let values = session.config.resolved.values();
        let pinned = provider::chat(&values);
        let first = pinned
            .as_deref()
            .and_then(|text| provider::target(&values, &self.profiles, text).ok());
        let limits = match &first {
            Some(target) => {
                let facts = provider::facts(&values, &self.table, target);
                Limits {
                    model: model_of(target),
                    window: facts.window,
                    max_output: facts.max_output,
                    images: images(target.provider.images),
                }
            }
            None => Limits {
                model: none(),
                window: None,
                max_output: None,
                images: None,
            },
        };
        Arc::new(Route {
            shared: self.clone(),
            session: session.id,
            texts: session.texts,
            blobs: session.blobs,
            pinned: Mutex::new((pinned, limits.model.clone())),
            limits,
        })
    }
}

/// 一个会话的路由。
struct Route {
    shared: Routes,
    session: SessionId,
    texts: DriverTexts,
    blobs: Blobs,
    /// 钉着的引用，和上一次解析出来的模型（运行日志的 `request` 那一行写它）。
    pinned: Mutex<(Option<String>, Model)>,
    /// 造端口时定的限额。
    limits: Limits,
}

impl ModelPort for Route {
    fn model(&self) -> Model {
        self.pinned
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .1
            .clone()
    }

    fn limits(&self) -> Limits {
        self.limits.clone()
    }

    fn call(
        &self,
        _: Seq,
        request: Request,
        config: &TurnConfig,
        reports: Reports,
        cancel: Cancel,
    ) {
        match self.choose(config) {
            Ok(chosen) => send::spawn(chosen, request, reports, cancel),
            Err(NoModel(message)) => {
                tracing::warn!(target: TARGET, why = %message, "no model");
                reports.ended(
                    None,
                    Some(CallError {
                        class: ErrorClass::NoModel,
                        message,
                        status: None,
                    }),
                    None,
                    None,
                );
            }
        }
    }
}

impl Route {
    /// 这一次请求发给谁、带哪个 key，照这一轮的配置 `config`。
    fn choose(&self, config: &TurnConfig) -> Result<send::Chosen, NoModel> {
        let values = config.resolved.values();
        let profiles = &self.shared.profiles;
        let mut pinned = self.pinned.lock().unwrap_or_else(PoisonError::into_inner);
        let target = match pinned
            .0
            .as_deref()
            .map(|text| provider::target(&values, profiles, text))
        {
            Some(Ok(target)) => target,
            stale => {
                let chat = provider::chat(&values);
                let target = match (&chat, stale) {
                    (Some(chat), _) => provider::target(&values, profiles, chat)?,
                    (None, Some(Err(error))) => return Err(error),
                    (None, _) => return Err(NoModel(NOT_CONFIGURED.to_string())),
                };
                pinned.0 = chat;
                target
            }
        };
        pinned.1 = model_of(&target);
        drop(pinned);
        let endpoint = self.endpoint(config, &target)?;
        let model = ModelName::parse(&target.model).map_err(|error| NoModel(error.to_string()))?;
        Ok(send::Chosen {
            client: self.shared.client.clone(),
            model: model_of(&target),
            endpoint,
            driver: OpenAiChat::new(target.provider.compat.clone(), self.texts.clone()),
            call: Call {
                model,
                max_output: None,
                inputs: target.provider.inputs,
            },
            blobs: self.blobs.clone(),
            idle: self.shared.idle,
        })
    }

    /// 发到哪：这一家的地址，带会话钉着的那一个 key（取不到值的照写的先后取下一个）。没写 key 的不带。
    fn endpoint(&self, config: &TurnConfig, target: &Target) -> Result<Endpoint, NoModel> {
        let provider = &target.provider;
        if provider.keys.is_empty() {
            return Ok(Endpoint::keyless(&provider.base_url));
        }
        keys::order(self.session.as_str(), provider.keys.len())
            .into_iter()
            .find_map(|at| config.secret(&provider.keys[at]))
            .map(|key| Endpoint::new(&provider.base_url, key.expose()))
            .ok_or_else(|| NoModel(format!("provider {:?} has no usable key", provider.id)))
    }
}

/// 记进 `model.called` 的端点和模型。
fn model_of(target: &Target) -> Model {
    match (
        ProviderId::parse(&target.provider.id),
        ModelName::parse(&target.model),
    ) {
        (Ok(endpoint), Ok(model)) => Model { endpoint, model },
        _ => none(),
    }
}

/// 还没有模型时写的：端点、模型都是 `none`。
fn none() -> Model {
    let name = "none";
    Model {
        endpoint: ProviderId::parse(name)
            .unwrap_or_else(|e| unreachable!("「{name}」合端点编号的写法：{e}")),
        model: ModelName::parse(name)
            .unwrap_or_else(|e| unreachable!("「{name}」合模型名的写法：{e}")),
    }
}

/// 一张图怎么算：照档案。
fn images(tokens: Option<ImageTokens>) -> Option<Arc<dyn ImagePrice>> {
    match tokens? {
        ImageTokens::DeepSeek => Some(Arc::new(DeepSeekImages)),
    }
}
