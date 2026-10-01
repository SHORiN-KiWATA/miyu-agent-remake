//! 每个会话的路由（`docs/blueprint/models.md`「怎么走」第一条、第三条、第四条 8-6、8-8 那一半，`session/actor.md` 第 8 条，
//! 施工 8-6）：请求模型的端口的真实现，取代原来照环境变量接一个端点的 `HttpModels`。
//!
//! - 造端口时（造会话、载入）记下这个会话用的引用：`session.created` 的 `model`（施工 8-8，模型或 `@池`），以前的日志没有的
//!   照那一刻的 `models.chat`。限额（窗口、最大输出、一张图怎么算）照它定，交给内核（会变随 8-10）。
//! - 池（施工 8-8，`route/pool.rs`）：钉住的造端口时就钉上一个成员（载入的照最近一条发出去了的 `model.called` 认回来，认不
//!   出的取指针指的），以后一直发给它；轮换的每次请求从指针指的成员起。钉着的、指到的那个这时用不了（那一家推不出来、key
//!   取不到），照池里的先后取下一个，钉住的以后钉在它上面。出错才换、冷却随 8-9。
//! - 每一次请求照这一轮冻结的配置（[`crate::TurnConfig`]）重新解析：钉着的引用解析得出就用它；解析不出的退回这一轮的
//!   `models.chat`，退得回去的以后就钉在它上面；都不行的当场说完，分类 `no_model`，不发（第一条第 7 条）。
//! - key：这一家写了几个，会话钉在照会话编号算出的那一个上（`miyu_models::keys`），取不到值的跳过，照写的先后取下一个；
//!   一个都取不到的也是 `no_model`。出错换 key、换端点随 8-9。没写 key 的不带认证头（本机的服务）。
//! - 地址：写死的直接用，是环境变量的引用的照 `config.secret` 取（施工 8-6b，`miyu_models::provider::resolve_base_url`），
//!   和取 key 同一个办法；取不到也是 `no_model`，地址不会流进请求之外的任何地方。
//! - 发：照驱动编码、经 HTTP 执行器发、流式读回来（[`send`]），和原来一样。
//! - 资料（施工 8-7）：窗口、最大输出、能收什么照核心一份的模型资料查（[`ModelData`]，`miyu_models::facts`）；目录在写了
//!   `ready` 以后才读完，造端口之前先等它（[`Models::ready`]）。报上下文超长、说了上限、比手头的窗口小的，记下用出来的
//!   窗口（第二条第 9 条）：新造的、载入的会话用上，开着的会话限额会变随 8-10。

mod lists;
mod pool;
mod send;
pub(crate) mod shared;

pub use lists::{STALE, refresh_list};
pub use shared::{ModelData, Observed, read_observed};

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
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
use miyu_models::facts::{Facts, facts};
use miyu_models::keys;
use miyu_models::pools::Member;
use miyu_models::profile::ImageTokens;
use miyu_models::provider::{self, NOT_CONFIGURED, NoModel, Target};
use miyu_models::reference::{Resolved, resolve};
use miyu_store::blob::Blobs;

use crate::TARGET;
use crate::config::TurnConfig;
use crate::port::{Cancel, ForSession, ModelPort, Models, Reports};

/// 空闲超时的初值：多久没收到新的字节就算断了（`05-内核接口.md` 第七节，以后按思考强度放大）。
pub const IDLE: Duration = Duration::from_secs(180);

/// 核心一份的：HTTP 客户端、模型资料（档案、目录、用出来的、供应商的列表）、空闲超时。给每个会话造一个路由。
#[derive(Clone)]
pub struct Routes {
    /// HTTP 客户端：一个核心一个，连接跨请求复用。
    pub client: Client,
    /// 模型资料（施工 8-7）。
    pub data: Arc<ModelData>,
    /// 空闲超时，平时是 [`IDLE`]。
    pub idle: Duration,
}

impl Models for Routes {
    fn ready(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(self.data.wait())
    }

    fn port(&self, session: ForSession) -> Arc<dyn ModelPort> {
        let config = &session.config;
        let values = config.resolved.values();
        let reference = session
            .reference
            .clone()
            .or_else(|| provider::chat(&values));
        let resolved = reference.as_deref().and_then(|text| {
            self.data
                .with(|knowledge| resolve(&values, knowledge, text))
                .ok()
        });
        let (member, limits) = match resolved {
            Some(Resolved::Model(target)) => (None, self.limits(config, &target)),
            Some(Resolved::Pool(pool)) => self.pool_start(config, &pool, session.sent.as_ref()),
            None => (None, nothing()),
        };
        Arc::new(Route {
            shared: self.clone(),
            session: session.id,
            texts: session.texts,
            blobs: session.blobs,
            pinned: Mutex::new(Pinned {
                reference,
                member,
                last: limits.model.clone(),
            }),
            limits,
        })
    }
}

