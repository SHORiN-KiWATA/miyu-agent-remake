//! 命令的回应什么时候回（`docs/blueprint/kernel/session.md`「命令和回应」第 3、4、8 条；施工 3-8 四补从 `session.rs` 挪出来，
//! 那边放不下了）：接受的记下编号、等它的事件落了盘；接受过的编号再来，照上一次回；回顾的结局不是「接受了」，也照这一套等。

use super::Session;
use super::action::{Action, Outcome};
use crate::id::{CommandId, Seq};

impl Session {
    /// 接受一个命令：记下编号和它产生的事件，等落了盘再回应。
    pub(super) fn accept(&mut self, id: CommandId, events: Vec<Seq>) {
        self.recent.insert(id.clone(), events.clone());
        let outcome = Outcome::Accepted {
            events: events.clone(),
        };
        self.waiting.push((id, events, outcome));
    }

    /// 接受过的命令又来了：它的事件都落了盘，当场回应；还没有，排队等落盘。
    pub(super) fn reply_when_stored(&mut self, id: CommandId, events: Vec<Seq>) -> Vec<Action> {
        let outcome = Outcome::Accepted {
            events: events.clone(),
        };
        self.reply_when(id, events, outcome)
    }

    /// 第 `after` 条落了盘就回 `outcome`：已经落了盘的当场回（施工 3-8 四补：回顾的结局）。
    pub(super) fn reply_after(
        &mut self,
        id: CommandId,
        after: Seq,
        outcome: Outcome,
    ) -> Vec<Action> {
        self.reply_when(id, vec![after], outcome)
    }

    /// `events` 都落了盘就回 `outcome`：都落了盘的当场回，不然排队等落盘。
    pub(super) fn reply_when(
        &mut self,
        id: CommandId,
        events: Vec<Seq>,
        outcome: Outcome,
    ) -> Vec<Action> {
        if self.is_stored(&events) {
            vec![Action::Reply { id, outcome }]
        } else {
            self.waiting.push((id, events, outcome));
            Vec::new()
        }
    }

    /// 这几条事件都落了盘没有。
    pub(super) fn is_stored(&self, events: &[Seq]) -> bool {
        match (events.last(), self.stored) {
            (None, _) => true,
            (Some(last), Some(stored)) => *last <= stored,
            (Some(_), None) => false,
        }
    }
}
