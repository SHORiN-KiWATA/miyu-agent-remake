//! 看守查压缩（`docs/blueprint/compaction.md` 第三条，施工 6-2 上）：
//!
//! - 摘要请求：替身的组装最后一条写着 `summarize`；照的是有效历史到第 N 条；N 看守自己照规矩算（这一轮要回应的话都在
//!   它后面，比上一次压缩的 `upto` 晚）；不算步数；
//! - 它的 `model.called`：说完了的，后面紧跟着替代到 N 的 `context.compacted`，`trigger` 是 `auto`、`by` 是内核；
//!   取不出摘要的（`bad_summary`），紧跟着出错的回合结束；
//! - 推给头的进度是在路上的那次摘要请求的，字数只增不减；摘要请求不推增量；
//! - 压完以后：请求照检查点以后的事件（`Watch::effective_events`）；撤销撤不到替代掉的回合，原因码 `compacted`。

use super::*;
use crate::event::{CompactTrigger, CompactionProgress, ContextCompacted, ModelCalled};

/// 看守记着的压缩。
#[derive(Default)]
pub(in super::super) struct Compactions {
    /// 交给过执行器的摘要请求，照 `seen`。
    issued: BTreeSet<Seq>,
    /// 在路上的那次摘要请求，和它推过的字数。
    summarizing: Option<(Seq, u64)>,
    /// 最近一次压缩替代到哪。
    pub(super) upto: Option<Seq>,
}

impl Watch {
    /// 这次请求是不是摘要请求：替身的组装最后一条写着 `summarize`。
    pub(super) fn is_summary(request: &Request) -> bool {
        listed_request(request).ends_with("summarize\n")
    }

    /// 这个 `seen` 是不是一次摘要请求的。
    pub(super) fn summary_seen(&self, seen: Seq) -> bool {
        self.compactions.issued.contains(&seen)
    }

    /// 还在有效历史里、请求里该有的事件：造会话那一条算在里面；撤回的、撤掉的，撤回、撤销、恢复那几条本身，
    /// 压缩替代掉的，和检查点本身除外。
    pub(super) fn effective_events(&self) -> Vec<Event> {
        let upto = self.compactions.upto;
        std::iter::once(self.created())
            .chain(self.events.iter().cloned())
            .filter(|event| !self.undo.gone.contains(&event.seq))
            .filter(|event| upto.is_none_or(|upto| event.seq > upto))
            .filter(|event| !matches!(event.body, Body::ContextCompacted(_)))
            .collect()
    }

    /// 发了一次摘要请求：替代到的 N 照规矩；请求是有效历史到 N 的清单加 `summarize`。
    pub(super) fn summary_called(&mut self, seen: Seq, request: &Request) {
        let seed = self.seed;
        let expected = self.expected_upto();
        assert_eq!(Some(seen), expected, "种子 {seed}：摘要请求替代到的不对");
        let effective = self.effective_events();
        let kept: Vec<Event> = effective
            .iter()
            .filter(|event| event.seq <= seen)
            .cloned()
            .collect();
        assert_eq!(
            listed_request(request),
            format!("{}summarize\n", listing(&kept)),
            "种子 {seed}：摘要请求照有效历史到第 {seen} 条"
        );
        let turn = self.open_turn();
        let replied = effective.iter().any(|event| {
            event.turn == Some(turn) && matches!(event.body, Body::MessageAssistant(_))
        });
        self.seen_paths.insert(if replied {
            "回合中途压缩"
        } else {
            "回合开头压缩"
        });
        // 名字是 N，可能和更早的请求一样：回合开头的摘要请求出错再来，N 不变；看到第 N 条的那次主请求以后排着新话
        // 的，N 就是它的名字。照一次新的请求记（内核只认在路上的那一次，过时的回报对不上阶段就不理）。
        if !self.compactions.issued.insert(seen) {
            self.seen_paths.insert("摘要请求再来");
        }
        self.recorded.remove(&seen);
        self.compactions.summarizing = Some((seen, 0));
    }

    /// 看守照规矩算的 N：这一轮要回应的话（以前的回合里的请求、这一轮的回复都没看到过的人的消息；这一轮还没有回复的，
    /// 加上触发它的那一条）里最早的那条当边界，N 是它前面那一条；没有的，是最后一条。不比上一次的 `upto` 晚、前面
    /// 没有能压的，不压。
    fn expected_upto(&mut self) -> Option<Seq> {
        let effective = self.effective_events();
        let turn = self.open_turn();
        let asked_before = effective
            .iter()
            .rev()
            .filter(|event| event.seq < turn.started())
            .find_map(|event| match &event.body {
                Body::ModelCalled(called) => Some(called.seen),
                _ => None,
            });
        let answered = effective.iter().rev().find_map(|event| match &event.body {
            Body::MessageAssistant(reply) => Some(reply.seen),
            _ => None,
        });
        let floor = asked_before.max(answered);
        let unanswered = effective
            .iter()
            .find(|event| {
                matches!(event.body, Body::MessageUser(_)) && floor.is_none_or(|f| event.seq > f)
            })
            .map(|event| event.seq);
        let replied = effective.iter().any(|event| {
            event.turn == Some(turn) && matches!(event.body, Body::MessageAssistant(_))
        });
        let trigger = effective
            .iter()
            .find_map(|event| match &event.body {
                Body::TurnStarted(opened) if event.seq == turn.started() => Some(opened.trigger),
                _ => None,
            })
            .filter(|_| !replied);
        let mut upto = match unanswered.into_iter().chain(trigger).min() {
            Some(boundary) => seq(boundary.get() - 1),
            None => seq(self.last()),
        };
        // 一组不拆：落在回复和它的结果之间的，退到回复前面。
        while let Some(reply) = effective
            .iter()
            .filter_map(|event| match &event.body {
                Body::ToolResult(result) if event.seq > upto => Some(result.call_id.message()),
                _ => None,
            })
            .filter(|reply| *reply <= upto)
            .min()
        {
            self.seen_paths.insert("一组不拆");
            upto = seq(reply.get() - 1);
        }
        if self.compactions.upto.is_some_and(|last| upto <= last) {
            return None;
        }
        effective
            .iter()
            .any(|event| {
                event.seq <= upto
                    && matches!(
                        event.body,
                        Body::MessageUser(_) | Body::MessageAssistant(_) | Body::ToolResult(_)
                    )
            })
            .then_some(upto)
    }

