//! 账本里照记下的几条开的回合（施工 O-14 上，`docs/blueprint/chat.md` 第七条第 2 条「`turn.started` 多一格 `triggers`」）：
//! 哪些是旁听的 `message.user`，哪些已经当过触发。`turn.started` 带 `triggers` 的，每一条都要是旁听的、没当过触发的，照序号
//! 排好，最后一条就是 `trigger`；`turn.joined`（施工 O-14 下）的也照这几条查，不能是空的。撤掉的回合当过的照样算当过：账本只增
//! 不减。

use std::collections::BTreeSet;

use super::Ledger;
use crate::event::{Body, Event, TurnJoined, TurnStarted};
use crate::id::Seq;

/// 旁听记下的、当过触发的。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Triggers {
    /// 旁听的 `message.user`。
    overheard: BTreeSet<Seq>,
    /// 当过触发的旁听消息（`turn.started` 的 `triggers`）。
    answered: BTreeSet<Seq>,
}

impl Triggers {
    /// `turn.started` 的 `triggers` 合不合规矩。
    pub(super) fn check(&self, started: &TurnStarted) -> Result<(), String> {
        let Some(last) = started.triggers.last() else {
            return Ok(());
        };
        if started.trigger != Some(*last) {
            return Err("trigger should be the last of triggers".to_string());
        }
        self.check_list(&started.triggers)
    }

    /// `turn.joined` 的 `triggers` 合不合规矩（施工 O-14 下）：不能是空的，别的同 `turn.started` 的。
    pub(super) fn check_joined(&self, joined: &TurnJoined) -> Result<(), String> {
        if joined.triggers.is_empty() {
            return Err("turn.joined should have triggers".to_string());
        }
        self.check_list(&joined.triggers)
    }

    /// 几条触发：照序号排好不重，都是旁听的，都没当过触发。
    fn check_list(&self, triggers: &[Seq]) -> Result<(), String> {
        if triggers.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("triggers should be in order, each once".to_string());
        }
        if let Some(not) = triggers.iter().find(|seq| !self.overheard.contains(seq)) {
            return Err(format!("trigger {not} is not an overheard message.user"));
        }
        if let Some(used) = triggers.iter().find(|seq| self.answered.contains(seq)) {
            return Err(format!("trigger {used} has already opened a turn"));
        }
        Ok(())
    }

    /// 记下这一条。
    pub(super) fn record(&mut self, event: &Event) {
        match &event.body {
            Body::MessageUser(message)
                if message.venue.as_ref().is_some_and(|venue| venue.ambient) =>
            {
                self.overheard.insert(event.seq);
            }
            Body::TurnStarted(started) => self.answered.extend(&started.triggers),
            Body::TurnJoined(joined) => self.answered.extend(&joined.triggers),
            _ => {}
        }
    }
}

impl Ledger {
    /// 第 `seq` 条是这个会话里旁听的 `message.user`（施工 O-14 上）。
    pub fn overheard(&self, seq: Seq) -> bool {
        self.triggers.overheard.contains(&seq)
    }

    /// 第 `seq` 条已经当过触发（施工 O-14 上）：撤掉的回合当过的也算。
    pub fn answered(&self, seq: Seq) -> bool {
        self.triggers.answered.contains(&seq)
    }
}
