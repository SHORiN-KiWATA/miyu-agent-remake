//! 每个会话的路由（`docs/blueprint/models.md`「怎么走」第一条、第三条、第四条、第五条，`session/actor.md` 第 8 条，施工
//! 8-6 起）：请求模型的端口的真实现，取代原来照环境变量接一个端点的 `HttpModels`。
//!
//! - 造端口时（造会话、载入）记下这个会话用的引用：`session.created` 的 `model`（施工 8-8，模型或 `@池`），以前的日志没有的
//!   照那一刻的 `models.chat`。限额（窗口、最大输出、一张图怎么算）照它定，交给内核；钉住的池钉着的成员换了，限额跟着换
//!   （施工 8-9，会话 actor 比了交给内核）。
//! - 池（施工 8-8，`route/pool.rs`）：钉住的造端口时就钉上一个成员（载入的照最近一条发出去了的 `model.called` 认回来，认不
//!   出的取指针指的），以后先发给它；轮换的每次请求从指针指的成员起。
//! - 每一次请求照这一轮冻结的配置（[`crate::TurnConfig`]）重新解析：钉着的引用解析得出就用它；解析不出的退回这一轮的
//!   `models.chat`，退得回去的以后就钉在它上面；都不行的当场说完，分类 `no_model`，不发（第一条第 7 条）。
//! - 候选（施工 8-9，`route/choice.rs`）：一个候选是一家、一个 key、一个模型。会话照编号钉一个 key（`miyu_models::keys`），
//!   出错换过去、成了的那一个以后在前；取不到值的 key 不当候选。池照先后排下来每个成员的。取排在最前、没在冷却的；上一次
//!   主请求说到一半断了的还发给它；都在冷却的，只有一个候选的照发，不止一个的不发、交 `cooling`。
//! - 说完了（施工 8-9，`route/ended.rs`）：出错的照分类记冷却，还有别的候选的说「换了端点」（`failover`），内核当场再来；
//!   成了的清零，钉住的池钉到它，会话的 key 换成它。
//! - 地址：写死的直接用，是环境变量的引用的照 `config.secret` 取（施工 8-6b，`miyu_models::provider::resolve_base_url`），
//!   和取 key 同一个办法；取不到也是 `no_model`，地址不会流进请求之外的任何地方。
//! - 发：照驱动编码、经 HTTP 执行器发、流式读回来（[`send`]），和原来一样。
//! - 资料（施工 8-7）：窗口、最大输出、能收什么照核心一份的模型资料查（[`ModelData`]，`miyu_models::facts`）；目录在写了
//!   `ready` 以后才读完，造端口之前先等它（[`Models::ready`]）。报上下文超长、说了上限、比手头的窗口小的，记下用出来的
//!   窗口（第二条第 9 条）：新造的、载入的会话用上，开着的会话限额会变随 8-10。

mod choice;
mod ended;
mod lists;
mod pool;
mod send;
pub(crate) mod shared;

pub use lists::{STALE, refresh_list};
pub use shared::{ModelData, Observed, read_observed};

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use miyu_drivers::DriverTexts;
use miyu_drivers::{Call, DeepSeekImages, OpenAiChat};
use miyu_http::Client;
use miyu_kernel::estimate::ImagePrice;
use miyu_kernel::event::{CallError, ErrorClass};
use miyu_kernel::id::{ModelName, ProviderId, Seq, SessionId};
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_kernel::session::Limits;
use miyu_models::cooldown::Candidate;
use miyu_models::facts::{Facts, facts};
use miyu_models::pools::{Member, Strategy};
use miyu_models::profile::ImageTokens;
use miyu_models::provider::{self, NOT_CONFIGURED, NoModel, Target};
use miyu_models::reference::{Resolved, resolve};
use miyu_store::blob::Blobs;

use crate::TARGET;
use crate::clock::wall_now;
use crate::config::TurnConfig;
use crate::port::{Cancel, ForSession, ModelPort, Models, Reports};
use choice::Unsent;

/// 空闲超时的初值：多久没收到新的字节就算断了（`05-内核接口.md` 第七节，以后按思考强度放大）。
pub const IDLE: Duration = Duration::from_secs(180);

/// 还没有模型、轮换的池的限额里写的端点和模型（施工 8-9 起会话 actor 也认它：限额里的模型是它的，不推 `model.changed`）。
pub(crate) const NONE: &str = "none";

/// 核心一份的：HTTP 客户端、模型资料（档案、目录、用出来的、供应商的列表、冷却表）、空闲超时。给每个会话造一个路由。
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
            pinned: Arc::new(Mutex::new(Pinned {
                reference,
                member,
                last: limits.model.clone(),
                limits,
                moved: BTreeMap::new(),
                sticky: None,
            })),
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
    /// 钉着的：引用、池里的成员、上一次解析出来的模型、限额、换过去的 key、要接着说的那一个。发出去的任务说完了也要改它
    /// （成了才钉，施工 8-9），所以放在 `Arc` 里。
    pinned: Arc<Mutex<Pinned>>,
}

