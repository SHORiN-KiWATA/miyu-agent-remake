//! 被动压缩（`docs/blueprint/compaction.md` 第六条，施工 6-7）：主请求被供应商报上下文超长，一个字都没收到的，先压一次
//! 再重发一次。本地估算偏小（例如图片算少了）时的兜底。
//!
//! 报超长那一刻定下压不压，记在回合上（[`super::turn::Turn`] 的 `passive`），回到准备好；这一批落了盘照常走发请求那一步，
//! 照那时的有效历史（环境、权限两块事实查过了）定压什么，先发摘要请求（`trigger` 是 `overflow`），压完照常组装、重发。一步只压一次：重发的这一次再报超长，照一次
//! 压缩失败算（第十条第 3 条）。

use super::Session;
use super::action::Action;
use super::compaction::Due;
use super::turn::Stage;
use crate::event::{Body, CompactTrigger, CompactionPaused, EndReason, Event, PauseReason};
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

/// 回合上记着的被动压缩（[`super::turn::Turn`] 的 `passive`）。
#[derive(Debug)]
pub(super) enum Passive {
    /// 主请求刚报了超长：发请求那一步照那时的有效历史定压什么。
    Due,
    /// 被动压缩的摘要请求要再来（能重试的错、截短、改走隔离式）：照上一次定的再压，不看压缩线。
    Again(Due),
}

impl Session {
    /// 主请求报了超长（`context_too_long`）。`replied` 是收到过输出、写成了半截回复。交回 `None` 是要先压再重发（记在
    /// 回合上，回到准备好，落了盘在发请求那一步定压什么，[`Session::passive_compaction`]）；交回 `Some` 是照出错结束，
    /// 里面是要排在 `turn.ended` 前面的事件（暂停）。
    ///
    /// 照出错结束的：这一步被动压过、重发还超长的（算一次失败，数到了写暂停）；收到过输出的（免得半截回答、重复执行）；
    /// 暂停着的；快照里没有压缩的。
    pub(super) fn overflow(
        &mut self,
        at: Timestamp,
        cause: Option<CommandId>,
        replied: bool,
    ) -> Option<Vec<Event>> {
        let turn = self.turn.as_ref()?;
        if turn.overflowed {
            return Some(self.after_failure(at, cause, None).into_iter().collect());
        }
        if replied || self.paused() || self.policy.compaction.is_none() {
            return Some(Vec::new());
        }
        let turn = self.turn.as_mut()?;
        turn.passive = Some(Passive::Due);
        turn.overflowed = true;
        turn.stage = Stage::Ready;
        None
    }

    /// 发请求那一步，主请求报过超长：定压什么再压（第六条第 3 条）。没有能压的，这一轮照出错结束；压完很快又到线连着到了
    /// 次数的，写暂停，照出错结束（第十条第 5 条）。
    pub(super) fn passive_compaction(&mut self, at: Timestamp) -> Vec<Action> {
        let cause = self.turn.as_ref().and_then(|turn| turn.cause.clone());
        let Some(due) = self.overflow_due() else {
            let events = self.finish_turn(at, By::Kernel, cause, EndReason::Error);
            return vec![Action::Append(events)];
        };
        if let Some(pause) = self.pause_numbers()
            && due.refills.is_some_and(|refills| refills >= pause.refills)
        {
            let body = Body::CompactionPaused(CompactionPaused {
                reason: PauseReason::TooLarge,
                failures: None,
                entry: self.largest(),
            });
            let mut events = vec![self.record(at, By::Kernel, cause.clone(), body)];
            events.extend(self.finish_turn(at, By::Kernel, cause, EndReason::Error));
            return vec![Action::Append(events)];
        }
        self.start_compaction(due)
    }

    /// 被动压缩压什么（第六条第 3 条）：尾巴照第三条第 2 条留、至少留最后一组；替代到的前面没有能压的，没有。压之前的用量
    /// 照这时的有效历史组装一次算；没交限额的（只有测试里）算 0。
    fn overflow_due(&self) -> Option<Due> {
        let compaction = self.policy.compaction.as_ref()?;
        let price = self.price()?;
        let budget = self
            .line()
            .map_or(compaction.tail, |line| compaction.tail.min(line / 4));
        let upto = self.compaction_upto(budget, &price, true)?;
        let request = self.policy.assembler.assemble(&self.history);
        Some(Due {
            upto,
            used: self.used(&request).unwrap_or(0),
            refills: self.refills(),
            trigger: CompactTrigger::Overflow,
            instructions: None,
        })
    }
}

#[cfg(test)]
mod tests;
