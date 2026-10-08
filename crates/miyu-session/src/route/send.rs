//! 会话入口发一次（`docs/designs/05-内核接口.md` 第七节「驱动的规格」「HTTP 执行器」，施工 3-7 下；施工 8-6 从 `http.rs`
//! 挪来；施工 8-20 起取 blob、编码、发都调底子 `route/exchange.rs`）：一次请求在派出去的任务里走完，不占 actor，回报交回
//! actor。任务带着会话的 span，HTTP 的日志写在会话编号后面。说完了先交给路由记（施工 8-9，[`Tried`]：冷却、换端点照底子
//! 的 `route/ended.rs`，会话自己的几样在这里），出错换了端点的照「换了端点」报。
//!
//! 会话自己记的（`docs/blueprint/models.md`「怎么走」第四条第 3、5、6 条）：
//! - 成了：钉住的池，钉着的成员换成它（限额跟着换成它的）；主请求的「说到一半断了」放开。
//! - 出错：收到过增量才出错的，主请求记下它，下一次主请求还发给它；别的放开。
//! - 被叫停：什么都不记，主请求的「说到一半断了」放开。

use std::sync::{Arc, Mutex};

use tracing::Instrument;

use miyu_http::Progress;
use miyu_kernel::event::ErrorClass;
use miyu_kernel::request::Request;
use miyu_store::blob::Blobs;

use super::base::Ready;
use super::ended::{Picked, Switch};
use super::exchange::{Exchanged, exchange};
use super::{Pinned, lock};
use crate::config::TurnConfig;
use crate::port::{Cancel, Reports};

/// 挑定了的这一次：底子备好的、编码要的 blob 在哪、说完了会话记什么。
pub(super) struct Chosen {
    pub(super) ready: Ready,
    pub(super) blobs: Blobs,
    /// 说完了路由照它记（施工 8-9）。
    pub(super) tried: Tried,
}

/// 发出去的这一次，会话这一头：底子挑中的那一次，和会话钉着的那一份。
pub(super) struct Tried {
    /// 底子挑中的：冷却照它记。
    pub(super) picked: Picked,
    /// 会话钉着的那一份。
    pub(super) pinned: Arc<Mutex<Pinned>>,
    /// 这一轮的配置：换成员时照它算限额。
    pub(super) config: TurnConfig,
    /// 是主请求（摘要请求也算）：只有它认「说到一半断了」。
    pub(super) main: bool,
    /// 引用是钉住的池：成了就钉到这个成员。
    pub(super) pins: bool,
}

impl Tried {
    /// 成了。
    fn succeeded(self) {
        self.picked.succeeded();
        let picked = &self.picked.choice;
        let changed =
            self.pins && picked.member.is_some() && lock(&self.pinned).member != picked.member;
        // 限额照模型资料算，不拿着钉着的锁算。
        let limits = changed.then(|| self.picked.routes.limits(&self.config, &picked.target));
        let mut pinned = lock(&self.pinned);
        if self.main {
            pinned.sticky = None;
        }
        if let Some(limits) = limits {
            pinned.member.clone_from(&picked.member);
            pinned.limits = limits;
        }
    }

    /// 出错了，分类 `class`，供应商说了要等 `said_ms` 毫秒（没说的没有）；`cut`：收到过增量才出错的。
    fn failed(self, class: &ErrorClass, said_ms: Option<u64>, cut: bool) -> Switch {
        let switch = self.picked.failed(class, said_ms, cut);
        if self.main {
            lock(&self.pinned).sticky = cut.then(|| self.picked.choice.who.clone());
        }
        switch
    }

    /// 被叫停了：什么都不记，主请求的「说到一半断了」放开。
    fn cancelled(self) {
        if self.main {
            lock(&self.pinned).sticky = None;
        }
    }
}

/// 在派出去的任务里发，带着当前的 span。
pub(super) fn spawn(chosen: Chosen, request: Request, reports: Reports, cancel: Cancel) {
    tokio::spawn(ask(chosen, request, reports, cancel).instrument(tracing::Span::current()));
}

/// 请求一次：经底子发，把回报交回 actor。被叫停的什么都不再报。说完了照这个模型的价格算金额（施工 8-15）。
async fn ask(chosen: Chosen, request: Request, reports: Reports, cancel: Cancel) {
    let ready = &chosen.ready;
    let reports = reports.billed(ready.tariff.clone());
    let mut received = false;
    let exchanged =
        exchange(
            ready,
            &request,
            &chosen.blobs,
            cancel.wait(),
            |progress| match progress {
                Progress::Sent { request } => reports.sent(ready.model.clone(), request),
                Progress::Delta(delta) => {
                    received = true;
                    reports.delta(delta);
                }
            },
        )
        .await;
    let (usage, classified) = match exchanged {
        Exchanged::Missing(error) => {
            chosen.tried.failed(&error.class, None, false);
            return reports.ended(None, Some(error), None, None);
        }
        Exchanged::Cancelled => return chosen.tried.cancelled(),
        Exchanged::Ended { usage, error: None } => {
            chosen.tried.succeeded();
            return reports.ended(usage, None, None, None);
        }
        Exchanged::Ended {
            usage,
            error: Some(classified),
        } => (usage, classified),
    };
    let class = &classified.error.class;
    // 池的成员回了 404：后台确认它是不是下架了（施工 8-23）。
    chosen
        .tried
        .picked
        .check_gone(&chosen.tried.config, classified.error.status);
    let switch = chosen
        .tried
        .failed(class, classified.retry_after_ms, received);
    match switch.failover {
        true => reports.failed_over(usage, classified.error, switch.wait_ms),
        false => reports.ended(
            usage,
            Some(classified.error),
            switch.wait_ms,
            classified.excess,
        ),
    }
}
