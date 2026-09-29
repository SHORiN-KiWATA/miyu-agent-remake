//! 看守查熔断（`docs/blueprint/compaction.md` 第十条、第二条第 5 条，施工 6-6 上），照看守自己记下的日志算，不看会话：
//!
//! - 摘要请求的 `model.called` 带 `compaction: auto`，主请求的不带；
//! - 自动压缩出错结束的回合，最近一次压缩以后数到 3，出错的那条和 `turn.ended` 中间夹一条暂停（`failures` 3）；
//!   不到 3、已经暂停着的，不夹；
//! - 每次压缩的 `refills` 照上一次压缩所在的回合算：第 3 个回合以内的是上一次的加一；该是第 3 次的不压，改写暂停
//!   （`too_large`），`entry` 是估得最大的那一条；
//! - 暂停着不发摘要请求；没发出去的 `model.called` 只在暂停着时有，分类 `compaction_paused`，紧跟着出错的回合结束。

use super::*;
use crate::event::{CompactTrigger, CompactionPaused, ContextCompacted, ModelCalled, PauseReason};

/// 随机测试的熔断的数（`random/compacting.rs`）。
const FAILURES: u32 = 3;
const TURNS: usize = 8;
const REFILLS: u32 = 2;

impl Watch {
    /// 还算数的事件里，最近一次压缩那一条以后写下的：撤掉的、撤回的不算。
    fn since_compaction(&self) -> Vec<&Event> {
        let live: Vec<&Event> = self
            .events
            .iter()
            .filter(|event| !self.undo.gone.contains(&event.seq))
            .collect();
        let after = live
            .iter()
            .rposition(|event| matches!(event.body, Body::ContextCompacted(_)));
        live[after.map_or(0, |k| k + 1)..].to_vec()
    }

    /// 暂停着：最近一次压缩以后写下了暂停。
    pub(super) fn breaker_paused(&self) -> bool {
        self.since_compaction()
            .iter()
            .any(|event| matches!(event.body, Body::CompactionPaused(_)))
    }

    /// 最近一次压缩以后，自动压缩出错结束了几轮。
    fn breaker_failures(&self) -> u32 {
        let mut count = 0;
        let mut last: Option<&ModelCalled> = None;
        for event in self.since_compaction() {
            match &event.body {
                Body::TurnStarted(_) => last = None,
                Body::ModelCalled(called) => last = Some(called),
                Body::TurnEnded(ended)
                    if ended.reason == EndReason::Error
                        && last.is_some_and(|called| {
                            called.result == CallResult::Error
                                && called.compaction == Some(CompactTrigger::Auto)
                        }) =>
                {
                    count += 1;
                }
                _ => {}
            }
        }
        count
    }

