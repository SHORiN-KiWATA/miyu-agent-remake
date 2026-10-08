//! 照记下的几条开一轮（施工 O-14 上，`docs/blueprint/chat.md` 第七条第 3 条第 1 项）：`Respond` 拿已经旁听记下的几条
//! `message.user` 当触发开一轮，不新记消息；带来的几块事实接在内核的事实后面，都排在触发前面。

use super::action::{Action, Reason};
use super::jobs::{Arrived, Waker};
use super::{Session, rejected, rejected_about};
use crate::event::{Body, ContextInjected, Event, TurnJoined};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 照 `to` 那几条开一轮，`facts` 原样记成 `by` 注入的事实，接在内核的事实后面。正在跑一轮的并进去（[`Session::join`]，施工
    /// O-14 下）；一条都没有的拒，`empty_message`；有不是旁听的拒，`not_ambient`；有当过触发的拒，`already_answered`：
    /// 后两种带上是哪几条。`to` 照序号排好、去重。
    pub(super) fn respond(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        mut to: Vec<Seq>,
        facts: Vec<ContextInjected>,
    ) -> Vec<Action> {
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
        if self.turn.is_some() {
            return self.join(id, by, at, to, facts);
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

    /// 正在跑一轮时（施工 O-14 下）：桥的事实、`turn.joined` 带这个回合当场记下，照回报排进队（下一步听到；没再请求就结束的
    /// 接着开一轮，由它触发；打断时不退回、不接着开）；这一步里在等人的调用作废，同回合中途来的人的话。叫停的 `CancelTool`
    /// 排在 `Append` 后面。
    fn join(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        triggers: Vec<Seq>,
        facts: Vec<ContextInjected>,
    ) -> Vec<Action> {
        let cause = Some(id.clone());
        let mut events: Vec<Event> = facts
            .into_iter()
            .map(|fact| self.record(at, by.clone(), cause.clone(), Body::ContextInjected(fact)))
            .collect();
        let joined = self.record(
            at,
            By::Kernel,
            cause.clone(),
            Body::TurnJoined(TurnJoined { triggers }),
        );
        if let Some(turn) = self.turn.as_mut() {
            turn.reports.push(Arrived {
                seq: joined.seq,
                cause,
                waker: Waker::Joined,
            });
        }
        events.push(joined);
        let (voided, stops) = self.void_waiting(at, &by, &id);
        events.extend(voided);
        self.accept(id, events.iter().map(|event| event.seq).collect());
        let mut actions = vec![Action::Append(events)];
        actions.extend(stops);
        actions
    }
}
