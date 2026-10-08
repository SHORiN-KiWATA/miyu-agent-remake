//! 照记下的几条开一轮（施工 O-14 上，`docs/blueprint/chat.md` 第七条第 3 条第 1 项）：`Respond` 拿已经旁听记下的几条
//! `message.user` 当触发开一轮，不新记消息；带来的几块事实接在内核的事实后面，都排在触发前面。

use super::action::{Action, Reason};
use super::{Session, rejected, rejected_about};
use crate::event::{Body, ContextInjected};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 照 `to` 那几条开一轮，`facts` 原样记成 `by` 注入的事实，接在内核的事实后面。正在跑一轮的拒，`turn_running`（O-14 下
    /// 改成并进去）；一条都没有的拒，`empty_message`；有不是旁听的拒，`not_ambient`；有当过触发的拒，`already_answered`：
    /// 后两种带上是哪几条。`to` 照序号排好、去重。
    pub(super) fn respond(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        mut to: Vec<Seq>,
        facts: Vec<ContextInjected>,
    ) -> Vec<Action> {
        if self.turn.is_some() {
            return vec![rejected(id, Reason::TurnRunning)];
        }
        to.sort_unstable();
        to.dedup();
        let Some(&trigger) = to.last() else {
            return vec![rejected(id, Reason::EmptyMessage)];
        };
        let not: Vec<Seq> = to
            .iter()
            .copied()
            .filter(|seq| !self.ledger.overheard(*seq))
            .collect();
        if !not.is_empty() {
            return vec![rejected_about(id, Reason::NotAmbient, not)];
        }
        let answered: Vec<Seq> = to
            .iter()
            .copied()
            .filter(|seq| self.ledger.answered(*seq))
            .collect();
        if !answered.is_empty() {
            return vec![rejected_about(id, Reason::AlreadyAnswered, answered)];
        }
        let mut events = self.open_turn_on(at, trigger, to, Some(id.clone()));
        for fact in facts {
            events.push(self.record(
                at,
                by.clone(),
                Some(id.clone()),
                Body::ContextInjected(fact),
            ));
        }
        if let (Some(turn), Some(last)) = (self.turn.as_mut(), events.last()) {
            turn.stage = super::turn::Stage::Opening { opened: last.seq };
        }
        self.accept(id, events.iter().map(|event| event.seq).collect());
        vec![Action::Append(events)]
    }
}
