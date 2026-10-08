//! 辅助请求在路上的那一次（施工 3-8 五补从 `recap.rs` 分出来，`docs/blueprint/kernel/session.md`「回顾」「起标题」）：回顾、
//! 起标题都是单独的一次请求，不接主对话的前缀、不碰主请求的「上一次请求」。回报照用途分给各自的一路（[`Session::aside_sent`]
//! 这几个），名字是它照到的那一条 `upto`，不和主请求的 `seen` 撞。
//!
//! 两种共用的是收回报的规矩：发出去了报两次的只认第一次；还没报发出去就来了增量、增量对不上的，记下错，后面的增量不收，
//! 等说完了照出错收（不叫停，叫停以后还到的回报会串进下一次）；说完了记一条带 `purpose` 的 `model.called`，不带回合编号。

use super::Session;
use super::action::Action;
use super::spans::millis;
use crate::accumulate::{Accumulator, Delta};
use crate::block::Block;
use crate::event::{
    Body, CallError, CallResult, Cost, ErrorClass, Event, ModelCalled, Purpose, Usage,
};
use crate::id::{CommandId, ContentHash, Seq};
use crate::origin::{By, Model};
use crate::time::Timestamp;

/// 在路上的一次辅助请求：照到第几条、发出去了没有、收到的增量。
#[derive(Debug)]
pub(super) struct Aside {
    /// 照到第几条，也是这一次的名字。
    pub(super) upto: Seq,
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

/// 说完了的一次辅助请求：记账的那条 `model.called`（还没编序号），和写成了的正文或者没写成的错。
pub(super) struct Finished {
    /// 那一条 `model.called` 的正文。
    pub(super) called: ModelCalled,
    /// 正文块连起来、去掉前后空白；出了错、没有正文的是错。
    pub(super) text: Result<String, CallError>,
}

impl Aside {
    /// 发出去之前：照到 `upto`，请求里有 `messages` 条消息。
    pub(super) fn new(upto: Seq, messages: usize) -> Aside {
        Aside {
            upto,
            messages,
            sent: None,
            first_token: None,
            accumulator: Accumulator::default(),
            broken: None,
        }
    }

    /// 发出去了：记下什么时候、发给了谁。报两次的，只认第一次。
    fn sent(&mut self, at: Timestamp, model: Model, request: ContentHash) {
        self.sent.get_or_insert((at, model, request));
    }

    /// 一段增量：交给累积器。对不上的记下错，说完了照出错收。
    fn delta(&mut self, at: Timestamp, delta: Delta) {
        if self.broken.is_some() {
            return;
        }
        if self.sent.is_none() {
            self.broken = Some(bad_stream("请求还没发出去就来了增量"));
            return;
        }
        self.first_token.get_or_insert(at);
        if let Err(error) = self.accumulator.apply(delta) {
            self.broken = Some(bad_stream(&error.to_string()));
        }
    }

    /// 说完了：正文块连起来、去掉前后空白，思考、工具调用不要。出错的、还没发出去的、没有正文的（`empty` 是那一句原话）
    /// 没写成。`next` 是累积器收尾时给块编号用的下一条序号。
    pub(super) fn finish(
        self,
        at: Timestamp,
        purpose: Purpose,
        spent: (Option<Usage>, Option<Cost>),
        error: Option<CallError>,
        next: Seq,
        empty: &str,
    ) -> Finished {
        let (mut called, blocks) = self.end(at, purpose, spent, error, next);
        let text = blocks.and_then(|blocks| {
            let text = blocks
                .iter()
                .filter_map(|block| match block {
                    Block::Text(text) => Some(text.text.as_str()),
                    _ => None,
                })
                .collect::<String>()
                .trim()
                .to_string();
            match text.is_empty() {
                true => Err(CallError {
                    class: ErrorClass::EmptyReply,
                    message: empty.to_string(),
                    status: None,
                }),
                false => Ok(text),
            }
        });
        if let Err(error) = &text {
            failed(&mut called, error);
        }
        Finished { called, text }
    }

