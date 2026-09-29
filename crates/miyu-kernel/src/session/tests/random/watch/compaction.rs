//! 看守查压缩（`docs/blueprint/compaction.md` 第三条，施工 6-2 上）：
//!
//! - 摘要请求：替身的组装最后一条写着 `summarize`；照的是有效历史到第 N 条；N 看守自己照规矩算（这一轮要回应的话都在
//!   它后面，比上一次压缩的 `upto` 晚）；不算步数；
//! - 它的 `model.called`：说完了的，后面紧跟着替代到 N 的 `context.compacted`，`trigger` 是 `auto`、`by` 是内核；
//!   取不出摘要的（`bad_summary`），紧跟着出错的回合结束；
//! - 推给头的进度是在路上的那次摘要请求的，字数只增不减；摘要请求不推增量；
//! - 压完以后：请求照还算数的检查点以后的事件（`Watch::effective_events`）；撤销能撤掉压缩（施工 6-9，`watch/undo.rs`）。

use super::*;
use crate::event::{CompactionProgress, ContextCompacted, ModelCalled};
use crate::id::ContentHash;

/// 看守记着的压缩。
#[derive(Default)]
pub(in super::super) struct Compactions {
    /// 交过的模型限额；载入以后会话不记得，看守也清掉（施工 6-2 下：尾巴的预算照它算）。
    pub(super) limits: Option<Limits>,
    /// 交给过执行器的摘要请求，照 `seen`。
    issued: BTreeSet<Seq>,
    /// 在路上的那次摘要请求，和它推过的字数。
    pub(super) summarizing: Option<(Seq, u64)>,
    /// 最近发的那一次摘要请求：说完了、写压缩时照它对。带了尾巴，没成的那一次的 N 可以比后来的大，不能拿交过的里最大的。
    latest: Option<Seq>,
    /// 还算数的几次压缩，照先后（施工 6-9）：撤掉的记在那一次撤销上，恢复了放回来。
    pub(super) live: Vec<Live>,
    /// 压完了，还没发这一步的主请求：一步至多压一次（施工 6-2 下）。
    pending: bool,
    /// 刚交出的重读，还没跟上它那次摘要请求（施工 6-5）。
    pub(super) reread: Option<Seq>,
    /// 压后重建（施工 6-5，`watch/rebuild.rs`）。
    pub(super) rebuild: super::rebuild::Rebuilds,
}

/// 一次还算数的压缩：在哪一轮、替代到哪、重读的文件的 blob。
#[derive(Clone)]
pub(super) struct Live {
    pub(super) turn: TurnId,
    pub(super) upto: Seq,
    pub(super) blobs: Vec<ContentHash>,
}

impl Compactions {
    /// 还算数的最近一次压缩替代到哪。
    pub(super) fn upto(&self) -> Option<Seq> {
        self.live.last().map(|live| live.upto)
    }
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
        let upto = self.compactions.upto();
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
        self.breaker_summary_called();
        assert!(
            !self.compactions.pending,
            "种子 {seed}：压完了还没发主请求，又压了一次"
        );
        // 被动压缩的（施工 6-7）：头一次照至少留最后一组算；再来的照上一次定的。
        let expected = match self.passive_summary(seen) {
            Some(again) => Some(again),
            None => self.expected_upto(self.passive_current()),
        };
        assert_eq!(Some(seen), expected, "种子 {seed}：摘要请求替代到的不对");
        let cut = self.summary_cut(seen, request);
        let effective = self.effective_events();
        let kept: Vec<Event> = effective
            .iter()
            .filter(|event| event.seq <= seen && cut.is_none_or(|cut| event.seq > cut))
            .cloned()
            .collect();
        let head = cut.map_or(String::new(), |cut| format!("truncated after {cut}\n"));
        assert_eq!(
            listed_request(request),
            format!("{head}{}summarize\n", listing(&kept)),
            "种子 {seed}：摘要请求照有效历史到第 {seen} 条，截过的从截到的以后"
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
        self.compactions.latest = Some(seen);
    }

