//! 有效历史：投影要用的那一段（`docs/designs/03-事件模型.md` 第七节「有效历史」）。
//!
//! 从最近一次压缩算起，去掉撤销掉的回合。账本（[`crate::ledger`]）查过的事件才交给这里，
//! 这里只留还要发给模型的那些。压缩一次就丢掉更早的，撤销一次就丢掉撤掉的，
//! 所以占的内存随上下文窗口走，不随日志走（`07-存储.md` 第七节）。

use std::collections::BTreeSet;

use crate::event::{Body, Event};
use crate::id::{Seq, TurnId};
use crate::origin::By;

/// 一个会话的有效历史：最近一次压缩的检查点，加上它之后还有效的事件。
///
/// 投影时检查点排在最前面，后面是 [`History::events`]，照日志的先后。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    /// 最近一次压缩的检查点（`context.compacted`）。日志里它排在被动压缩保下来的尾巴后面，
    /// 投影里却要排在最前，所以单独放。
    checkpoint: Option<Event>,
    /// 检查点之后还有效的事件，照日志的先后。被动压缩保下来的尾巴也在这里。
    events: Vec<Event>,
}

impl History {
    /// 最近一次压缩的检查点；没压缩过就没有。
    pub fn checkpoint(&self) -> Option<&Event> {
        self.checkpoint.as_ref()
    }

    /// 检查点之后还有效的事件，照日志的先后。
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// 追加一条账本查过的事件。
    ///
    /// 压缩：换上新的检查点，序号在它 `upto` 之前的事件和旧的检查点一起丢掉，
    /// 新摘要里已经包着它们。撤销：丢掉撤掉的回合；`turn.reverted` 本身用过就丢，
    /// 它不进上下文。其余的照先后留着。
    pub fn append(&mut self, event: Event) {
        match &event.body {
            Body::ContextCompacted(compacted) => {
                let upto = compacted.upto;
                self.events.retain(|kept| kept.seq > upto);
                self.checkpoint = Some(event);
            }
            Body::TurnReverted(reverted) => {
                let turns: BTreeSet<TurnId> = reverted.turns.iter().copied().collect();
                let triggers = self.triggers_of(&turns);
                self.events.retain(|kept| !undone(kept, &turns, &triggers));
            }
            _ => self.events.push(event),
        }
    }

    /// 这几个回合各自由哪一条事件触发：照它们 `turn.started` 里的 `trigger` 找。
    fn triggers_of(&self, turns: &BTreeSet<TurnId>) -> BTreeSet<Seq> {
        self.events
            .iter()
            .filter_map(|event| match &event.body {
                Body::TurnStarted(started) if event.turn.is_some_and(|t| turns.contains(&t)) => {
                    Some(started.trigger)
                }
                _ => None,
            })
            .collect()
    }
}

/// 撤销时要一起去掉的：`turn` 是被撤的回合；或者它触发了被撤的回合，
/// 而且是人亲口发来的消息（`by` 是账号）。别处来的触发留着：子代理的回报、
/// 后台命令结束、定时触发、群里别人说的话、另一个会话发来的消息（03 第七节）。
fn undone(event: &Event, turns: &BTreeSet<TurnId>, triggers: &BTreeSet<Seq>) -> bool {
    let in_turn = event.turn.is_some_and(|turn| turns.contains(&turn));
    let said_by_the_person =
        matches!(event.body, Body::MessageUser(_)) && matches!(event.by, By::Person(_));
    in_turn || (triggers.contains(&event.seq) && said_by_the_person)
}

#[cfg(test)]
mod tests;
