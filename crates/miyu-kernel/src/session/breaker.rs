//! 熔断（`docs/blueprint/compaction.md` 第十条、第二条第 5 条，施工 6-6 上）：自动压缩连续失败、压完很快又到线，就暂停
//! 自动压缩；暂停着，明知放不下的请求不发。
//!
//! 全从有效历史里、最近一次压缩那一条以后写下的事件算，不另记状态：载入、重启以后和不重启一样；撤掉写着暂停的那一轮，
//! 暂停跟着撤掉；有了新的检查点（手动压缩成功），以前的失败和暂停都写在它前面，不再算。只看序号，不看在不在检查点
//! 后面：失败的那几轮她没真看到的话留在尾巴里（`compaction.md` 第三条第 2 条），那几轮的失败照样在压缩那一条前面。
//! 换了模型的（施工 8-10），写在最近一次换模型前面的暂停、失败也不再算：引用照整份日志算（`configure.rs`），撤掉换模型那
//! 一轮，暂停也不回来。

use std::cmp::Reverse;

use super::Session;
use super::action::Action;
use super::compaction::Due;
use super::policy::Pause;
use crate::estimate;
use crate::event::{
    Body, CallError, CallResult, CompactTrigger, CompactionPaused, EndReason, ErrorClass, Event,
    ModelCalled, PauseReason,
};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::request::Request;
use crate::time::Timestamp;

/// 发一次主请求至少给回答留多少 token（施工 6-11 三补，2026-10-10 终端界面的会话转来项目主人的意思：还有空间就不停）：和输出
/// 预留取小的。工具调用那几步的回答通常几百 token，整份输出预留（至多 20000）让停下等的点提前了一大截。
const LEAST_OUTPUT: u64 = 4096;

/// 发主请求之前，熔断这边的结论（[`Session::before_asking`]）。
pub(super) enum Before {
    /// 照发。
    Send,
    /// 先压。
    Compact(Due),
    /// 压完很快又到线，连着到了次数：不压，写暂停（第十条第 5 条）。
    Pause(CompactionPaused),
    /// 暂停着，这一次明知放不下：不发，这一轮出错结束（第二条第 5 条）。
    Refuse(CallError),
}

impl Session {
    /// 发主请求之前：暂停着的，放得下照发、放不下不发；没暂停的，到线要压时先算这一次是不是压完很快又到线，连着到了
    /// 次数的改成暂停。
    pub(super) fn before_asking(&self, request: &Request) -> Before {
        if self.paused() {
            return self
                .cannot_fit(request)
                .map_or(Before::Send, Before::Refuse);
        }
        let Some((upto, used)) = self.compaction_due(request) else {
            return Before::Send;
        };
        let refills = self.refills();
        match self.pause_numbers() {
            Some(pause) if refills.is_some_and(|refills| refills >= pause.refills) => {
                Before::Pause(CompactionPaused {
                    reason: PauseReason::TooLarge,
                    failures: None,
                    entry: self.largest(),
                })
            }
            _ => Before::Compact(Due {
                upto,
                used,
                refills,
                trigger: CompactTrigger::Auto,
                instructions: None,
            }),
        }
    }

    /// 写暂停：回到准备好，这一条落了盘再来，那时已经暂停着了。
    pub(super) fn pause(&mut self, at: Timestamp, paused: CompactionPaused) -> Vec<Action> {
        let cause = self.turn.as_ref().and_then(|turn| turn.cause.clone());
        let event = self.record(at, By::Kernel, cause, Body::CompactionPaused(paused));
        vec![Action::Append(vec![event])]
    }

    /// 暂停着，这一次明知放不下：记一条没发出去的 `model.called`（没有端点、模型、请求字节），这一轮出错结束。
    pub(super) fn refuse(
        &mut self,
        at: Timestamp,
        seen: Seq,
        request: &Request,
        error: CallError,
    ) -> Vec<Action> {
        let cause = self.turn.as_ref().and_then(|turn| turn.cause.clone());
        let called = ModelCalled {
            seen,
            endpoint: None,
            model: None,
            request: None,
            messages: request.messages.len() as u64,
            first_difference: None,
            usage: None,
            cost: None,
            first_token_ms: None,
            duration_ms: None,
            blocks: None,
            result: CallResult::Error,
            error: Some(error),
            compaction: None,
            purpose: None,
        };
        let mut events =
            vec![self.record(at, By::Kernel, cause.clone(), Body::ModelCalled(called))];
        events.extend(self.finish_turn(at, By::Kernel, cause, EndReason::Error));
        vec![Action::Append(events)]
    }