    /// 这次压缩该写的 `refills`：上一次压缩所在的回合算第 1 个，这一轮是第 3 个以内的，是上一次的加一。
    fn expected_refills(&self) -> Option<u32> {
        let live: Vec<&Event> = self
            .events
            .iter()
            .filter(|event| !self.undo.gone.contains(&event.seq))
            .collect();
        let (previous_turn, previous) = live.iter().rev().find_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some((event.turn?, compacted.refills)),
            _ => None,
        })?;
        let current = self.open_turn();
        let later = live
            .iter()
            .filter(|event| {
                matches!(event.body, Body::TurnStarted(_))
                    && event.seq > previous_turn.started()
                    && event.seq <= current.started()
            })
            .count();
        (later < TURNS).then(|| previous.unwrap_or(0) + 1)
    }

    /// 发了摘要请求：暂停着不该发。
    pub(super) fn breaker_summary_called(&self) {
        assert!(
            !self.breaker_paused(),
            "种子 {}：暂停着还发了摘要请求",
            self.seed
        );
    }

    /// 一条 `model.called` 带不带 `compaction`：摘要请求的带 `auto`，主请求的不带。
    pub(super) fn breaker_called(&mut self, called: &ModelCalled) {
        let expected = self
            .summary_seen(called.seen)
            .then_some(CompactTrigger::Auto);
        assert_eq!(called.compaction, expected, "种子 {}", self.seed);
    }

    /// 没发出去的那一条（施工 6-6 上）：暂停着才有，分类 `compaction_paused`，紧跟着出错的回合结束。交回是不是它。
    pub(super) fn refused(&mut self, called: &ModelCalled, after: Option<&Body>) -> bool {
        let refused = called
            .error
            .as_ref()
            .is_some_and(|error| error.class == ErrorClass::CompactionPaused);
        if !refused {
            return false;
        }
        let seed = self.seed;
        self.seen_paths.insert("暂停着放不下不发");
        assert!(self.breaker_paused(), "种子 {seed}：没暂停却不发");
        assert_eq!(called.endpoint, None, "种子 {seed}：没发出去的没有端点");
        assert_eq!(called.compaction, None);
        assert_eq!(
            called.seen.get(),
            self.last(),
            "种子 {seed}：seen 是最后一条"
        );
        assert!(
            matches!(after, Some(Body::TurnEnded(ended)) if ended.reason == EndReason::Error),
            "种子 {seed}：不发的，紧跟着出错的回合结束"
        );
        true
    }

    /// 摘要请求出了不再来的错：后面要么紧跟着出错的回合结束，要么先夹一条连续失败的暂停。交回后面那一条去掉暂停以后
    /// 的样子，好照原来的规矩查。
    pub(super) fn breaker_failed<'a>(&mut self, events: &'a [Event], k: usize) -> Option<&'a Body> {
        let seed = self.seed;
        let after = events.get(k + 1).map(|event| &event.body);
        let ends = |body: Option<&Body>| matches!(body, Some(Body::TurnEnded(ended)) if ended.reason == EndReason::Error);
        let paused = matches!(after, Some(Body::CompactionPaused(_)));
        let next = events.get(k + 2).map(|event| &event.body);
        // 再来的不算失败：后面既不是暂停，也不是出错的回合结束。
        if !paused && !ends(after) {
            return after;
        }
        let expected = !self.breaker_paused() && self.breaker_failures() + 1 >= FAILURES;
        assert_eq!(
            paused,
            expected,
            "种子 {seed}：第 {} 次自动压缩失败，暂停该不该夹",
            self.breaker_failures() + 1
        );
        if paused {
            assert!(ends(next), "种子 {seed}：暂停后面紧跟着出错的回合结束");
            return next;
        }
        after
    }

    /// 追加了一条暂停：内核写的、在开着的回合里。连续失败的照上面查过了；快满的，是该第 3 次快满的那一次，`entry`
    /// 是估得最大的那一条。
    pub(super) fn pause_appended(&mut self, event: &Event, paused: &CompactionPaused) {
        let seed = self.seed;
        assert_eq!(event.by, By::Kernel);
        assert_eq!(event.turn, Some(self.open_turn()));
        assert!(!self.breaker_paused(), "种子 {seed}：暂停着又写了一次暂停");
        match &paused.reason {
            PauseReason::Failures => {
                self.seen_paths.insert("连续失败暂停");
                assert_eq!(paused.failures, Some(FAILURES), "种子 {seed}");
            }
            PauseReason::TooLarge => {
                self.seen_paths.insert("快满暂停");
                assert_eq!(
                    self.expected_refills(),
                    Some(REFILLS),
                    "种子 {seed}：不是第 {REFILLS} 次快满却暂停了"
                );
                assert_eq!(
                    paused.entry,
                    self.largest(),
                    "种子 {seed}：不是估得最大的那一条"
                );
            }
            PauseReason::Other(other) => panic!("种子 {seed}：内核写了不认识的原因 {other}"),
        }
    }

    /// 追加了一条压缩：`refills` 对得上，不该是第 3 次快满。
    pub(super) fn breaker_compacted(&mut self, compacted: &ContextCompacted) {
        let seed = self.seed;
        let expected = self.expected_refills();
        if expected.is_some() {
            self.seen_paths.insert("压完很快又到线");
        }
        assert_eq!(compacted.refills, expected, "种子 {seed}：refills 不对");
        assert!(
            expected.is_none_or(|refills| refills < REFILLS),
            "种子 {seed}：第 {REFILLS} 次快满还在压"
        );
    }

    /// 有效历史里检查点后面估得最大的那一条，一样大的取早的。
    fn largest(&self) -> Option<Seq> {
        let price = crate::estimate::Flat {
            image: 50,
            file: 50,
        };
        self.effective_events()
            .iter()
            .max_by_key(|event| {
                (
                    crate::estimate::event(event, &price),
                    std::cmp::Reverse(event.seq),
                )
            })
            .map(|event| event.seq)
    }
}
