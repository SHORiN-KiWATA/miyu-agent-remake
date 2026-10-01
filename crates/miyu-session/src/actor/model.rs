//! 请求模型的那几样（施工 3-7 下；施工 4-7 上从 `actor.rs` 挪出来，那边放不下了）：交给端口、叫停、说完了记一行
//! 收场（`28-运行日志.md` 第三节）。辅助请求（施工 3-8 四补的回顾、五补的起标题）也在这里：同一个端口，回报另走一路。
//! 说完了跟着端口的限额（施工 8-9，[`Actor::follow_limits`]）：变了交给内核，模型变了推 `model.changed`。

use std::time::Instant;

use tokio::sync::oneshot;

use miyu_kernel::event::{
    CallError, ChangeWhy, ModelChanged, Purpose, Transient, TransientBody, Usage,
};
use miyu_kernel::id::Seq;
use miyu_kernel::origin::By;
use miyu_kernel::request::{Difference, Request};
use miyu_kernel::session::Input;
use miyu_kernel::time::Timestamp;

use super::{Actor, answer};
use crate::TARGET;
use crate::handle::Pushed;
use crate::lines::{millis, where_};
use crate::port::{Cancel, Report, Reports};
use crate::route::NONE;

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
        let config = self.config.current();
        self.model
            .call(seen, request, config, reports, Cancel::new(cancel));
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
        let config = self.config.current();
        self.model
            .call(upto, request, config, reports, Cancel::new(cancel));
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

    /// 请求 `seen` 说完了：不用再叫停它了；记一行收场（`28-运行日志.md` 第三节）；跟着端口的限额（施工 8-9）。
    pub(super) fn ended(&mut self, seen: Seq, usage: Option<&Usage>, error: Option<&CallError>) {
        self.follow_limits();
        let Some((_, asked)) = self.calls.remove(&seen) else {
            return;
        };
        finished(seen, asked, usage, error, "");
    }

    /// 一次请求说完了（主请求、辅助请求都算）：端口的限额和上一次交给内核的比（施工 8-9，`models.md`「怎么走」第五条第 7
    /// 条）。变了的当场交 `Input::Limits`（不出动作），给头看的限额跟着换；限额里的模型变了、不是 `none` 的（轮换的池总是
    /// `none`），推一条 `model.changed`，`why` 是 `failover`：`by` 是内核，`turn`、`cause` 照内核这时的回合。在送进说完了
    /// 之前做：回合还开着，说完了的用量也照新的模型算锚。
    pub(super) fn follow_limits(&mut self) {
        let limits = self.model.limits();
        if limits == self.handed {
            return;
        }
        let moved = limits.model != self.handed.model && limits.model.endpoint.as_str() != NONE;
        self.session.handle(Input::Limits(limits.clone()));
        let context = self.session.context_limits();
        *self
            .limits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = context;
        let model = limits.model.clone();
        self.handed = limits;
        if !moved {
            return;
        }
        let (turn, cause) = self
            .session
            .turn_cause()
            .map_or((None, None), |(turn, cause)| (Some(turn), cause));
        let at = self.clock.now();
        self.push(Pushed::Transient(Transient {
            at,
            turn,
            by: By::Kernel,
            cause,
            body: TransientBody::ModelChanged(Box::new(ModelChanged {
                reference: self.model.reference(),
                endpoint: Some(model.endpoint),
                model: Some(model.model),
                limits: context,
                why: ChangeWhy::Failover,
            })),
        }));
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
                self.follow_limits();
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
