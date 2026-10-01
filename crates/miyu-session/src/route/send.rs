//! 发一次（`docs/designs/05-内核接口.md` 第七节「驱动的规格」「HTTP 执行器」，施工 3-7 下；施工 8-6 从 `http.rs` 挪来）：
//! 一次请求在派出去的任务里走完，不占 actor：照驱动列的清单在阻塞线程里取 blob，编码成字节，经 `miyu_http::send` 发出去、
//! 流式读回来，回报交回 actor。任务带着会话的 span，HTTP 的日志写在会话编号后面。报上下文超长、说了上限、比手头的窗口
//! 小的，记下用出来的窗口（施工 8-7，`models.md`「怎么走」第二条第 9 条）。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use tracing::Instrument;

use miyu_drivers::{Call, Driver, EncodeError, OpenAiChat};
use miyu_http::{Attempt, Client, Endpoint, Outcome, Progress, send};
use miyu_kernel::event::{CallError, ErrorClass};
use miyu_kernel::id::ContentHash;
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_store::blob::Blobs;

use crate::blocking::blocking;
use crate::clock::wall_now;
use crate::port::{Cancel, Reports};
use crate::route::shared::ModelData;

/// 挑定了的这一次：发给谁、怎么编码、编码要的 blob 在哪。
pub(super) struct Chosen {
    pub(super) client: Client,
    /// 记进 `model.called` 的端点和模型。
    pub(super) model: Model,
    pub(super) endpoint: Endpoint,
    pub(super) driver: OpenAiChat,
    pub(super) call: Call,
    pub(super) blobs: Blobs,
    pub(super) idle: Duration,
    /// 报了上限时记到哪。
    pub(super) learn: Learn,
}

/// 用出来的窗口记到哪：这一家、这个模型，和发的时候手头的窗口。
pub(super) struct Learn {
    pub(super) data: Arc<ModelData>,
    pub(super) provider: String,
    pub(super) model: String,
    pub(super) window: Option<u64>,
}

impl Learn {
    /// 报了的上限 `limit` 比手头的窗口小（或者手头没有窗口）：记下。
    async fn limit(self, limit: u64) {
        if self.window.is_some_and(|window| window <= limit) {
            return;
        }
        blocking(move || {
            self.data
                .learn(&self.provider, &self.model, limit, wall_now());
        })
        .await;
    }
}

/// 在派出去的任务里发，带着当前的 span。
pub(super) fn spawn(chosen: Chosen, request: Request, reports: Reports, cancel: Cancel) {
    tokio::spawn(ask(chosen, request, reports, cancel).instrument(tracing::Span::current()));
}

/// 请求一次：取 blob、编码、发、把回报交回 actor。被叫停的什么都不再报。
async fn ask(chosen: Chosen, request: Request, reports: Reports, cancel: Cancel) {
    let needed = chosen.driver.blobs_needed(&request, &chosen.call);
    let blobs = chosen.blobs.clone();
    let fetched = blocking(move || fetch(&blobs, needed)).await;
    let encoded = match chosen.driver.encode(&request, &chosen.call, &fetched) {
        Ok(encoded) => encoded,
        Err(EncodeError::MissingBlob(hash)) => {
            return reports.ended(None, Some(missing(&hash)), None, None);
        }
    };
    let attempt = Attempt {
        client: &chosen.client,
        endpoint: &chosen.endpoint,
        driver: &chosen.driver,
        body: &encoded.body,
        path: &encoded.path,
        idle: chosen.idle,
    };
    let outcome = send(attempt, cancel.wait(), |progress| match progress {
        Progress::Sent { request } => reports.sent(chosen.model.clone(), request),
        Progress::Delta(delta) => reports.delta(delta),
    })
    .await;
    if let Outcome::Ended { usage, error } = outcome {
        let wait_ms = error
            .as_ref()
            .and_then(|classified| classified.retry_after_ms);
        let excess = error.as_ref().and_then(|classified| classified.excess);
        if let Some(limit) = error.as_ref().and_then(|classified| classified.limit) {
            chosen.learn.limit(limit).await;
        }
        reports.ended(
            usage,
            error.map(|classified| classified.error),
            wait_ms,
            excess,
        );
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
        status: None,
    }
}
