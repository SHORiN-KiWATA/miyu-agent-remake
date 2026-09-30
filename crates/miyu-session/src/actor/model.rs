//! 请求模型的那几样（施工 3-7 下；施工 4-7 上从 `actor.rs` 挪出来，那边放不下了）：交给端口、叫停、说完了记一行
//! 收场（`28-运行日志.md` 第三节）。辅助请求（施工 3-8 四补的回顾、五补的起标题）也在这里：同一个端口，回报另走一路。

use std::time::Instant;

use tokio::sync::oneshot;

use miyu_kernel::event::{CallError, Purpose, Usage};
use miyu_kernel::id::Seq;
use miyu_kernel::request::{Difference, Request};
use miyu_kernel::session::Input;
use miyu_kernel::time::Timestamp;

use super::{Actor, answer};
use crate::TARGET;
use crate::lines::{millis, where_};
use crate::port::{Cancel, Report, Reports};

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

    /// 发一次辅助请求（施工 3-8 四补的回顾、五补的起标题）：交给同一个端口，回报走辅助请求那一路，名字是用途和它照到的
    /// 那一条。叫停它的那一头拿着不用：内核不叫停辅助请求，actor 停了放下它，请求跟着停。运行日志那一行前面带用途
    /// （`recap request`、`title request`）。
    pub(super) fn aside(&mut self, purpose: Purpose, upto: Seq, request: Request) {
        let model = self.model.model();
        tracing::info!(
            target: TARGET,
            seen = upto.get(),
            endpoint = model.endpoint.as_str(),
            model = model.model.as_str(),
            "{} request",
            purpose.as_str()
        );
        let (stop, cancel) = oneshot::channel();
        self.asides.retain(|(asking, ..)| *asking != purpose);
        self.asides
            .push((purpose.clone(), upto, stop, Instant::now()));
        let reports = Reports::aside(purpose, upto, self.backs.clone());
        self.model.call(upto, request, reports, Cancel::new(cancel));
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
        finished(seen, asked, usage, error, "");
    }

    /// 辅助请求的一样回报，写成内核的输入；说完了的先记一行收场（施工 3-8 五补从 `actor.rs` 挪来，那边放不下了）。
    pub(super) fn aside_back(
        &mut self,
        at: Timestamp,
        purpose: Purpose,
        upto: Seq,
        report: Report,
    ) -> Input {
        match report {
            Report::Sent { model, request } => Input::AsideSent {
                at,
                purpose,
                upto,
                model,
                request,
            },
            Report::Delta(delta) => Input::AsideDelta {
                at,
                purpose,
                upto,
                delta,
            },
            Report::Ended { usage, error, .. } => {
                self.aside_ended(&purpose, upto, usage.as_ref(), error.as_ref());
                Input::AsideEnded {
                    at,
                    purpose,
                    upto,
                    usage,
                    error,
                }
            }
        }
    }

    /// 辅助请求说完了（施工 3-8 四补的回顾、五补的起标题）：记一行收场，和主请求的一样，前面带用途（`recap failed`、
    /// `title failed`……）。起标题两次都没起成就不再试，第二行 `title failed` 就是那一行。
    fn aside_ended(
        &mut self,
        purpose: &Purpose,
        upto: Seq,
        usage: Option<&Usage>,
        error: Option<&CallError>,
    ) {
        let Some(k) = self
            .asides
            .iter()
            .position(|(asking, seen, ..)| asking == purpose && *seen == upto)
        else {
            return;
        };
        let (_, _, _, asked) = self.asides.remove(k);
        finished(upto, asked, usage, error, &format!("{} ", purpose.as_str()));
    }
}

/// 一次请求收场的那一行：出错的写分类，说完了的写输入、命中、写进缓存的、输出。`what` 是主请求的空着，辅助请求的是用途加一个
/// 空格（`recap `、`title `）。
fn finished(
    seen: Seq,
    asked: Instant,
    usage: Option<&Usage>,
    error: Option<&CallError>,
    what: &str,
) {
    let took_ms = millis(asked.elapsed());
    match error {
        Some(error) => tracing::info!(
            target: TARGET,
            seen = seen.get(),
            took_ms,
            class = error.class.as_str(),
            "{what}failed"
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
            "{what}ended"
        ),
    }
}
