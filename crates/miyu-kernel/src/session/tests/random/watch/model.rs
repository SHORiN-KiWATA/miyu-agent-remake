//! 看守查模型调用的收场和重试（`docs/designs/02-内核.md` 第六节「回复怎么收」第 4 条，施工 3-5 下）：
//!
//! - 一条 `model.called`：交给过执行器、只记一次；说完了的前面是它的回复；
//! - 出错的：要么紧跟着出错的回合结束，要么再来。再来的，前面是半截回复的（带 `interrupted`，一个
//!   工具调用都没有），后面紧跟着被打断的那一句；什么都没收到的，这一批到它为止；
//! - 再来的交出到点叫醒，推一条 `status`；叫醒以前不请求；叫醒以后的那一次是重试，不算步数；
//! - 一步里连着再来不超过 5 次。

use super::*;
use crate::event::{ModelCalled, Status};

/// 重试走到哪了。
#[derive(Debug, Default)]
pub(super) struct Retries {
    /// 记了出错、该交出到点叫醒的那次请求。
    expecting: Option<Seq>,
    /// 交出了到点叫醒、还在等的那次。
    pub(super) waiting: Option<Seq>,
    /// 到点了：下一次请求是重试。
    woken: bool,
    /// 这一步连着再来了几次。
    failures: u32,
}

impl Watch {
    /// 一条 `model.called`：交给过执行器、只记一次；说完了的前面是它的回复；出错的，要么紧跟着出错
    /// 的回合结束，要么再来。
    pub(super) fn model_called(&mut self, called: &ModelCalled, events: &[Event], k: usize) {
        let seed = self.seed;
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
        let interrupted_later = events[k..].iter().any(|event| {
            matches!(&event.body, Body::TurnEnded(ended)
                if matches!(ended.reason, EndReason::Interrupted | EndReason::Restarted))
        });
        match called.result {
            CallResult::Ok => {
                self.seen_paths.insert("说完了");
                self.retries.failures = 0;
                assert!(
                    matches!(before, Some(Body::MessageAssistant(reply)) if reply.seen == called.seen),
                    "种子 {seed}：说完了的，前面是它的回复"
                );
            }
            CallResult::Interrupted => {
                self.seen_paths.insert("打断了请求");
                assert!(
                    interrupted_later,
                    "种子 {seed}：被打断的请求，这一批里接着是被打断（或者被重启打断）的回合结束"
                );
            }
            _ => {
                self.seen_paths.insert("出错了");
                self.failed(called.seen, before, after);
            }
        }
        self.undo_called(called);
        if self.asking == Some(called.seen) {
            self.asking = None;
        }
    }

    /// 出错的收场：紧跟着出错的回合结束的，是不再来了；不是的，是再来。
    fn failed(&mut self, seen: Seq, before: Option<&Body>, after: Option<&Body>) {
        let seed = self.seed;
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
            return;
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

    /// 喂进「到点了」：对得上在等的那一次，下一次请求就是重试。
    pub(super) fn retry_fed(&mut self, input: &Input) {
        if let Input::Woke { seen, .. } = input
            && self.retries.waiting == Some(*seen)
        {
            self.seen_paths.insert("到点了接着请求");
            self.retries.waiting = None;
            self.retries.woken = true;
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

    /// 回合结束了：在等的、连着的次数都清掉。
    pub(super) fn retry_ended(&mut self) {
        self.retries = Retries::default();
    }
}
