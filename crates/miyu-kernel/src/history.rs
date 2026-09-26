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

    /// 检查点之后还有效的事件，照每次请求当时看到的样子排好（03 第六节「照每次请求
    /// 看到的范围排」）。投影照这个先后一条条渲染。
    ///
    /// 以回复为界切段：一段是上一次请求看到的之后、下一次请求看到的为止。每一段里，
    /// 先是这一段开头的那条回复，接着是它的工具结果，按调用的先后；然后是这一段里
    /// 别的事件，照日志的先后。第一条回复之前的那一段照日志的先后。
    pub fn ordered(&self) -> Vec<&Event> {
        // 每条回复看到了第几条为止。账本保证它一次比一次大（02 第九节），所以可以当段的边界。
        let seen: Vec<Seq> = self
            .events
            .iter()
            .filter_map(|event| match &event.body {
                Body::MessageAssistant(reply) => Some(reply.seen),
                _ => None,
            })
            .collect();
        // 段 0 是第一条回复看到的那些；段 k 从第 k 条回复看到的之后开始。
        let mut segments: Vec<Vec<&Event>> = vec![Vec::new(); seen.len() + 1];
        for event in &self.events {
            let segment = seen.partition_point(|&boundary| boundary < event.seq);
            segments[segment].push(event);
        }
        segments.into_iter().flat_map(reply_first).collect()
    }

    /// 追加一条账本查过的事件。
    ///
    /// 压缩：换上新的检查点，序号在它 `upto` 之前的事件和旧的检查点一起丢掉，
    /// 新摘要里已经包着它们。撤销：丢掉撤掉的回合；撤回：丢掉撤回的消息。`turn.reverted`、
    /// `message.withdrawn` 本身用过就丢，它们不进上下文。其余的照先后留着。
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
            Body::MessageWithdrawn(withdrawn) => {
                let messages: BTreeSet<Seq> = withdrawn.messages.iter().copied().collect();
                self.events.retain(|kept| {
                    !(messages.contains(&kept.seq) && matches!(kept.body, Body::MessageUser(_)))
                });
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

/// 一段里的先后：这一段开头的那条回复，它的工具结果（按调用的先后），然后别的事件
/// （照日志的先后）。段 0 没有开头的回复，照日志的先后。
fn reply_first(segment: Vec<&Event>) -> Vec<&Event> {
    let Some(reply) = segment
        .iter()
        .copied()
        .find(|event| matches!(event.body, Body::MessageAssistant(_)))
    else {
        return segment;
    };
    let result_of_reply = |event: &Event| match &event.body {
        Body::ToolResult(result) if result.call_id.message() == reply.seq => {
            Some(result.call_id.index())
        }
        _ => None,
    };
    let mut results: Vec<&Event> = segment
        .iter()
        .copied()
        .filter(|event| result_of_reply(event).is_some())
        .collect();
    results.sort_by_key(|event| result_of_reply(event));
    let rest = segment
        .iter()
        .copied()
        .filter(|event| event.seq != reply.seq && result_of_reply(event).is_none());
    std::iter::once(reply).chain(results).chain(rest).collect()
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
