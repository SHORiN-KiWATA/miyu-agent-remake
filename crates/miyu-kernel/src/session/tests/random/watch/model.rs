//! 看守查模型调用的收场和重试（`docs/designs/02-内核.md` 第六节「回复怎么收」第 4 条，施工 3-5 下）：
//!
//! - 请求模型：挂接点跑完了、事件都落了盘、上一步的调用都有了结果，请求照全部历史，一个回合不超过上限（施工 3-8 四补从
//!   `watch.rs` 挪来）；
//! - 一条 `model.called`：交给过执行器、只记一次；说完了的前面是它的回复；
//! - 块的起止：写了回复的才有，和回复的块一块一项，止不早于起（施工 2-3 补）；
//! - 出错的：要么紧跟着出错的回合结束，要么再来。再来的，前面是半截回复的（带 `interrupted`，一个
//!   工具调用都没有），后面紧跟着被打断的那一句；什么都没收到的，这一批到它为止；
//! - 再来的交出到点叫醒，推一条 `status`；叫醒以前不请求；叫醒以后的那一次是重试，不算步数；
//! - 一步里连着再来不超过 5 次；
//! - 再来的只有能再来的分类，和端口说换了端点的（不管分类）；换了端点、全在冷却的，没到 5 次、要等的不超过 2 分钟就一定
//!   再来；推的 `status` 带的 `failover` 和端口说的一样，换了端点又没说等多久的等 0 毫秒（施工 8-9）；
//! - 金额：说完了的 `model.called` 带的就是最近一次喂进去的说完了带的，原样；被打断的没有（施工 8-15）。

use std::collections::BTreeMap;

use super::*;
use crate::event::{Cost, ModelCalled, Prices, Real, Status};

/// 供应商说的、要等多久的上限（`retry.rs` 的 `WAIT_LIMIT_MS`）：看守自己记一份，不借内核的。
const WAIT_LIMIT_MS: u64 = 120_000;

/// 端口说完一次请求时说的（施工 8-9）：要等多久、换没换端点。
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Said {
    wait_ms: Option<u64>,
    failover: bool,
}

/// 重试走到哪了。
#[derive(Debug, Default)]
pub(super) struct Retries {
    /// 记了出错、该交出到点叫醒的那次请求。
    pub(super) expecting: Option<Seq>,
    /// 交出了到点叫醒、还在等的那次。
    pub(super) waiting: Option<Seq>,
    /// 到点了：下一次请求是重试。
    woken: bool,
    /// 这一步连着再来了几次。
    failures: u32,
    /// 每次请求最近一次喂进去的说完了，端口说的（施工 8-9）。记了 `model.called` 就拿走：摘要请求再来时名字不变。
    said: BTreeMap<Seq, Said>,
    /// 刚记了出错、要再来的那一次端口说的：推的 `status` 照它查。
    expecting_said: Said,
    /// 每次请求最近一次喂进去的说完了带的金额（施工 8-15）：记了 `model.called` 就拿走，照它查。
    costs: BTreeMap<Seq, Option<Cost>>,
}

/// 喂进去的金额，照请求的序号造（施工 8-15）：不同的请求金额不同，记混了查得出来。
pub(in crate::session::tests::random) fn priced(seen: Seq) -> Cost {
    Cost {
        amount: Real::new(seen.get() as f64 / 1000.0),
        currency: "USD".to_string(),
        price: Prices::default(),
        multiplier: Real::new(1.0),
        source: "local".to_string(),
        above: None,
    }
}