impl Routes {
    /// 发给 `target` 的限额：窗口、最大输出照模型资料，一张图怎么算照档案。
    fn limits(&self, config: &TurnConfig, target: &Target) -> Limits {
        let (facts, _) = self
            .data
            .with(|knowledge| facts(&config.resolved, knowledge, &target.provider, &target.model));
        Limits {
            model: model_of(target),
            window: facts.window.value,
            max_output: facts.max_output.value,
            images: images(target.provider.images),
        }
    }
}

/// 一个会话的路由。
struct Route {
    shared: Routes,
    session: SessionId,
    texts: DriverTexts,
    blobs: Blobs,
    /// 钉着的：引用、池里的成员、上一次解析出来的模型。
    pinned: Mutex<Pinned>,
    /// 造端口时定的限额。
    limits: Limits,
}

/// 一个会话钉着的（施工 8-6、8-8）。
struct Pinned {
    /// 会话的引用：模型或 `@池`。解析不出、退回 `models.chat` 的，以后钉在它上面；`models.chat` 也没配的是空的。
    reference: Option<String>,
    /// 引用是钉住的池的：钉着的那个成员。别的没有。
    member: Option<Member>,
    /// 上一次解析出来的模型：运行日志的 `request` 那一行写它。
    last: Model,
}

impl ModelPort for Route {
    fn model(&self) -> Model {
        self.lock().last.clone()
    }

    fn reference(&self) -> Option<String> {
        self.lock().reference.clone()
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
    fn lock(&self) -> MutexGuard<'_, Pinned> {
        self.pinned.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 这一次请求发给谁、带哪个 key，照这一轮的配置 `config`。钉着的引用解析不出（供应商、池删了，池空了）的退回这一轮
    /// 的 `models.chat`，以后钉在它上面；池照钉住、轮换挑成员（`route/pool.rs`）。
    fn choose(&self, config: &TurnConfig) -> Result<send::Chosen, NoModel> {
        let values = config.resolved.values();
        let data = &self.shared.data;
        let resolve = |text: &str| data.with(|knowledge| resolve(&values, knowledge, text));
        let mut pinned = self.lock();
        let resolved = match pinned.reference.as_deref().map(resolve) {
            Some(Ok(resolved)) => resolved,
            stale => {
                let chat = provider::chat(&values);
                let resolved = match (&chat, stale) {
                    (Some(chat), _) => resolve(chat)?,
                    (None, Some(Err(error))) => return Err(error),
                    (None, _) => return Err(NoModel(NOT_CONFIGURED.to_string())),
                };
                pinned.reference = chat;
                pinned.member = None;
                resolved
            }
        };
        let (target, endpoint) = match resolved {
            Resolved::Model(target) => {
                let endpoint = self.endpoint(config, &target)?;
                (target, endpoint)
            }
            Resolved::Pool(pool) => self.member(config, &values, &pool, &mut pinned)?,
        };
        pinned.last = model_of(&target);
        drop(pinned);
        let model = ModelName::parse(&target.model).map_err(|error| NoModel(error.to_string()))?;
        let (facts, _): (Facts, _) = data
            .with(|knowledge| facts(&config.resolved, knowledge, &target.provider, &target.model));
        Ok(send::Chosen {
            client: self.shared.client.clone(),
            model: model_of(&target),
            endpoint,
            driver: OpenAiChat::new(target.provider.compat.clone(), self.texts.clone()),
            call: Call {
                model,
                max_output: None,
                inputs: facts.driver_inputs(),
            },
            blobs: self.blobs.clone(),
            idle: self.shared.idle,
            learn: send::Learn {
                data: Arc::clone(data),
                provider: target.provider.id.clone(),
                model: target.model.clone(),
                window: facts.window.value,
            },
        })
    }

    /// 发到哪：这一家的地址（写死的直接用，是环境变量的引用照 `config.secret` 取，施工 8-6b），带会话钉着的那一个 key
    /// （取不到值的照写的先后取下一个）。没写 key 的不带。
    fn endpoint(&self, config: &TurnConfig, target: &Target) -> Result<Endpoint, NoModel> {
        let provider = &target.provider;
        let base_url = provider::resolve_base_url(provider, &|reference| config.secret(reference))?;
        if provider.keys.is_empty() {
            return Ok(Endpoint::keyless(&base_url));
        }
        keys::order(self.session.as_str(), provider.keys.len())
            .into_iter()
            .find_map(|at| config.secret(&provider.keys[at]))
            .map(|key| Endpoint::new(&base_url, key.expose()))
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

/// 还没有模型、引用解析不出时的限额：模型是 `none`，别的都没有（不主动压）。
fn nothing() -> Limits {
    Limits {
        model: none(),
        window: None,
        max_output: None,
        images: None,
    }
}

/// 还没有模型时写的：端点、模型都是 `none`。轮换的池也写它（「怎么走」第三条第 7 条：每次请求的模型都不一样，锚总是
/// 对不上，用量全靠本地估）。
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
