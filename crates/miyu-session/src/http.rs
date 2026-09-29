//! 经驱动和 HTTP 执行器请求模型（`docs/designs/05-内核接口.md` 第七节「驱动的规格」「HTTP 执行器」，
//! 施工 3-7 下）：请求模型的端口的真实现。
//!
//! 一次请求在派出去的任务里走完，不占 actor：照驱动列的清单在阻塞线程里取 blob，编码成字节，经
//! `miyu_http::send` 发出去、流式读回来，回报交回 actor。任务带着会话的 span，HTTP 的日志写在会话
//! 编号后面。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use tracing::Instrument;

use miyu_drivers::openai_chat::Compat;
use miyu_drivers::{Call, Driver, EncodeError, OpenAiChat};
use miyu_http::{Attempt, Client, Endpoint, Outcome, Progress, send};
use miyu_kernel::estimate::ImagePrice;
use miyu_kernel::event::{CallError, ErrorClass};
use miyu_kernel::id::{ContentHash, ProviderId, Seq};
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_kernel::session::Limits;
use miyu_store::blob::Blobs;

use crate::blocking::blocking;
use crate::port::{Cancel, ForSession, ModelPort, Models, Reports};

/// 空闲超时的初值：多久没收到新的字节就算断了（`05-内核接口.md` 第七节，以后按思考强度放大）。
pub const IDLE: Duration = Duration::from_secs(180);

/// 经 HTTP 请求一个端点上的一个模型：给每个会话造一个端口。
#[derive(Clone)]
pub struct HttpModels {
    /// HTTP 客户端：一个核心一个，连接跨请求复用。
    pub client: Client,
    /// 端点的编号：记进 `model.called`，和运行日志的 `endpoint`。
    pub provider: ProviderId,
    /// 端点的地址、key、另配的头。
    pub endpoint: Endpoint,
    /// 供应商之间不一样的几处：OpenAI 兼容的对话接口（`05-内核接口.md` 第七节）。
    pub compat: Compat,
    /// 这一次调用要定的：模型名、输出的上限、模型能收哪些输入。
    pub call: Call,
    /// 空闲超时，平时是 [`IDLE`]。
    pub idle: Duration,
    /// 模型的上下文窗口，照模型资料查的；查不到的没有，不主动压（施工 6-3 上）。
    pub window: Option<u64>,
    /// 模型的最大输出，同上。
    pub max_output: Option<u64>,
    /// 一张图怎么算：跟着驱动的写法走，DeepSeek 的交官方计算器的算法。
    pub images: Option<Arc<dyn ImagePrice>>,
}

impl Models for HttpModels {
    fn port(&self, session: ForSession) -> Arc<dyn ModelPort> {
        Arc::new(HttpModel(Arc::new(Route {
            client: self.client.clone(),
            model: Model {
                endpoint: self.provider.clone(),
                model: self.call.model.clone(),
            },
            endpoint: self.endpoint.clone(),
            driver: OpenAiChat::new(self.compat, session.texts),
            call: self.call.clone(),
            blobs: session.blobs,
            idle: self.idle,
            window: self.window,
            max_output: self.max_output,
            images: self.images.clone(),
        })))
    }
}

/// 一个会话的端口：一次请求派一个任务，任务拿着同一份 [`Route`]。
struct HttpModel(Arc<Route>);

/// 一个会话怎么请求模型：发给谁、怎么编码、编码要的 blob 在哪。
struct Route {
    client: Client,
    model: Model,
    endpoint: Endpoint,
    driver: OpenAiChat,
    call: Call,
    blobs: Blobs,
    idle: Duration,
    window: Option<u64>,
    max_output: Option<u64>,
    images: Option<Arc<dyn ImagePrice>>,
}

impl ModelPort for HttpModel {
    fn model(&self) -> &Model {
        &self.0.model
    }

    fn limits(&self) -> Limits {
        Limits {
            model: self.0.model.clone(),
            window: self.0.window,
            max_output: self.0.max_output,
            images: self.0.images.clone(),
        }
    }

    fn call(&self, _: Seq, request: Request, reports: Reports, cancel: Cancel) {
        let route = Arc::clone(&self.0);
        tokio::spawn(
            route
                .ask(request, reports, cancel)
                .instrument(tracing::Span::current()),
        );
    }
}

impl Route {
    /// 请求一次：取 blob、编码、发、把回报交回 actor。被叫停的什么都不再报。
    async fn ask(self: Arc<Route>, request: Request, reports: Reports, cancel: Cancel) {
        let needed = self.driver.blobs_needed(&request, &self.call);
        let blobs = self.blobs.clone();
        let fetched = blocking(move || fetch(&blobs, needed)).await;
        let encoded = match self.driver.encode(&request, &self.call, &fetched) {
            Ok(encoded) => encoded,
            Err(EncodeError::MissingBlob(hash)) => {
                return reports.ended(None, Some(missing(&hash)), None);
            }
        };
        let attempt = Attempt {
            client: &self.client,
            endpoint: &self.endpoint,
            driver: &self.driver,
            body: &encoded.body,
            path: encoded.path,
            idle: self.idle,
        };
        let outcome = send(attempt, cancel.wait(), |progress| match progress {
            Progress::Sent { request } => reports.sent(self.model.clone(), request),
            Progress::Delta(delta) => reports.delta(delta),
        })
        .await;
        if let Outcome::Ended { usage, error } = outcome {
            let wait_ms = error
                .as_ref()
                .and_then(|classified| classified.retry_after_ms);
            reports.ended(usage, error.map(|classified| classified.error), wait_ms);
        }
    }
}

/// 照清单取 blob。取不出来的（丢了、坏了、读不了）不在里面：编码时驱动报缺的是哪一个。
fn fetch(blobs: &Blobs, needed: BTreeSet<ContentHash>) -> BTreeMap<ContentHash, Vec<u8>> {
    needed
        .into_iter()
        .filter_map(|hash| blobs.get(&hash).ok().map(|bytes| (hash, bytes)))
        .collect()
}

/// 编码要的 blob 取不出来（`05-内核接口.md` 第七节）：分类「其他」，重试也没用。
fn missing(hash: &ContentHash) -> CallError {
    CallError {
        class: ErrorClass::Unclassified,
        message: format!("编码要用的 blob {hash} 取不出来"),
    }
}
