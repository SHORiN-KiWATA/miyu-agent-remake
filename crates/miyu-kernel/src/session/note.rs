//! 记几块事实，不开回合（施工 O-14 补，`docs/blueprint/venues.md`「记几块事实」）：通讯平台的桥退信用（她的话发不出去，下一步
//! 知道）。当场记成 `context.injected`：正在跑一轮的带这一轮的回合编号（`record` 照会话现在的回合填），她下一次请求就看到，撤销
//! 这一轮跟着撤；不打断、不作废在等的调用、不叫醒，这一轮没再请求就结束的，下一轮开头看到。空闲的不带回合编号，下一轮开头看到，
//! 不开回合。

use super::action::{Action, Reason};
use super::{Session, rejected};
use crate::event::{Body, ContextInjected};
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 把 `facts` 原样记成 `by` 注入的事实。一块都没有的拒，`empty_message`。
    pub(super) fn note(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        facts: Vec<ContextInjected>,
    ) -> Vec<Action> {
        if facts.is_empty() {
            return vec![rejected(id, Reason::EmptyMessage)];
        }
        let events: Vec<_> = facts
            .into_iter()
            .map(|fact| {
                self.record(
                    at,
                    by.clone(),
                    Some(id.clone()),
                    Body::ContextInjected(fact),
                )
            })
            .collect();
        self.accept(id, events.iter().map(|event| event.seq).collect());
        vec![Action::Append(events)]
    }
}
