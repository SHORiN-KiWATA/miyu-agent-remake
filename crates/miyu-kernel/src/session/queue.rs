//! 排队的消息（`docs/designs/02-内核.md` 第六节「排队的消息」）：回合进行中来的消息排着队，
//! 下一次请求里就有它；最后一步里排着的，回合结束时接着开一轮；打断退回时撤回来。

use super::Session;
use crate::event::{Body, EndReason, Event, MessageWithdrawn};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 回合进行中来了一条消息：排进队里，等下一次请求。
    pub(super) fn enqueue(&mut self, message: Seq, cause: CommandId) {
        if let Some(turn) = self.turn.as_mut() {
            turn.queued.push((message, Some(cause)));
        }
    }

    /// 结束正在进行的回合。还有排着队的，接着开下一轮，由最后那一条触发，`cause` 是它的；
    /// 两轮在同一批里追加。返回追加的事件。
    pub(super) fn finish_turn(
        &mut self,
        at: Timestamp,
        by: By,
        cause: Option<CommandId>,
        reason: EndReason,
    ) -> Vec<Event> {
        let next = self
            .turn
            .as_ref()
            .and_then(|turn| turn.queued.last().cloned());
        let mut events = vec![self.end_turn(at, by, cause, reason)];
        if let Some((trigger, cause)) = next {
            events.extend(self.open_turn(at, trigger, cause));
        }
        events
    }

    /// 打断退回：把排着队的撤回来，追加一条 `message.withdrawn`，`by` 是打断的人、`cause` 是
    /// 打断的命令。没有排着队的，什么都不追加。撤回以后队里是空的，回合结束时不再接着开。
    pub(super) fn withdraw_queued(
        &mut self,
        at: Timestamp,
        by: &By,
        cause: &CommandId,
    ) -> Option<Event> {
        let turn = self.turn.as_mut()?;
        let messages: Vec<Seq> = turn.queued.drain(..).map(|(seq, _)| seq).collect();
        if messages.is_empty() {
            return None;
        }
        let body = Body::MessageWithdrawn(MessageWithdrawn { messages });
        Some(self.record(at, by.clone(), Some(cause.clone()), body))
    }
}