impl Watch {
    /// 请求模型：挂接点跑完了、事件都落了盘、上一步的调用都有了结果；请求照全部历史；
    /// 一个回合的请求不超过上限。摘要请求照压缩的规矩查（`watch/compaction.rs`），不算步数。
    pub(super) fn called(&mut self, seen: Seq, request: &Request) {
        let seed = self.seed;
        // 交出去以前喂进去的说完了不算（内核不收不在路上的）：只查交出去以后的（施工 8-15）。
        self.retries.costs.remove(&seen);
        let turn = self.open_turn();
        assert!(
            self.done.contains(&turn),
            "种子 {seed}：挂接点还没跑完就请求"
        );
        assert!(
            self.events
                .iter()
                .all(|event| self.pushed.contains(&event.seq)),
            "种子 {seed}：还有事件没落盘就请求"
        );
        assert!(
            self.calls_in(turn)
                .all(|call| self.resulted.contains(&call)),
            "种子 {seed}：上一步还有调用没结果就请求"
        );
        self.request_from_log(seen, request);
        self.sight_called(request, Watch::is_summary(request));
        match Watch::is_summary(request) {
            true => self.undo_summary(),
            false => self.undo_request(seen),
        }
        self.manual_request(request);
        self.issued.insert(seen);
        self.sent.remove(&seen);
        self.asking = Some(seen);
        self.next_block = 0;
        self.open_block = None;
        if Watch::is_summary(request) {
            self.retry_summary();
            self.summary_called(seen, request);
            return;
        }
        self.main_request_sent();
        assert_eq!(seen.get(), self.last(), "种子 {seed}：seen 是最后一条");
        // 请求照的是全部历史，撤回的、撤掉的，撤回、撤销、恢复那几条本身，和压缩替代掉的除外。
        assert_eq!(
            listed_request(request),
            listing(&self.effective_events()),
            "种子 {seed}：请求照全部历史，撤回的、撤掉的、压缩掉的除外"
        );
        let retry = self.retry_request();
        let count = self.requests.entry(turn).or_default();
        if !retry {
            *count += 1;
        }
        assert!(
            *count <= STEP_LIMIT,
            "种子 {seed}：回合 {turn} 请求超过了上限"
        );
        if *count > 1 {
            self.seen_paths.insert("一步接一步");
        }
    }

    /// 一条 `model.called`：交给过执行器、只记一次；说完了的前面是它的回复；出错的，要么紧跟着出错
    /// 的回合结束，要么再来。
    pub(super) fn model_called(&mut self, called: &ModelCalled, events: &[Event], k: usize) {
        let seed = self.seed;
        // 回顾的请求另查（施工 3-8 四补，`watch/recap.rs`）：它不是这一轮的请求。
        if called.aside() {
            return self.recap_called(called, events, k);
        }
        // 暂停着明知放不下、没发出去的那一条：没交给过执行器（施工 6-6 上，`watch/breaker.rs`）。
        if self.refused(called, events.get(k + 1).map(|event| &event.body)) {
            return;
        }
        self.breaker_called(called);
        self.cost_recorded(called);
        assert!(
            self.issued.contains(&called.seen),
            "种子 {seed}：没交给执行器的请求 {} 记了一条",
            called.seen
        );
        assert!(
            self.recorded.insert(called.seen),
            "种子 {seed}：请求 {} 记了两条",
            called.seen
        );
        let before = k.checked_sub(1).map(|k| &events[k].body);
        let after = events.get(k + 1).map(|event| &event.body);
        self.block_spans(called, before);
        let interrupted_later = events[k..].iter().any(|event| {
            matches!(&event.body, Body::TurnEnded(ended)
                if matches!(ended.reason, EndReason::Interrupted | EndReason::Restarted))
        });
        if called.result == CallResult::Interrupted {
            assert!(
                interrupted_later,
                "种子 {seed}：被打断的请求，这一批里接着是被打断（或者被重启打断）的回合结束"
            );
        }
        if self.summary_seen(called.seen) {
            self.summary_ended(called, events, k);
        } else if !self.main_too_long(called, events, k) {
            self.reply_ended(called, before, after);
        }
        self.undo_called(called);
        if self.asking == Some(called.seen) {
            self.asking = None;
        }
    }

    /// 金额（施工 8-15）：被打断的没有；别的是最近一次喂进去的说完了带的，原样。
    fn cost_recorded(&mut self, called: &ModelCalled) {
        let seed = self.seed;
        let fed = self.retries.costs.remove(&called.seen);
        if called.result == CallResult::Interrupted {
            assert_eq!(called.cost, None, "种子 {seed}：被打断的请求不该有金额");
            return;
        }
        if let Some(fed) = fed {
            assert_eq!(
                called.cost.as_deref(),
                fed.as_ref(),
                "种子 {seed}：请求 {} 记的金额不是喂进去的",
                called.seen
            );
            if fed.is_some() {
                self.seen_paths.insert("记下金额");
            }
        }
    }