    /// 一次压缩失败、这一轮要出错结束了（第十条第 3、4 条）：连着数到了次数，交回要排在 `turn.ended` 前面的暂停。刚记下
    /// 的那条 `model.called` 还没有 `turn.ended` 跟着，照一次算。`trigger` 是失败的摘要请求是哪一种压缩：手动压缩的失败
    /// 不数（施工 6-8：人就在跟前，看得到）；没有的是被动压完、重发的主请求还超长（施工 6-7），照算。自动的暂停着不发
    /// 摘要请求，走不到这里；暂停着手动压缩失败的，不数，也就不会再写一次暂停。
    pub(super) fn after_failure(
        &mut self,
        at: Timestamp,
        cause: Option<CommandId>,
        trigger: Option<&CompactTrigger>,
    ) -> Option<Event> {
        if trigger.is_some_and(|trigger| !automatic(trigger)) {
            return None;
        }
        let pause = self.pause_numbers()?;
        let failures = self.failures().saturating_add(1);
        (failures >= pause.failures).then(|| {
            let body = Body::CompactionPaused(CompactionPaused {
                reason: PauseReason::Failures,
                failures: Some(failures),
                entry: None,
            });
            self.record(at, By::Kernel, cause, body)
        })
    }

    /// 暂停着：最近一次压缩以后、最近一次换模型以后写下了 `context.compaction_paused`，不认识的原因也算。换模型解除暂停
    /// （施工 8-10，第十条第 6 条）：只看写下的先后。
    pub(super) fn paused(&self) -> bool {
        self.since_compaction().any(|event| {
            matches!(event.body, Body::CompactionPaused(_))
                && self.reference.after_change(event.seq)
        })
    }

    /// 有效历史里，最近一次压缩那一条以后写下的事件；没压缩过的，全部。
    fn since_compaction(&self) -> impl Iterator<Item = &Event> {
        let after = self.history.checkpoint().map(|checkpoint| checkpoint.seq);
        self.history
            .events()
            .iter()
            .filter(move |event| after.is_none_or(|after| event.seq > after))
    }

    /// 熔断的数；快照里没有的不熔断。
    pub(super) fn pause_numbers(&self) -> Option<Pause> {
        self.policy.compaction.as_ref()?.pause
    }

    /// 这份请求的用量、窗口，和至少要给回答留的（输出预留和 [`LEAST_OUTPUT`] 取小的，施工 6-11 三补）。策略里没有压缩、
    /// 没交限额、没有窗口的，没有。
    fn measure(&self, request: &Request) -> Option<(u64, u64, u64)> {
        let compaction = self.policy.compaction.as_ref()?;
        let limits = self.limits.as_ref()?;
        let window = limits.window?;
        let reserve = estimate::reserve(limits.max_output, compaction.reserve_cap);
        let used = self.used_in(&self.history, request)?;
        Some((used, window, reserve.min(LEAST_OUTPUT)))
    }

    /// 这一次请求放得下：用量加上回答的下限不超过窗口（施工 6-11 补：到线不停，放得下就照发；三补：下限是输出预留和
    /// [`LEAST_OUTPUT`] 取小的，真满了才停）。算不了的当放不下。
    pub(super) fn fits(&self, request: &Request) -> bool {
        self.measure(request)
            .is_some_and(|(used, window, least)| used.saturating_add(least) <= window)
    }

    /// 这份请求明知放不下：用量加回答的下限超过窗口。没交限额、没有窗口的，不知道，照发。
    fn cannot_fit(&self, request: &Request) -> Option<CallError> {
        let (used, window, least) = self.measure(request)?;
        (used.saturating_add(least) > window).then(|| CallError {
            class: ErrorClass::CompactionPaused,
            message: format!(
                "the request would not fit: {used} tokens used + {least} reserved for output > window {window}"
            ),
            status: None,
        })
    }