    /// 看守照规矩算的 N：这一轮要回应的话（以前的回合里的请求、这一轮的回复都没看到过的人的消息；这一轮还没有回复的，
    /// 加上触发它的那一条）里最早的那条当边界，N 是它前面那一条；没有的，是最后一条。不比上一次的 `upto` 晚、前面
    /// 没有能压的，不压。
    fn expected_upto(&mut self, keep_last: bool) -> Option<Seq> {
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
        let ordered = self.rendered_order();
        // 留尾巴：尾巴只会让 N 往前挪。
        if let Some(tail) = self.expected_tail(&ordered, keep_last)
            && tail < upto
        {
            self.seen_paths.insert("留了尾巴");
            upto = tail;
        }
        // 一组不拆：落在回复和它的结果之间的，退到回复前面；切出来的前一段要是投影的开头一段。照到不动为止。
        loop {
            let before = upto;
            if let Some(reply) = ordered
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
            if !cut_at(&ordered, upto) {
                self.seen_paths.insert("切不开往前退");
                let first = ordered.iter().position(|event| event.seq > upto).unwrap();
                let lowest = ordered[first..]
                    .iter()
                    .map(|event| event.seq)
                    .min()
                    .unwrap();
                upto = seq(lowest.get() - 1);
            }
            if upto == before {
                break;
            }
        }
        if self.compactions.upto().is_some_and(|last| upto <= last) {
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

    /// 照看守记下的日志重建有效历史，交回投影的先后（`History::ordered`）：撤掉过压缩的日志一条条收不出来，照载入的办法
    /// 从还算数的最近一次压缩替代到的下一条起重建（施工 6-9）。
    fn rendered_order(&self) -> Vec<Event> {
        let from = self.compactions.upto().map_or(Seq::FIRST, Seq::next);
        let mut history = History::whole();
        for event in std::iter::once(self.created()).chain(self.events.iter().cloned()) {
            if event.seq >= from {
                history.append(event);
            }
        }
        history.settle();
        history.ordered().into_iter().cloned().collect()
    }

    /// 看守照规矩算的尾巴：预算是 min(30, 压缩线的四分之一)（随机测试的策略）。照投影的先后，一组从人的消息或者回复
    /// 开始；从最新的一组往回，尾巴（这一组起到最后的全部事件）还在预算以内、这里切得开的，N 可以是这一组前面那一条，
    /// 取最早的；加上就超了的停下。
    fn expected_tail(&self, ordered: &[Event], keep_last: bool) -> Option<Seq> {
        let limits = self.compactions.limits.as_ref()?;
        let line = crate::estimate::line(limits.window, limits.max_output, 10, 10);
        // 被动压缩（施工 6-7）没有线也压，预算照策略的 30；最后一组比预算大也留。
        let budget = match line {
            Some(line) => (line / 4).min(30),
            None if keep_last => 30,
            None => return None,
        };
        let price = crate::estimate::Flat {
            image: 50,
            file: 50,
        };
        let mut tail = None;
        for (index, event) in ordered.iter().enumerate().rev() {
            if !matches!(event.body, Body::MessageUser(_) | Body::MessageAssistant(_)) {
                continue;
            }
            let size: u64 = ordered[index..]
                .iter()
                .map(|event| crate::estimate::event(event, &price))
                .sum();
            let lowest = ordered[index..]
                .iter()
                .map(|event| event.seq)
                .min()
                .unwrap();
            // 这一组前面切得开：前面的序号都比从这一组起的小。
            let cuttable = ordered[..index].iter().all(|event| event.seq < lowest);
            if size > budget {
                if keep_last && tail.is_none() && cuttable {
                    tail = Some(seq(lowest.get() - 1));
                }
                break;
            }
            if cuttable {
                tail = Some(seq(lowest.get() - 1));
            }
        }
        tail
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
            // 说完了也不清零连着出错的次数：和这一步的主请求合用一个计数（施工 6-2 下）。
            (CallResult::Ok, _) => {
                assert!(
                    matches!(after, Some(Body::ContextCompacted(compacted)) if compacted.upto == called.seen),
                    "种子 {seed}：摘要请求说完了，后面紧跟着替代到 {} 的压缩",
                    called.seen
                );
            }
            (CallResult::Error, Some(ErrorClass::BadSummary))
                if called
                    .error
                    .as_ref()
                    .is_some_and(|error| error.message.ends_with("trying again without tools")) =>
            {
                // 调了工具、改走隔离式（施工 6-6 下）：这一轮不结束，后面只跟着发之前照查的事实。
                assert!(
                    after.is_none_or(|body| matches!(body, Body::ContextInjected(_))),
                    "种子 {seed}：改走隔离式的，这一轮不结束"
                );
                self.summary_isolating(called.seen);
                self.passive_again();
            }
            (CallResult::Error, Some(ErrorClass::BadSummary)) => {
                self.seen_paths.insert("取不出摘要");
                // 连续失败到了次数的，中间夹一条暂停（施工 6-6 上）。
                let after = self.breaker_failed(events, k);
                assert!(
                    matches!(after, Some(Body::TurnEnded(ended)) if ended.reason == EndReason::Error),
                    "种子 {seed}：取不出摘要的，紧跟着出错的回合结束"
                );
            }
            (CallResult::Interrupted, _) => {
                self.seen_paths.insert("打断了摘要请求");
            }
            _ => {
                let after = self.breaker_failed(events, k);
                // 报超长、这一轮没结束的：截短了等着再发，不交到点叫醒；后面只跟着发之前照查的事实（施工 6-6 中）。
                let too_long = called
                    .error
                    .as_ref()
                    .is_some_and(|error| error.class == ErrorClass::ContextTooLong);
                if too_long && after.is_none_or(|body| matches!(body, Body::ContextInjected(_))) {
                    self.summary_too_long(called.seen);
                    self.passive_again();
                } else {
                    if too_long {
                        self.seen_paths.insert("截不动算失败");
                    }
                    self.seen_paths.insert("摘要请求出错");
                    self.failed(called.seen, before, after);
                    // 再来的（施工 6-7）：被动压缩的下一次还是它。
                    if self.retries.expecting == Some(called.seen) {
                        self.passive_again();
                    }
                }
            }
        }
        self.compactions.summarizing = None;
    }

    /// 追加了一条压缩：替代到刚说完的那次摘要请求的 N；`trigger` 是 `auto`，`by` 是内核，在开着的回合里。它是还算数
    /// 的最近一次；更早的撤销恢复不了。
    pub(super) fn compaction_appended(&mut self, event: &Event, compacted: &ContextCompacted) {
        self.rebuild_checked(compacted);
        self.breaker_compacted(compacted);
        self.shorten_compacted(compacted);
        let seed = self.seed;
        self.seen_paths.insert("压缩了");
        let issued = self.compactions.latest;
        assert_eq!(
            Some(compacted.upto),
            issued,
            "种子 {seed}：压缩替代到的是摘要请求的 N"
        );
        assert_eq!(
            compacted.trigger,
            Some(self.summary_trigger()),
            "种子 {seed}"
        );
        assert_eq!(event.by, By::Kernel);
        assert_eq!(event.turn, Some(self.open_turn()));
        assert!(!compacted.summary.is_empty(), "种子 {seed}：摘要不是空的");
        self.compactions.live.push(Live {
            turn: self.open_turn(),
            upto: compacted.upto,
            blobs: compacted
                .restored
                .iter()
                .map(|file| file.blob.clone())
                .collect(),
        });
        self.compactions.pending = true;
        self.undo.compacted();
    }

    /// 推了进度：是在路上的那次摘要请求的，发出去了，字数只增不减。
    pub(super) fn compaction_progress(&mut self, progress: &CompactionProgress) {
        let seed = self.seed;
        self.seen_paths.insert("推了压缩的进度");
        let Some((seen, written)) = self.compactions.summarizing.as_mut() else {
            panic!("种子 {seed}：没有在路上的摘要请求，却推了进度");
        };
        assert_eq!(progress.seen, *seen, "种子 {seed}：进度不是在路上的那次的");
        // 发出去时先推一条 0 字的（施工 6-3 下），之后每一条都比上一条多。
        if progress.written == 0 {
            self.seen_paths.insert("推了还没写字的进度");
        } else {
            assert!(
                progress.written > *written,
                "种子 {seed}：进度的字数只增不减"
            );
        }
        assert!((20_000..=80_000).contains(&progress.expected));
        *written = progress.written;
        assert!(
            self.sent.contains(&progress.seen),
            "种子 {seed}：摘要请求还没发出去就推了进度"
        );
    }

    /// 载入了：会话不记得交过的限额。
    pub(super) fn forget_limits(&mut self) {
        self.compactions.limits = None;
    }

    /// 发了主请求，或者这一轮结束了：下一步又能压了。
    pub(super) fn main_request_sent(&mut self) {
        self.compactions.pending = false;
    }

    /// 推了压好了（施工 6-3 下）：紧跟在压缩后面、这一步的主请求以前，替代到的对得上。
    pub(super) fn compaction_done(&mut self, done: &crate::event::CompactionDone) {
        let seed = self.seed;
        self.seen_paths.insert("推了压好了");
        assert!(
            self.compactions.pending,
            "种子 {seed}：没压、或者已经发了主请求，却推了压好了"
        );
        assert_eq!(Some(done.seen), self.compactions.upto(), "种子 {seed}");
    }

    /// 在路上的主请求（施工 6-7：另一串随机数照它报超长）。
    pub(in super::super) fn asking_main(&self) -> Option<Seq> {
        self.asking.filter(|seen| !self.summary_seen(*seen))
    }

    /// 在路上的那次摘要请求（施工 6-6 中：另一串随机数照它报超长）。
    pub(in super::super) fn summarizing(&self) -> Option<Seq> {
        self.compactions.summarizing.map(|(seen, _)| seen)
    }

    /// 推了增量：不是摘要请求的。
    pub(super) fn not_summarizing(&self, seen: Seq) {
        assert!(
            !self.summary_seen(seen),
            "种子 {}：摘要请求推了增量",
            self.seed
        );
    }
}

/// 切在 `upto` 后面，前一段是不是投影的开头一段：序号不超过它的，都排在超过它的前面。
fn cut_at(ordered: &[Event], upto: Seq) -> bool {
    let first = ordered.iter().position(|event| event.seq > upto);
    first.is_none_or(|first| ordered[first..].iter().all(|event| event.seq > upto))
}
