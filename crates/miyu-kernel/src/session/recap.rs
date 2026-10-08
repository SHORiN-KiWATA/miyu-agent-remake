//! 回顾（施工 3-8 四补，`docs/blueprint/kernel/session.md`「回顾」）：头要一句回顾，照这一刻落了盘的有效历史组装一次单独的
//! 辅助请求（`Assembler::recap`，`kernel/request.md`「回顾的请求」），说完了记 `model.called`（`purpose` 是 `recap`）和
//! `session.recapped`，回应那一句。
//!
//! - 不属于哪一轮：有回合在进行时照收；两条事件都不带回合编号（撤哪一轮都不会跟着拿走，和回报一样），不进她的上下文，不碰
//!   主请求的「上一次请求」：它的前缀和主对话不相干，比了也没用。
//! - 没有新内容的：有效历史里最近一条 `session.recapped` 照到的和这一次一样，交回它，不请求。照到的是喂进去的最新那一条消息，
//!   所以改标题、工具结果、`model.called` 这些都不算新内容。
//! - 一次只有一个在路上：路上又来的并进这一次，一起回。回报走辅助请求那一路（`aside.rs`，施工 3-8 五补分出来）：执行器照
//!   用途和 `upto` 分开主请求、回顾、起标题的回报。
//! - 出错、回复里没有正文：记出错的 `model.called`，都拒绝，`recap_failed`，不再来。增量对不上的，记下错，等说完了一起收：
//!   不叫停，也就不会有叫停以后还到的回报串进下一次回顾。

use super::Session;
use super::action::{Action, Outcome, Reason};
use super::aside::{Aside, Finished};
use crate::event::{Body, CallError, Cost, Purpose, SessionRecapped, Usage};
use crate::id::{CommandId, Seq};
use crate::time::Timestamp;

/// 在路上的那一次回顾。
#[derive(Debug)]
pub(super) struct Recapping {
    /// 在路上的请求：照到第几条、收到的增量。
    pub(super) aside: Aside,
    /// 等着回应的命令，照收到的先后：第一个是要它的那一个，两条事件的 `cause` 是它。
    waiting: Vec<CommandId>,
}

impl Session {
    /// 收下 `session.recap`：照这一刻落了盘的有效历史组装。没有能回顾的拒绝，`nothing_to_recap`；照到的和最近一条
    /// `session.recapped` 一样的，交回它（它落了盘才回）；有一次在路上的，并进去；不然发请求，名字是照到的那一条。
    pub(super) fn recap(&mut self, id: CommandId) -> Vec<Action> {
        let recap = self
            .stored
            .and_then(|stored| self.policy.assembler.recap(&self.history.until(stored)));
        let Some((request, upto)) = recap else {
            return vec![super::rejected(id, Reason::NothingToRecap)];
        };
        if let Some((seq, last)) = self.last_recap()
            && last.upto == upto
        {
            let outcome = Outcome::Recapped {
                text: last.text.clone(),
                upto,
                cached: true,
            };
            return self.reply_after(id, seq, outcome);
        }
        if let Some(recapping) = self.recapping.as_mut() {
            recapping.waiting.push(id);
            return Vec::new();
        }
        self.recapping = Some(Recapping {
            aside: Aside::new(upto, request.messages.len()),
            waiting: vec![id],
        });
        vec![Action::Aside {
            purpose: Purpose::Recap,
            upto,
            request,
        }]
    }

    /// 有效历史里最近一条 `session.recapped` 和它的序号；一条都没有的，没有。
    fn last_recap(&self) -> Option<(Seq, &SessionRecapped)> {
        self.history
            .events()
            .iter()
            .rev()
            .find_map(|event| match &event.body {
                Body::SessionRecapped(recapped) => Some((event.seq, recapped)),
                _ => None,
            })
    }

    /// 回顾的请求说完了：取正文块连起来、去掉前后空白。取到了，记 `model.called` 和 `session.recapped`，等着的都回那一句；
    /// 出错的、还没发出去的、没有正文的，记出错的 `model.called`，等着的都拒绝，`recap_failed`。都落了盘才回。
    pub(super) fn recap_ended(
        &mut self,
        at: Timestamp,
        upto: Seq,
        spent: (Option<Usage>, Option<Cost>),
        error: Option<CallError>,
    ) -> Vec<Action> {
        let Some(recapping) = self
            .recapping
            .take_if(|recapping| recapping.aside.upto == upto)
        else {
            return Vec::new();
        };
        let Recapping { aside, waiting } = recapping;
        let next = self.ledger.next_seq();
        let Finished { called, text } = aside.finish(
            at,
            Purpose::Recap,
            spent,
            error,
            next,
            "the recap reply has no text",
        );
        let cause = waiting.first().cloned();
        let called = self.record_aside(at, cause.clone(), Body::ModelCalled(called));
        // 先见结果，后见回应：拒绝也等那条 `model.called` 落了盘。
        let mut after = called.seq;
        let mut events = vec![called];
        let outcome = match text {
            Err(_) => Outcome::Rejected {
                reason: Reason::RecapFailed,
                about: Vec::new(),
            },
            Ok(text) => {
                let body = Body::SessionRecapped(SessionRecapped {
                    text: text.clone(),
                    upto,
                });
                let recapped = self.record_aside(at, cause, body);
                after = recapped.seq;
                events.push(recapped);
                Outcome::Recapped {
                    text,
                    upto,
                    cached: false,
                }
            }
        };
        let mut actions = vec![Action::Append(events)];
        for id in waiting {
            actions.extend(self.reply_after(id, after, outcome.clone()));
        }
        actions
    }
}