    /// 说完了：记账的那条 `model.called`（还没编序号），和收到的块；出错的、还没发出去的是错（施工 6-11 上从
    /// [`Aside::finish`] 拆出来：提前压好的摘要照块取）。`next` 是累积器收尾时给块编号用的下一条序号。
    pub(super) fn end(
        self,
        at: Timestamp,
        purpose: Purpose,
        (usage, cost): (Option<Usage>, Option<Cost>),
        error: Option<CallError>,
        next: Seq,
    ) -> (ModelCalled, Result<Vec<Block>, CallError>) {
        let mut error = error.or(self.broken);
        if self.sent.is_none() && error.is_none() {
            error = Some(bad_stream("请求还没发出去就说完了"));
        }
        let blocks = self.accumulator.finish(next);
        let sent = self.sent.as_ref();
        let called = ModelCalled {
            seen: self.upto,
            endpoint: sent.map(|(_, model, _)| model.endpoint.clone()),
            model: sent.map(|(_, model, _)| model.model.clone()),
            request: sent.map(|(_, _, request)| request.clone()),
            messages: self.messages as u64,
            first_difference: None,
            usage,
            cost: cost.map(Box::new),
            first_token_ms: sent
                .zip(self.first_token)
                .map(|((asked, _, _), first)| millis(*asked, first)),
            duration_ms: sent.map(|(asked, _, _)| millis(*asked, at)),
            blocks: None,
            result: match error {
                Some(_) => CallResult::Error,
                None => CallResult::Ok,
            },
            error: error.clone(),
            compaction: None,
            purpose: Some(purpose),
        };
        let blocks = match error {
            Some(error) => Err(error),
            None => Ok(blocks),
        };
        (called, blocks)
    }
}

impl Session {
    /// 在路上、用途是 `purpose`、名字是 `upto` 的那一次辅助请求。
    fn aside(&mut self, purpose: &Purpose, upto: Seq) -> Option<&mut Aside> {
        let aside = match purpose {
            Purpose::Recap => self
                .recapping
                .as_mut()
                .map(|recapping| &mut recapping.aside),
            Purpose::Title => self.titling.as_mut(),
            Purpose::Compaction => self.prepare.aside(upto),
            Purpose::Other(_) => None,
        };
        aside.filter(|aside| aside.upto == upto)
    }

    /// 辅助请求发出去了：记下什么时候、发给了谁。不是在路上的那一次的不理。
    pub(super) fn aside_sent(
        &mut self,
        at: Timestamp,
        purpose: &Purpose,
        upto: Seq,
        model: Model,
        request: ContentHash,
    ) -> Vec<Action> {
        if let Some(aside) = self.aside(purpose, upto) {
            aside.sent(at, model, request);
        }
        Vec::new()
    }

    /// 辅助请求的一段增量：交给累积器，不推给头。不是在路上的那一次的不理。
    pub(super) fn aside_delta(
        &mut self,
        at: Timestamp,
        purpose: &Purpose,
        upto: Seq,
        delta: Delta,
    ) -> Vec<Action> {
        if let Some(aside) = self.aside(purpose, upto) {
            aside.delta(at, delta);
        }
        Vec::new()
    }

    /// 辅助请求说完了：照用途交给回顾、起标题各自收。不是在路上的那一次的不理。
    pub(super) fn aside_ended(
        &mut self,
        at: Timestamp,
        purpose: &Purpose,
        upto: Seq,
        spent: (Option<Usage>, Option<Cost>),
        error: Option<CallError>,
    ) -> Vec<Action> {
        match purpose {
            Purpose::Recap => self.recap_ended(at, upto, spent, error),
            Purpose::Title => self.title_ended(at, upto, spent, error),
            Purpose::Compaction => self.prepare_ended(at, upto, spent, error),
            Purpose::Other(_) => Vec::new(),
        }
    }

    /// 造一条不带回合编号的事件（辅助请求的几条）：`by` 是内核。交给账本查过，记在账上，交给有效历史，等着落盘。
    ///
    /// # Panics
    ///
    /// 过不了账本：那是内核的 bug。
    pub(super) fn record_aside(
        &mut self,
        at: Timestamp,
        cause: Option<CommandId>,
        body: Body,
    ) -> Event {
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

/// 一次辅助请求没写成：记账的那条改成出错、带上错；已经带着错的不动（施工 6-11 上：说完了才认出来的错也照这样记）。
pub(super) fn failed(called: &mut ModelCalled, error: &CallError) {
    if called.error.is_none() {
        called.result = CallResult::Error;
        called.error = Some(error.clone());
    }
}

/// 增量对不上、回报的先后不对：驱动或执行器的错。主请求（`call.rs`）、辅助请求共用（施工 8-15 从 `call.rs` 并过来）。
pub(super) fn bad_stream(message: &str) -> CallError {
    CallError {
        class: ErrorClass::BadStream,
        message: message.to_string(),
        status: None,
    }
}