    /// 这一次回答至多多少（施工 6-11 三补，[`Request::output_cap`]）：用量加输出预留超过窗口的，是窗口减用量；放得下整份输出
    /// 预留的、算不了的没有，照模型的。
    pub(super) fn output_cap(&self, request: &Request) -> Option<u64> {
        let compaction = self.policy.compaction.as_ref()?;
        let limits = self.limits.as_ref()?;
        let window = limits.window?;
        let reserve = estimate::reserve(limits.max_output, compaction.reserve_cap);
        let used = self.used_in(&self.history, request)?;
        (used.saturating_add(reserve) > window).then(|| window.saturating_sub(used))
    }

    /// 这一次压缩是不是压完很快又到线（第十条第 5 条）：上一个检查点所在的那一轮算第 1 个回合，这一轮在策略的
    /// `turns` 个以内的，是；交回连着的第几次（上一个检查点的加一）。不是的、没有检查点的、快照里没有熔断的数的，没有。
    pub(super) fn refills(&self) -> Option<u32> {
        let pause = self.pause_numbers()?;
        let checkpoint = self.history.checkpoint()?;
        let Body::ContextCompacted(previous) = &checkpoint.body else {
            return None;
        };
        let since = checkpoint.turn?.started();
        let current = self.turn.as_ref()?.id.started();
        let later = self
            .history
            .events()
            .iter()
            .filter(|event| {
                matches!(event.body, Body::TurnStarted(_))
                    && event.seq > since
                    && event.seq <= current
            })
            .count();
        (u32::try_from(later).ok()? < pause.turns)
            .then(|| previous.refills.unwrap_or(0).saturating_add(1))
    }

    /// 有效历史里检查点后面的，估得最大的那一条，照第一条第 2 条的数法；一样大的取早的。尾巴也算：它也在请求里。
    pub(super) fn largest(&self) -> Option<Seq> {
        let price = self.price()?;
        self.history
            .events()
            .iter()
            .max_by_key(|event| (estimate::event(event, &price), Reverse(event.seq)))
            .map(|event| event.seq)
    }

    /// 最近一次压缩以后，自动压缩失败、出错结束的回合有几个（第十条第 3 条）：`turn.ended` 是 `error`，前面最近的那条
    /// `model.called` 是自动压缩的摘要请求、结果是出错；或者最近一次压缩是这一轮里的被动压缩，重发的主请求还是超长。出错结束的两条路（请求出错、暂停着不发）都紧跟着记一条
    /// `model.called`，最近的那条一定是这一轮的。
    fn failures(&self) -> u32 {
        // 最近一次压缩是被动压缩的，它所在的那一轮：那一轮重发还超长，也算一次失败（施工 6-7）。
        let passive = self
            .history
            .checkpoint()
            .filter(|checkpoint| {
                matches!(&checkpoint.body, Body::ContextCompacted(compacted)
                    if compacted.trigger == Some(CompactTrigger::Overflow))
            })
            .and_then(|checkpoint| checkpoint.turn);
        let mut count = 0u32;
        let mut last: Option<&ModelCalled> = None;
        for event in self.since_compaction() {
            match &event.body {
                Body::ModelCalled(called) => last = Some(called),
                // 换过模型的，换过去以后再失败的才数（施工 8-10）。
                Body::TurnEnded(ended)
                    if ended.reason == EndReason::Error
                        && self.reference.after_change(event.seq)
                        && last.is_some_and(|called| {
                            failed_automatically(called)
                                || (passive.is_some()
                                    && event.turn == passive
                                    && resent_too_long(called))
                        }) =>
                {
                    count = count.saturating_add(1);
                }
                _ => {}
            }
        }
        count
    }
}

/// 自动的压缩：到线的、供应商报超长的。人要的不算。
fn automatic(trigger: &CompactTrigger) -> bool {
    matches!(trigger, CompactTrigger::Auto | CompactTrigger::Overflow)
}

/// 这一次是主请求，报了超长（被动压缩以后重发的那一次，施工 6-7）。
fn resent_too_long(called: &ModelCalled) -> bool {
    called.compaction.is_none()
        && called
            .error
            .as_ref()
            .is_some_and(|error| error.class == ErrorClass::ContextTooLong)
}

/// 这一次是自动压缩的摘要请求，出错了。
fn failed_automatically(called: &ModelCalled) -> bool {
    called.result == CallResult::Error && called.compaction.as_ref().is_some_and(automatic)
}

#[cfg(test)]
mod tests;