    /// 主请求的 `model.called`：说完了的前面是它的回复；出错的照重试的规矩。
    fn reply_ended(&mut self, called: &ModelCalled, before: Option<&Body>, after: Option<&Body>) {
        let seed = self.seed;
        match called.result {
            CallResult::Ok => {
                self.seen_paths.insert("说完了");
                self.retries.failures = 0;
                self.passive_step_done();
                assert!(
                    matches!(before, Some(Body::MessageAssistant(reply)) if reply.seen == called.seen),
                    "种子 {seed}：说完了的，前面是它的回复"
                );
            }
            CallResult::Interrupted => {
                self.seen_paths.insert("打断了请求");
            }
            _ => {
                self.seen_paths.insert("出错了");
                self.failed(called, before, after);
            }
        }
    }

    /// 块的起止（施工 2-3 补）：写了回复的才有，和回复的块一块一项，每一块的止不早于起；没写回复的没有。
    fn block_spans(&self, called: &ModelCalled, before: Option<&Body>) {
        let seed = self.seed;
        let reply = match before {
            Some(Body::MessageAssistant(reply)) if reply.seen == called.seen => {
                Some(reply.blocks.len())
            }
            _ => None,
        };
        let spans = called.blocks.as_ref();
        assert_eq!(
            spans.map(Vec::len),
            reply,
            "种子 {seed}：块的起止和回复的块一块一项，没写回复的没有"
        );
        assert!(
            spans
                .into_iter()
                .flatten()
                .all(|span| span.start_ms <= span.end_ms),
            "种子 {seed}：块的止早于起"
        );
    }

    /// 出错的收场：紧跟着出错的回合结束的，是不再来了；不是的，是再来。再来的只有能再来的分类和换了端点的；换了端点、
    /// 全在冷却的，没到上限、要等的不超过 2 分钟就一定再来（施工 8-9）。
    pub(super) fn failed(
        &mut self,
        called: &ModelCalled,
        before: Option<&Body>,
        after: Option<&Body>,
    ) {
        let seed = self.seed;
        let seen = called.seen;
        let said = self.retries.said.remove(&seen).unwrap_or_default();
        let class = called.error.as_ref().map(|error| &error.class);
        let cooling = class == Some(&ErrorClass::Cooling);
        let waits_ok = said.wait_ms.is_none_or(|wait| wait <= WAIT_LIMIT_MS);
        let partial = match before {
            Some(Body::MessageAssistant(reply)) if reply.seen == seen => {
                assert!(
                    reply.interrupted,
                    "种子 {seed}：出错断了的半截回复带 interrupted"
                );
                assert!(
                    reply
                        .blocks
                        .iter()
                        .all(|block| !matches!(block, Block::ToolCall(_))),
                    "种子 {seed}：出错断了的半截回复里不留工具调用"
                );
                true
            }
            _ => false,
        };
        if matches!(after, Some(Body::TurnEnded(ended)) if ended.reason == EndReason::Error) {
            assert!(
                !((said.failover || cooling) && waits_ok && self.retries.failures < 5),
                "种子 {seed}：换了端点、全在冷却的没到上限也不再来（{class:?}，{said:?}）"
            );
            return;
        }
        assert!(
            said.failover || class.is_some_and(can_retry),
            "种子 {seed}：{class:?} 没换端点也再来了"
        );
        if said.failover {
            self.seen_paths.insert("换了端点再来");
        }
        if cooling {
            self.seen_paths.insert("全在冷却等着再来");
        }
        if partial {
            self.seen_paths.insert("带着半截再来");
            assert!(
                matches!(after, Some(Body::ContextInjected(fact)) if fact.kind.as_str() == "reply_cut"),
                "种子 {seed}：半截回复后面紧跟着被打断的那一句"
            );
        } else {
            assert!(
                after.is_none(),
                "种子 {seed}：什么都没收到的再来，这一批到 model.called 为止"
            );
        }
        self.retries.failures += 1;
        assert!(
            self.retries.failures <= 5,
            "种子 {seed}：一步里连着再来了 {} 次",
            self.retries.failures
        );
        self.retries.expecting = Some(seen);
        self.retries.expecting_said = said;
    }

