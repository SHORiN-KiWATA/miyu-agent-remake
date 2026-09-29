//! 看守查被动压缩（`docs/blueprint/compaction.md` 第六条、第十条第 3 条，施工 6-7）：
//!
//! - 主请求报超长、这一轮没结束的：只在一个字都没收到、这一步还没被动压过、没暂停时；后面只跟着发之前照查的事实；
//! - 接着的摘要请求是被动的：`compaction` 是 `overflow`，替代到哪照至少留最后一组算；能重试的错、截短、改走隔离式以后
//!   再来的，还是这次被动压缩、替代到的不变；
//! - 被动压过、重发还超长的，照出错结束，算一次失败，数到了中间夹一条暂停。

use super::*;
use crate::event::{CompactTrigger, ModelCalled, PauseReason};

/// 随机测试的熔断：连续失败几次暂停（`random/compacting.rs`）。
const FAILURES: u32 = 3;

/// 看守记着的被动压缩。
#[derive(Default)]
pub(super) struct Passives {
    /// 主请求刚报了超长，等着先压：下一次摘要请求是被动的，替代到哪照那时算。
    pending: bool,
    /// 被动压缩的摘要请求要再来：下一次摘要请求替代到的是这一条。
    again: Option<Seq>,
    /// 最近发的那次摘要请求是被动压缩的：替代到哪。
    current: Option<Seq>,
    /// 这一步被动压过。
    overflowed: bool,
}

impl Watch {
    /// 发了一次摘要请求：是被动压缩再来的，交回上一次定的替代到哪；头一次被动的交回没有、记下是被动的；自动的也交回
    /// 没有。
    pub(super) fn passive_summary(&mut self, seen: Seq) -> Option<Seq> {
        if let Some(again) = self.passives.again.take() {
            self.seen_paths.insert("被动压缩再来");
            self.passives.current = Some(again);
            self.passives.pending = false;
            return Some(again);
        }
        self.passives.current = std::mem::take(&mut self.passives.pending).then_some(seen);
        None
    }

    /// 最近发的那次摘要请求是被动压缩的。
    pub(super) fn passive_current(&self) -> bool {
        self.passives.current.is_some()
    }

    /// 最近发的那次摘要请求是哪一种压缩：手动压缩那一轮的是手动的（施工 6-8）。
    pub(super) fn summary_trigger(&self) -> CompactTrigger {
        match (self.manual_turn(), self.passives.current) {
            (Some(_), _) => CompactTrigger::Manual,
            (None, Some(_)) => CompactTrigger::Overflow,
            (None, None) => CompactTrigger::Auto,
        }
    }

    /// 摘要请求要再来（能重试的错、截短、改走隔离式）：被动的，下一次还是它。
    pub(super) fn passive_again(&mut self) {
        self.passives.again = self.passives.current;
    }

    /// 主请求说完了：这一步过去了。
    pub(super) fn passive_step_done(&mut self) {
        self.passives.overflowed = false;
    }

    /// 回合结束了：都作废。
    pub(super) fn passive_turn_ended(&mut self) {
        self.passives = Passives::default();
    }

    /// 主请求的 `model.called`：报了超长的照被动压缩的规矩查，交回是不是查过了（查过了的不再照出错再来的规矩查）。
    pub(super) fn main_too_long(
        &mut self,
        called: &ModelCalled,
        events: &[Event],
        k: usize,
    ) -> bool {
        let too_long = called
            .error
            .as_ref()
            .is_some_and(|error| error.class == ErrorClass::ContextTooLong);
        if !too_long {
            return false;
        }
        let seed = self.seed;
        let before = k.checked_sub(1).map(|k| &events[k].body);
        let after = events.get(k + 1).map(|event| &event.body);
        let ends = |body: Option<&Body>| matches!(body, Some(Body::TurnEnded(ended)) if ended.reason == EndReason::Error);
        let replied =
            matches!(before, Some(Body::MessageAssistant(reply)) if reply.seen == called.seen);
        match after {
            Some(Body::TurnEnded(_)) => {
                assert!(ends(after), "种子 {seed}：超长的照出错结束");
                if self.passives.overflowed {
                    self.seen_paths.insert("被动压完重发还超长");
                }
                false
            }
            Some(Body::CompactionPaused(paused)) => {
                self.seen_paths.insert("被动压完重发数到暂停");
                assert_eq!(paused.reason, PauseReason::Failures, "种子 {seed}");
                assert!(
                    self.passives.overflowed,
                    "种子 {seed}：没被动压过却数了失败"
                );
                assert!(
                    self.breaker_failures() + 1 >= FAILURES,
                    "种子 {seed}：没数到 {FAILURES} 次就暂停了"
                );
                assert!(
                    ends(events.get(k + 2).map(|event| &event.body)),
                    "种子 {seed}：暂停后面紧跟着出错的回合结束"
                );
                true
            }
            None | Some(Body::ContextInjected(_)) => {
                self.seen_paths.insert("主请求超长被动压缩");
                assert!(!replied, "种子 {seed}：收到过输出的不该被动压");
                assert!(!self.passives.overflowed, "种子 {seed}：一步被动压了两次");
                assert!(!self.breaker_paused(), "种子 {seed}：暂停着还被动压");
                self.passives.pending = true;
                self.passives.overflowed = true;
                true
            }
            other => panic!("种子 {seed}：主请求超长后面接的不对：{other:?}"),
        }
    }
}
