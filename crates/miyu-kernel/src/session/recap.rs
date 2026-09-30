//! 回顾（施工 3-8 四补，`docs/blueprint/kernel/session.md`「回顾」）：头要一句回顾，照这一刻落了盘的有效历史组装一次单独的
//! 辅助请求（`Assembler::recap`，`kernel/request.md`「回顾的请求」），说完了记 `model.called`（`purpose` 是 `recap`）和
//! `session.recapped`，回应那一句。
//!
//! - 不属于哪一轮：有回合在进行时照收；两条事件都不带回合编号（撤哪一轮都不会跟着拿走，和回报一样），不进她的上下文，不碰
//!   主请求的「上一次请求」：它的前缀和主对话不相干，比了也没用。
//! - 没有新内容的：有效历史里最近一条 `session.recapped` 照到的和这一次一样，交回它，不请求。照到的是喂进去的最新那一条消息，
//!   所以改标题、工具结果、`model.called` 这些都不算新内容。
//! - 一次只有一个在路上：路上又来的并进这一次，一起回。执行器照 `upto` 分开主请求和回顾的回报，不和主请求的 `seen` 撞。
//! - 出错、回复里没有正文：记出错的 `model.called`，都拒绝，`recap_failed`，不再来。增量对不上的，记下错，等说完了一起收：
//!   不叫停，也就不会有叫停以后还到的回报串进下一次回顾。

use super::Session;
use super::action::{Action, Outcome, Reason};
use super::spans::millis;
use crate::accumulate::{Accumulator, Delta};
use crate::block::Block;
use crate::event::{
    Body, CallError, CallResult, ErrorClass, Event, ModelCalled, Purpose, SessionRecapped, Usage,
};
use crate::id::{CommandId, ContentHash, Seq};
use crate::origin::{By, Model};
use crate::time::Timestamp;

/// 在路上的那一次回顾。
#[derive(Debug)]
pub(super) struct Recapping {
    /// 照到第几条，也是这一次的名字。
    upto: Seq,
    /// 等着回应的命令，照收到的先后：第一个是要它的那一个，两条事件的 `cause` 是它。
    waiting: Vec<CommandId>,
    /// 请求里有几条消息。
    messages: usize,
    /// 发出去了：什么时候、发给了谁、请求字节的哈希。
    sent: Option<(Timestamp, Model, ContentHash)>,
    /// 第一段增量到的时刻。
    first_token: Option<Timestamp>,
    /// 收到的增量。
    accumulator: Accumulator,
    /// 增量对不上、还没发出去就来了增量：记下，说完了照出错收。
    broken: Option<CallError>,
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
            upto,
            waiting: vec![id],
            messages: request.messages.len(),
            sent: None,
            first_token: None,
            accumulator: Accumulator::default(),
            broken: None,
        });
        vec![Action::Recap { upto, request }]
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

    /// 在路上、名字是 `upto` 的那一次回顾。
    fn recapping(&mut self, upto: Seq) -> Option<&mut Recapping> {
        self.recapping
            .as_mut()
            .filter(|recapping| recapping.upto == upto)
    }

    /// 回顾的请求发出去了：记下什么时候、发给了谁。报两次的，只认第一次。
    pub(super) fn recap_sent(
        &mut self,
        at: Timestamp,
        upto: Seq,
        model: Model,
        request: ContentHash,
    ) -> Vec<Action> {
        if let Some(recapping) = self.recapping(upto) {
            recapping.sent.get_or_insert((at, model, request));
        }
        Vec::new()
    }

    /// 回顾的请求的一段增量：交给累积器，不推给头。对不上的记下错，说完了照出错收。
    pub(super) fn recap_delta(&mut self, at: Timestamp, upto: Seq, delta: Delta) -> Vec<Action> {
        let Some(recapping) = self.recapping(upto) else {
            return Vec::new();
        };
        if recapping.broken.is_some() {
            return Vec::new();
        }
        if recapping.sent.is_none() {
            recapping.broken = Some(bad_stream("请求还没发出去就来了增量"));
            return Vec::new();
        }
        recapping.first_token.get_or_insert(at);
        if let Err(error) = recapping.accumulator.apply(delta) {
            recapping.broken = Some(bad_stream(&error.to_string()));
        }
        Vec::new()
    }

    /// 回顾的请求说完了：取正文块连起来、去掉前后空白。取到了，记 `model.called` 和 `session.recapped`，等着的都回那一句；
    /// 出错的、还没发出去的、没有正文的，记出错的 `model.called`，等着的都拒绝，`recap_failed`。都落了盘才回。
    pub(super) fn recap_ended(
        &mut self,
        at: Timestamp,
        upto: Seq,
        usage: Option<Usage>,
        error: Option<CallError>,
    ) -> Vec<Action> {
        let Some(recapping) = self.recapping.take_if(|recapping| recapping.upto == upto) else {
            return Vec::new();
        };
        let Recapping {
            upto,
            waiting,
            messages,
            sent,
            first_token,
            accumulator,
            broken,
        } = recapping;
        let mut error = error.or(broken);
        if sent.is_none() && error.is_none() {
            error = Some(bad_stream("请求还没发出去就说完了"));
        }
        let text = accumulator
            .finish(self.ledger.next_seq())
            .iter()
            .filter_map(|block| match block {
                Block::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<String>()
            .trim()
            .to_string();
        if error.is_none() && text.is_empty() {
            error = Some(CallError {
                class: ErrorClass::EmptyReply,
                message: "the recap reply has no text".to_string(),
                status: None,
            });
        }
        let cause = waiting.first().cloned();
        let called = ModelCalled {
            seen: upto,
            endpoint: sent.as_ref().map(|(_, model, _)| model.endpoint.clone()),
            model: sent.as_ref().map(|(_, model, _)| model.model.clone()),
            request: sent.as_ref().map(|(_, _, request)| request.clone()),
            messages: messages as u64,
            first_difference: None,
            usage,
            first_token_ms: sent
                .as_ref()
                .zip(first_token)
                .map(|((asked, _, _), first)| millis(*asked, first)),
            duration_ms: sent.as_ref().map(|(asked, _, _)| millis(*asked, at)),
            blocks: None,
            result: match error {
                Some(_) => CallResult::Error,
                None => CallResult::Ok,
            },
            error: error.clone(),
            compaction: None,
            purpose: Some(Purpose::Recap),
        };
        let called = self.record_aside(at, cause.clone(), Body::ModelCalled(called));
        // 先见结果，后见回应：拒绝也等那条 `model.called` 落了盘。
        let mut after = called.seq;
        let mut events = vec![called];
        let outcome = match error {
            Some(_) => Outcome::Rejected {
                reason: Reason::RecapFailed,
            },
            None => {
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

    /// 造一条不带回合编号的事件：`by` 是内核。交给账本查过，记在账上，交给有效历史，等着落盘。
    ///
    /// # Panics
    ///
    /// 过不了账本：那是内核的 bug。
    fn record_aside(&mut self, at: Timestamp, cause: Option<CommandId>, body: Body) -> Event {
        let event = Event {
            seq: self.ledger.next_seq(),
            at,
            turn: None,
            by: By::Kernel,
            cause,
            body,
        };
        if let Err(error) = self.commit(&event) {
            panic!("the kernel's own event failed the ledger, a kernel bug: {error}");
        }
        event
    }
}

/// 增量对不上、回报的先后不对：驱动或执行器的错。
fn bad_stream(message: &str) -> CallError {
    CallError {
        class: ErrorClass::BadStream,
        message: message.to_string(),
        status: None,
    }
}
