//! 发一条消息（`docs/blueprint/kernel/session.md`「发一条消息」；施工 8-10 从 `session.rs` 挪出来，那边放不下了）：
//! 空的拒绝；别处来的照回报的规矩到（`messages.rs`）；空闲时记下、同一批开一个回合，回合进行中记下、排进队。

use super::action::{Action, Reason};
use super::{Session, rejected};
use crate::block::Block;
use crate::event::{Body, MessageUser};
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 发一条消息：一块内容都没有的拒绝，`empty_message`。别处来的（子代理的留言，施工 7-7；别的 harness 发来的话，
    /// 施工 7-10；别的会话发来的话，施工 C-2）照回报的规矩到。空闲时追加 `message.user`、同一批开一个回合；回合进行中
    /// 追加、带上这个回合、排进队，这一步里在等人的调用作废，急着插话的再跳过还没跑的，叫停的 `CancelTool` 排在 `Append`
    /// 后面。
    pub(super) fn send(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        blocks: Vec<Block>,
        urgent: bool,
    ) -> Vec<Action> {
        if blocks.is_empty() {
            return vec![rejected(id, Reason::EmptyMessage)];
        }
        if let Some(waker) = self.elsewhere(&by) {
            return self.elsewhere_says(id, by, at, blocks, waker);
        }
        let body = Body::MessageUser(MessageUser { blocks });
        let message = self.record(at, by.clone(), Some(id.clone()), body);
        self.accept(id.clone(), vec![message.seq]);
        let trigger = message.seq;
        let mut events = vec![message];
        let mut stops = Vec::new();
        if self.turn.is_none() {
            events.extend(self.open_turn(at, trigger, Some(id)));
        } else {
            self.enqueue(trigger, id.clone());
            let (voided, stopped) = self.void_waiting(at, &by, &id);
            events.extend(voided);
            stops = stopped;
            if urgent {
                events.extend(self.interject(at, by, id));
            }
        }
        let mut actions = vec![Action::Append(events)];
        actions.extend(stops);
        actions
    }
}
