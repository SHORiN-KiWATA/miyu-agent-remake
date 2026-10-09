//! 发一条消息（`docs/blueprint/kernel/session.md`「发一条消息」；施工 8-10 从 `session.rs` 挪出来，那边放不下了）：
//! 空的拒绝；别处来的照回报的规矩到（`messages.rs`）；空闲时记下、同一批开一个回合，回合进行中记下、排进队。场所里旁听的
//! 只记下（施工 O-13 上）。`events.append` 记的也在这里：不带回合编号，不开回合。

use super::action::{Action, Reason};
use super::{Appended, Session, rejected};
use crate::block::Block;
use crate::event::{Body, MessageUser, VenueMessage};
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 发一条消息：一块内容都没有、也没带场所的东西的拒绝，`empty_message`。别处来的（子代理的留言，施工 7-7；别的 harness 发来的话，
    /// 施工 7-10；别的会话发来的话，施工 C-2）照回报的规矩到。空闲时追加 `message.user`、同一批开一个回合；回合进行中
    /// 追加、带上这个回合、排进队，这一步里在等人的调用作废，急着插话的再跳过还没跑的，叫停的 `CancelTool` 排在 `Append`
    /// 后面。
    pub(super) fn send(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        (blocks, venue): (Vec<Block>, Option<VenueMessage>),
        urgent: bool,
    ) -> Vec<Action> {
        // 只有带的东西（图、表情、文件）、没有字的场所消息照样收（施工 O-13 补）：内容块和带的东西都没有才是空的。
        if blocks.is_empty() && venue.as_ref().is_none_or(|venue| venue.media.is_empty()) {
            return vec![rejected(id, Reason::EmptyMessage)];
        }
        if let Some(waker) = self.elsewhere(&by) {
            return self.elsewhere_says(id, by, at, blocks, waker);
        }
        let ambient = venue.as_ref().is_some_and(|venue| venue.ambient);
        let body = Body::MessageUser(MessageUser { blocks, venue });
        if ambient {
            // 旁听的：只记下，不开回合，回合进行中也不排进这一轮（施工 O-13 上）。
            let message = self.record_outside(at, by, Some(id.clone()), body);
            self.accept(id, vec![message.seq]);
            return vec![Action::Append(vec![message])];
        }
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

    /// `events.append`（施工 O-13 上）：记一条不带回合编号的事件，任何时候都收，不开回合、不打断。
    pub(super) fn append(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        event: Appended,
    ) -> Vec<Action> {
        let event = self.record_outside(at, by, Some(id.clone()), event.into_body());
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }
}