    /// 推了 `status`：是刚记了出错、要再来的那一次。
    pub(super) fn retry_status(&mut self, status: &Status) {
        let seed = self.seed;
        self.seen_paths.insert("推了重试的状态");
        assert_eq!(
            Some(status.seen),
            self.retries.expecting,
            "种子 {seed}：推的重试状态不是刚出错的那一次"
        );
        assert_eq!(status.retry.attempt, self.retries.failures);
        let said = self.retries.expecting_said;
        assert_eq!(
            status.retry.failover, said.failover,
            "种子 {seed}：状态的 failover 照端口说的"
        );
        if said.failover && said.wait_ms.is_none() {
            assert_eq!(
                status.retry.wait_ms, 0,
                "种子 {seed}：换了端点没说等多久的当场来"
            );
        }
    }

    /// 交出到点叫醒：是刚记了出错、要再来的那一次。
    pub(super) fn retry_wake(&mut self, seen: Seq) {
        let seed = self.seed;
        self.seen_paths.insert("出错了再来");
        assert_eq!(
            self.retries.expecting.take(),
            Some(seen),
            "种子 {seed}：到点叫醒的不是刚出错的那一次"
        );
        self.retries.waiting = Some(seen);
    }

    /// 喂进「到点了」：对得上在等的那一次，下一次请求就是重试。喂进说完了的：记下端口说的（施工 8-9）。
    pub(super) fn retry_fed(&mut self, input: &Input) {
        // 内核只收在路上的那一次的：对不上的不理，也不记。
        if let Input::ModelEnded { seen, cost, .. } = input {
            self.retries.costs.insert(*seen, cost.clone());
        }
        if let Input::ModelEnded {
            seen,
            wait_ms,
            failover,
            ..
        } = input
            && self.asking == Some(*seen)
        {
            let said = Said {
                wait_ms: *wait_ms,
                failover: *failover,
            };
            self.retries.said.insert(*seen, said);
        }
        if let Input::Woke { seen, .. } = input
            && self.retries.waiting == Some(*seen)
        {
            self.seen_paths.insert("到点了接着请求");
            self.retries.waiting = None;
            // 再来的是摘要请求的，它后面那一次主请求照常算一步；主请求的重试标记留着，隔着一次压缩也算。
            self.retries.woken |= !self.summary_seen(*seen);
        }
    }

    /// 请求模型：等着重试的时候不请求。交回这一次是不是重试：重试的不算步数。
    pub(super) fn retry_request(&mut self) -> bool {
        let seed = self.seed;
        assert!(
            self.retries.waiting.is_none() && self.retries.expecting.is_none(),
            "种子 {seed}：还在等着重试就请求了"
        );
        std::mem::take(&mut self.retries.woken)
    }

    /// 发摘要请求：等着重试的时候不请求；它不是主请求的重试，重试标记留给后面那一次主请求。
    pub(super) fn retry_summary(&mut self) {
        assert!(
            self.retries.waiting.is_none() && self.retries.expecting.is_none(),
            "种子 {}：还在等着重试就请求了",
            self.seed
        );
    }

    /// 回合结束了：在等的、连着的次数都清掉。
    pub(super) fn retry_ended(&mut self) {
        self.retries = Retries::default();
    }
}

/// 不换端点也能再来的分类：看守自己列一份，不借内核的（`kernel/session.md`「出错再来」第 1 条，施工 8-9 加 `cooling`）。
fn can_retry(class: &ErrorClass) -> bool {
    matches!(
        class,
        ErrorClass::Retryable
            | ErrorClass::RateLimited
            | ErrorClass::BadStream
            | ErrorClass::EmptyReply
            | ErrorClass::Cooling
    )
}
