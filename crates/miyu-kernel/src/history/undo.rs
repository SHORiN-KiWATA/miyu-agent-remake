//! 撤销与恢复（`docs/designs/03-事件模型.md` 第七节「有效历史」，`02-内核.md` 第六节「撤销与
//! 恢复」）：撤掉的回合连同跟着撤的话从有效历史里拿走，先放在一边；恢复时照序号放回原处。
//! 下一轮开始、压缩了，放在一边的就丢掉：那以后不能再恢复（账本查着）。

use std::collections::BTreeSet;

use super::History;
use crate::event::{Body, Event};
use crate::id::{Seq, TurnId};
use crate::origin::By;

impl History {
    /// 撤掉这几轮：`turn` 是它们的事件，和它们接过去的、人亲口说的话，一起拿走，放在一边。
    pub(super) fn revert(&mut self, turns: &[TurnId]) {
        let turns: BTreeSet<TurnId> = turns.iter().copied().collect();
        let taken = self.taken_over(&turns);
        let (gone, kept): (Vec<Event>, Vec<Event>) = std::mem::take(&mut self.events)
            .into_iter()
            .partition(|event| {
                event.turn.is_some_and(|turn| turns.contains(&turn)) || taken.contains(&event.seq)
            });
        self.events = kept;
        self.undone.push(gone);
    }

    /// 恢复最近一次撤销：放在一边的照序号放回原处。恢复只在下一轮开始之前，中间没发过请求，
    /// 所以放回去以后和撤销之前一模一样。
    pub(super) fn unrevert(&mut self) {
        if let Some(back) = self.undone.pop() {
            self.events.extend(back);
            self.events.sort_by_key(|event| event.seq);
        }
    }

    /// 这几轮接过去的、人亲口发来的消息（`message.user`，`by` 是账号），撤了就跟没说过一样：
    ///
    /// - 触发它们的那条；
    /// - 由上一轮排着队的消息接着开的那一轮（`02-内核.md` 第六节「排队的消息」第 2 条），上一轮
    ///   结束时还排着的那几条：它们带的是上一轮的编号，可她是在这一轮才听到的。
    ///
    /// 别处来的留着：子代理的回报、后台命令结束、定时触发、群里别人说的话、另一个会话发来的
    /// 消息，撤掉的只是她对它们的反应。
    fn taken_over(&self, turns: &BTreeSet<TurnId>) -> BTreeSet<Seq> {
        let mut taken = BTreeSet::new();
        for event in &self.events {
            let Body::TurnStarted(started) = &event.body else {
                continue;
            };
            if !event.turn.is_some_and(|turn| turns.contains(&turn)) {
                continue;
            }
            let Some(trigger) = self.find(started.trigger) else {
                continue;
            };
            if !matches!(trigger.body, Body::MessageUser(_)) {
                continue;
            }
            if said_by_the_person(trigger) {
                taken.insert(trigger.seq);
            }
            if let Some(previous) = trigger.turn.filter(|turn| !turns.contains(turn)) {
                taken.extend(
                    self.left_queued(previous)
                        .filter(|message| said_by_the_person(message))
                        .map(|message| message.seq),
                );
            }
        }
        taken
    }

    /// 回合 `turn` 结束时还排着队的消息：带着它的编号，它的请求一次都没看到过的 `message.user`。
    /// 请求看到哪里，看它记下的 `model.called` 和回复里的 `seen`。
    fn left_queued(&self, turn: TurnId) -> impl Iterator<Item = &Event> {
        let in_turn = move |event: &&Event| event.turn == Some(turn);
        let heard = self
            .events
            .iter()
            .filter(in_turn)
            .filter_map(|event| match &event.body {
                Body::ModelCalled(called) => Some(called.seen),
                Body::MessageAssistant(reply) => Some(reply.seen),
                _ => None,
            })
            .max();
        self.events
            .iter()
            .filter(in_turn)
            .filter(|event| matches!(event.body, Body::MessageUser(_)))
            .filter(move |event| heard.is_none_or(|heard| event.seq > heard))
    }

    /// 序号是 `seq` 的那条，还在有效历史里的话。
    fn find(&self, seq: Seq) -> Option<&Event> {
        self.events
            .binary_search_by_key(&seq, |event| event.seq)
            .ok()
            .map(|k| &self.events[k])
    }
}

/// 人亲口发来的消息：`message.user`，`by` 是有账号的人。
fn said_by_the_person(event: &Event) -> bool {
    matches!(event.body, Body::MessageUser(_)) && matches!(event.by, By::Person(_))
}
