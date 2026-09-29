//! 向上回报（施工 7-6，`docs/blueprint/agents.md` 第二条、第八条，`docs/blueprint/kernel/session.md`「向上回报」）：子会话
//! 的一轮结束时要不要报给父会话、报什么。内核只交出动作 [`Action::Report`]，执行器经端口交给父会话（命令 `Report`）。
//!
//! 欠不欠着一份回报，从日志一条条算（[`Duty`]）：活着时每追加一条记一次，载入时照日志再走一遍，算出来的一样。
//!
//! - 欠下：父会话发来的消息进了日志（派它的交代、父会话的留言），不管是开了一轮还是排进正在进行的那一轮。人发来的、
//!   它自己的子代理的留言和回报不欠。
//! - 报：欠着、闲着、它自己派出去的子代理都报过了、最近结束的那一轮不是被打断、被重启打断的。报的是最近结束的那一轮
//!   最后说的话，超长的留头尾；欠着的这几轮里人发来过消息的，注明 `person`。
//! - 什么时候交：活着时，事件都落了盘再交（[`Session::report_up`] 在「落了盘」里叫）；载入时，最后报过的那一份再交一次
//!   （父会话照命令编号认出重的），崩了、重启了没接着干的那一轮照样报，`reason` 是 `aborted`。

use std::collections::BTreeMap;

use super::Session;
use super::action::Action;
use super::policy::Reports;
use crate::block::Block;
use crate::event::{Body, ChildReason, EndReason, Event};
use crate::id::{SessionId, TurnId};
use crate::ledger::Ledger;
use crate::origin::By;

/// 交给父会话的一份回报（`child.reported` 的正文几格）：执行器补上任务编号、子会话，经端口交给父会话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upward {
    /// 报的是哪一轮结束时的：它最后说的话就是正文。执行器照它定命令编号，同一份再交一次，父会话认得出是重的。
    pub turn: TurnId,
    /// `done`：那一轮结束了，出错、到了步数上限的也算；`aborted`：崩了，或者重启以后没接着干。
    pub reason: ChildReason,
    /// 那一轮最后说的话，超过 `jobs.report_chars` 的留头尾；一个字都没说的是空的。
    pub text: String,
    /// 正文截过。
    pub truncated: bool,
    /// 欠着的这几轮里人发来过消息（`agents.md` 第二条第 4 条）。
    pub person: bool,
}

/// 子会话欠着父会话的回报：从日志一条条算。主会话一直什么都不欠。
#[derive(Debug, Default)]
pub(super) struct Duty {
    /// 父会话：`session.created` 的 `parent`，主会话没有。
    parent: Option<SessionId>,
    /// 有回合开着。
    open: bool,
    /// 这一轮里人发来过消息：开这一轮的、排进来的都算。一轮结束时并进欠着的那一份。
    person: bool,
    /// 欠着一份回报：里面是欠着的这几轮里人发来过消息没有。
    owed: Option<bool>,
    /// 最近结束的那一轮和它结束的原因。
    ended: Option<(TurnId, EndReason)>,
    /// 最近开的那一轮到现在最后说的话：最后一条有字的回复里的正文块，接起来。
    answer: String,
    /// 最后报出去的那一份：载入时再交一次。
    last: Option<Upward>,
}

impl Duty {
    /// 记下追加的一条。
    pub(super) fn note(&mut self, event: &Event) {
        match &event.body {
            Body::SessionCreated(created) => self.parent = created.parent.clone(),
            Body::TurnStarted(_) => {
                self.open = true;
                self.answer.clear();
            }
            Body::MessageUser(_) => match &event.by {
                By::Person(_) => self.person = true,
                By::Session(session) if self.parent.as_ref() == Some(&session.id) => {
                    self.owed.get_or_insert(false);
                }
                _ => {}
            },
            Body::MessageAssistant(reply) => {
                let said: String = reply
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        Block::Text(text) => Some(text.text.as_str()),
                        _ => None,
                    })
                    .collect();
                if !said.trim().is_empty() {
                    self.answer = said;
                }
            }
            Body::TurnEnded(ended) => {
                self.open = false;
                self.ended = event.turn.map(|turn| (turn, ended.reason.clone()));
                if let Some(person) = self.owed.as_mut() {
                    *person |= self.person;
                }
                self.person = false;
            }
            _ => {}
        }
    }

    /// 该报了：欠着、闲着、自己派出去的子代理都报过了，最近结束的那一轮不是被打断（等人说话或者被停）、被有计划的重启
    /// 打断（再起来接着干）的。
    pub(super) fn due(&self, ledger: &Ledger) -> bool {
        self.owed.is_some()
            && !self.open
            && self.ended.as_ref().is_some_and(|(_, reason)| {
                !matches!(reason, EndReason::Interrupted | EndReason::Restarted)
            })
            && ledger.waiting_children().next().is_none()
    }

    /// 报：交回这一份，记成最后报的那一份，不再欠着。
    pub(super) fn take(&mut self, reports: &Reports) -> Option<Upward> {
        let (turn, reason) = self.ended.clone()?;
        let person = self.owed.take()?;
        let (text, truncated) = cut(&self.answer, reports);
        let upward = Upward {
            turn,
            reason: match reason {
                EndReason::Aborted => ChildReason::Aborted,
                _ => ChildReason::Done,
            },
            text,
            truncated,
            person,
        };
        self.last = Some(upward.clone());
        Some(upward)
    }

    /// 最后报的那一份。
    pub(super) fn last(&self) -> Option<&Upward> {
        self.last.as_ref()
    }

    /// 载入以后没接着干：最近结束的那一轮是被有计划的重启打断的，再也不会走完，当它崩了，照 `aborted` 报，父会话不会
    /// 一直等（`agents.md` 第八条）。只改这里记着的原因，日志不动。
    pub(super) fn not_resumed(&mut self) {
        if let Some((_, reason)) = self.ended.as_mut()
            && *reason == EndReason::Restarted
        {
            *reason = EndReason::Aborted;
        }
    }
}

impl Session {
    /// 该向上回报了就报（「向上回报」）：追加过的事件都落了盘才交，由它开的那一轮已经在盘上。收到过「要重启了」的不交：
    /// 要关了，父会话也在停，交过去会把它重新载入；再载入时照日志算出来、照样交（施工 7-3 的旗，施工 7-6）。
    pub(super) fn report_up(&mut self) -> Option<Action> {
        if self.restarting || !self.unstored.is_empty() || !self.duty.due(&self.ledger) {
            return None;
        }
        self.duty.take(&self.policy.reports).map(Action::Report)
    }
}

/// 截正文：超过 `reports.chars` 个字的留头尾各一半，中间接一行省了多少个字。交回正文和截没截过。
fn cut(text: &str, reports: &Reports) -> (String, bool) {
    let total = text.chars().count();
    if total <= reports.chars {
        return (text.to_string(), false);
    }
    let head_chars = reports.chars / 2;
    let tail_chars = reports.chars - head_chars;
    let head: String = text.chars().take(head_chars).collect();
    let tail: String = text.chars().skip(total - tail_chars).collect();
    let count = (total - reports.chars).to_string();
    let seam = reports
        .omitted
        .render(&BTreeMap::from([("count", count.as_str())]))
        .unwrap_or_else(|error| {
            panic!("the omitted line was tried with count when the policy was built: {error}")
        });
    (format!("{head}\n{seam}{tail}"), true)
}

#[cfg(test)]
mod tests;