/// 一个会话钉着的（施工 8-6、8-8、8-9）。
struct Pinned {
    /// 会话的引用：模型或 `@池`。解析不出、退回 `models.chat` 的，以后钉在它上面；`models.chat` 也没配的是空的。
    reference: Option<String>,
    /// 引用是钉住的池的：钉着的那个成员（成了才换，施工 8-9）。别的没有。
    member: Option<Member>,
    /// 上一次解析出来的模型：运行日志的 `request` 那一行写它。
    last: Model,
    /// 交给内核的限额：造端口时定，钉住的池钉着的成员换了跟着换（施工 8-9）。
    limits: Limits,
    /// 出错换过去、成了的 key（施工 8-9，第一条第 6 条）：供应商的编号 → key 的名字。以后这一家先用它，只在内存里。
    moved: BTreeMap<String, String>,
    /// 上一次主请求收到过增量、然后出错的那一个（施工 8-9，第四条第 3 条）：下一次主请求不挑，还发给它。说完了就放开。
    sticky: Option<Candidate>,
}

impl ModelPort for Route {
    fn model(&self) -> Model {
        self.lock().last.clone()
    }

    fn reference(&self) -> Option<String> {
        self.lock().reference.clone()
    }

    fn limits(&self) -> Limits {
        self.lock().limits.clone()
    }

    fn call(
        &self,
        _: Seq,
        request: Request,
        config: &TurnConfig,
        reports: Reports,
        cancel: Cancel,
    ) {
        let main = reports.purpose().is_none();
        match self.choose(config, main) {
            Ok(chosen) => send::spawn(chosen, request, reports, cancel),
            Err(Unsent::NoModel(NoModel(message))) => {
                tracing::warn!(target: TARGET, why = %message, "no model");
                reports.ended(None, Some(unsent(ErrorClass::NoModel, message)), None, None);
            }
            Err(Unsent::Cooling { message, wait_ms }) => {
                reports.ended(
                    None,
                    Some(unsent(ErrorClass::Cooling, message)),
                    Some(wait_ms),
                    None,
                );
            }
        }
    }
}

impl Route {
    fn lock(&self) -> MutexGuard<'_, Pinned> {
        lock(&self.pinned)
    }

    /// 这一次请求发给谁、带哪个 key，照这一轮的配置 `config`。钉着的引用解析不出（供应商、池删了，池空了）的退回这一轮
    /// 的 `models.chat`，以后钉在它上面；照第四条排候选、挑一个（`route/choice.rs`、`route/pool.rs`）。`main`：主请求才认
    /// 「说到一半断了还发给它」。
    fn choose(&self, config: &TurnConfig, main: bool) -> Result<send::Chosen, Unsent> {
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
                    (None, Some(Err(error))) => return Err(error.into()),
                    (None, _) => return Err(NoModel(NOT_CONFIGURED.to_string()).into()),
                };
                pinned.reference = chat;
                pinned.member = None;
                resolved
            }
        };
        let (mut choices, pins) = match resolved {
            Resolved::Model(target) => (self.choices(config, &target, None, &pinned.moved)?, false),
            Resolved::Pool(pool) => (
                self.members(config, &values, &pool, &pinned)?,
                pool.strategy == Strategy::Pin,
            ),
        };
        let sticky = pinned.sticky.as_ref().filter(|_| main);
        let at = choice::pick(&choices, sticky, data, wall_now())?;
        let picked = choices.remove(at);
        pinned.last = model_of(&picked.target);
        drop(pinned);
        let target = picked.target.clone();
        let model = ModelName::parse(&target.model).map_err(|error| NoModel(error.to_string()))?;
        let (facts, _): (Facts, _) = data
            .with(|knowledge| facts(&config.resolved, knowledge, &target.provider, &target.model));
        let endpoint = picked.endpoint.clone();
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
            tried: ended::Tried {
                pinned: Arc::clone(&self.pinned),
                routes: self.shared.clone(),
                config: Arc::clone(config),
                main,
                pins,
                picked,
                others: choices.into_iter().map(choice::Choice::who).collect(),
            },
        })
    }
}

/// 拿着钉着的那一份：锁坏了照样用（里面只是几样记着的，没有写到一半的）。
fn lock(pinned: &Mutex<Pinned>) -> MutexGuard<'_, Pinned> {
    pinned.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 没发出去就说完的：分类、原话，没有状态码。
fn unsent(class: ErrorClass, message: String) -> CallError {
    CallError {
        class,
        message,
        status: None,
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
    Model {
        endpoint: ProviderId::parse(NONE)
            .unwrap_or_else(|e| unreachable!("「{NONE}」合端点编号的写法：{e}")),
        model: ModelName::parse(NONE)
            .unwrap_or_else(|e| unreachable!("「{NONE}」合模型名的写法：{e}")),
    }
}

/// 一张图怎么算：照档案。
fn images(tokens: Option<ImageTokens>) -> Option<Arc<dyn ImagePrice>> {
    match tokens? {
        ImageTokens::DeepSeek => Some(Arc::new(DeepSeekImages)),
    }
}
