//! 发一次（`docs/designs/05-内核接口.md` 第七节「驱动的规格」「HTTP 执行器」，施工 3-7 下；施工 8-20 从 `route/send.rs`
//! 拆出来，是底子的一块，两个入口共用）：照驱动列的清单在阻塞线程里取 blob，编码成字节，经 `miyu_http::send` 发出去、
//! 流式读回来，「发出去了」和每一段增量交给挑的一方。报上下文超长、说了上限、比手头的窗口小的，记下用出来的窗口（施工
//! 8-7，`models.md`「怎么走」第二条第 9 条），再交回说完了。

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::sync::Arc;

use miyu_drivers::EncodeError;
use miyu_drivers::classify::Classified;
use miyu_http::{Attempt, Outcome, Progress, send};
use miyu_kernel::event::{CallError, ErrorClass, Usage};
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::Request;
use miyu_store::blob::Blobs;

use super::base::Ready;
use crate::blocking::blocking;
use crate::clock::wall_now;
use crate::route::shared::ModelData;

/// 用出来的窗口记到哪：这一家、这个模型，和发的时候手头的窗口。
pub(super) struct Learn {
    pub(super) data: Arc<ModelData>,
    pub(super) provider: String,
    pub(super) model: String,
    pub(super) window: Option<u64>,
}

impl Learn {
    /// 报了的上限 `limit` 比手头的窗口小（或者手头没有窗口）：记下。
    async fn limit(&self, limit: u64) {
        if self.window.is_some_and(|window| window <= limit) {
            return;
        }
        let (data, provider, model) = (
            Arc::clone(&self.data),
            self.provider.clone(),
            self.model.clone(),
        );
        blocking(move || data.learn(&provider, &model, limit, wall_now())).await;
    }
}

/// 发一次怎么收场的。
pub(super) enum Exchanged {
    /// 编码要的 blob 取不出来：没发出去（分类「其他」，重试也没用）。
    Missing(CallError),
    /// 被叫停了：之后什么都没报。
    Cancelled,
    /// 说完了：用量，出错的分类、原话、要等多久（报了上限的已经记下了）。
    Ended {
        usage: Option<Usage>,
        error: Option<Classified>,
    },
}

/// 发一次：取 blob、编码、发、读回来，「发出去了」和增量一有就交给 `on`；`cancel` 一完成就停。
pub(super) async fn exchange(
    ready: &Ready,
    request: &Request,
    blobs: &Blobs,
    cancel: impl Future<Output = ()> + Send,
    on: impl FnMut(Progress) + Send,
) -> Exchanged {
    let needed = ready.driver.blobs_needed(request, &ready.call);
    let blobs = blobs.clone();
    let fetched = blocking(move || fetch(&blobs, needed)).await;
    let encoded = match ready.driver.encode(request, &ready.call, &fetched) {
        Ok(encoded) => encoded,
        Err(EncodeError::MissingBlob(hash)) => return Exchanged::Missing(missing(&hash)),
    };
    let attempt = Attempt {
        client: &ready.client,
        endpoint: &ready.endpoint,
        driver: ready.driver.as_ref(),
        body: &encoded.body,
        path: &encoded.path,
        idle: ready.idle,
    };
    match send(attempt, cancel, on).await {
        Outcome::Cancelled => Exchanged::Cancelled,
        Outcome::Ended { usage, error } => {
            if let Some(limit) = error.as_ref().and_then(|classified| classified.limit) {
                ready.learn.limit(limit).await;
            }
            Exchanged::Ended { usage, error }
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
        status: None,
    }
}