    /// 摘要请求的 `model.called`：说完了的，后面紧跟着替代到它的压缩；取不出摘要的，紧跟着出错的回合结束；别的错
    /// 照重试的规矩。
    pub(super) fn summary_ended(&mut self, called: &ModelCalled, events: &[Event], k: usize) {
        let seed = self.seed;
        let before = k.checked_sub(1).map(|k| &events[k].body);
        let after = events.get(k + 1).map(|event| &event.body);
        assert!(
            !matches!(before, Some(Body::MessageAssistant(reply)) if reply.seen == called.seen),
            "种子 {seed}：摘要请求不写回复"
        );
        match (
            &called.result,
            called.error.as_ref().map(|error| &error.class),
        ) {
            (CallResult::Ok, _) => {
                self.retries.reset_failures();
                assert!(
                    matches!(after, Some(Body::ContextCompacted(compacted)) if compacted.upto == called.seen),
                    "种子 {seed}：摘要请求说完了，后面紧跟着替代到 {} 的压缩",
                    called.seen
                );
            }
            (CallResult::Error, Some(ErrorClass::BadSummary)) => {
                self.seen_paths.insert("取不出摘要");
                assert!(
                    matches!(after, Some(Body::TurnEnded(ended)) if ended.reason == EndReason::Error),
                    "种子 {seed}：取不出摘要的，紧跟着出错的回合结束"
                );
            }
            (CallResult::Interrupted, _) => {
                self.seen_paths.insert("打断了摘要请求");
            }
            _ => {
                self.seen_paths.insert("摘要请求出错");
                self.failed(called.seen, before, after);
            }
        }
        self.compactions.summarizing = None;
    }

    /// 追加了一条压缩：替代到刚说完的那次摘要请求的 N；`trigger` 是 `auto`，`by` 是内核，在开着的回合里。撤销从此
    /// 撤不到它替代掉的回合，也恢复不了更早的撤销。
    pub(super) fn compaction_appended(&mut self, event: &Event, compacted: &ContextCompacted) {
        let seed = self.seed;
        self.seen_paths.insert("压缩了");
        let issued = self.compactions.issued.last().copied();
        assert_eq!(
            Some(compacted.upto),
            issued,
            "种子 {seed}：压缩替代到的是摘要请求的 N"
        );
        assert_eq!(compacted.trigger, Some(CompactTrigger::Auto));
        assert_eq!(event.by, By::Kernel);
        assert_eq!(event.turn, Some(self.open_turn()));
        assert!(!compacted.summary.is_empty(), "种子 {seed}：摘要不是空的");
        self.compactions.upto = Some(compacted.upto);
        self.undo.compacted(compacted.upto);
    }

    /// 推了进度：是在路上的那次摘要请求的，发出去了，字数只增不减。
    pub(super) fn compaction_progress(&mut self, progress: &CompactionProgress) {
        let seed = self.seed;
        self.seen_paths.insert("推了压缩的进度");
        let Some((seen, written)) = self.compactions.summarizing.as_mut() else {
            panic!("种子 {seed}：没有在路上的摘要请求，却推了进度");
        };
        assert_eq!(progress.seen, *seen, "种子 {seed}：进度不是在路上的那次的");
        assert!(
            progress.written > *written,
            "种子 {seed}：进度的字数只增不减"
        );
        assert!((20_000..=80_000).contains(&progress.expected));
        *written = progress.written;
        assert!(
            self.sent.contains(&progress.seen),
            "种子 {seed}：摘要请求还没发出去就推了进度"
        );
    }

    /// 推了增量：不是摘要请求的。
    pub(super) fn not_summarizing(&self, seen: Seq) {
        assert!(
            !self.summary_seen(seen),
            "种子 {}：摘要请求推了增量",
            self.seed
        );
    }

    /// 撤销的那一轮替代掉了：撤不到它，原因码 `compacted`。
    pub(super) fn compacted_turn(&self, turn: TurnId) -> bool {
        self.compactions
            .upto
            .is_some_and(|upto| turn.started() <= upto)
    }
}
