//! 请求模型的那几样（施工 3-7 下；施工 4-7 上从 `actor.rs` 挪出来，那边放不下了）：交给端口、叫停、说完了记一行
//! 收场（`28-运行日志.md` 第三节）。

use std::time::Instant;

use tokio::sync::oneshot;

use miyu_kernel::event::{CallError, Usage};
use miyu_kernel::id::Seq;
use miyu_kernel::request::{Difference, Request};

use super::{Actor, answer};
use crate::TARGET;
use crate::lines::{millis, where_};
use crate::port::{Cancel, Reports};

impl Actor {
    /// 请求模型：交给端口，记下叫停它的那一头和这一刻。前缀和上一次比变了的，运行日志里写上第一处不同在哪
    /// （施工 3-9 下）：缓存没命中时，一看就知道是不是我们的前缀变了。
    pub(super) fn call(&mut self, seen: Seq, request: Request, changed: Option<Difference>) {
        let model = self.model.model();
        tracing::info!(
            target: TARGET,
            seen = seen.get(),
            endpoint = model.endpoint.as_str(),
            model = model.model.as_str(),
            changed = changed.map(|changed| where_(&changed)),
            "request"
        );
        let (stop, cancel) = oneshot::channel();
        self.calls.insert(seen, (stop, Instant::now()));
        let reports = Reports::new(seen, self.backs.clone());
        self.model.call(seen, request, reports, Cancel::new(cancel));
    }

    /// 不要请求 `seen` 了：叫端口停下。
    pub(super) fn cancel(&mut self, seen: Seq) {
        let Some((stop, asked)) = self.calls.remove(&seen) else {
            return;
        };
        answer(stop, ());
        tracing::info!(
            target: TARGET,
            seen = seen.get(),
            took_ms = millis(asked.elapsed()),
            "cancelled"
        );
    }

    /// 请求 `seen` 说完了：不用再叫停它了；记一行收场（`28-运行日志.md` 第三节）。
    pub(super) fn ended(&mut self, seen: Seq, usage: Option<&Usage>, error: Option<&CallError>) {
        let Some((_, asked)) = self.calls.remove(&seen) else {
            return;
        };
        let took_ms = millis(asked.elapsed());
        match error {
            Some(error) => tracing::info!(
                target: TARGET,
                seen = seen.get(),
                took_ms,
                class = error.class.as_str(),
                "failed"
            ),
            None => tracing::info!(
                target: TARGET,
                seen = seen.get(),
                took_ms,
                "in" = usage.map(|usage| usage
                    .uncached
                    .saturating_add(usage.cache_read)
                    .saturating_add(usage.cache_write)),
                hit = usage.map(|usage| usage.cache_read),
                write = usage.map(|usage| usage.cache_write).filter(|written| *written > 0),
                out = usage.map(|usage| usage.output),
                "ended"
            ),
        }
    }
}
