//! 请求在路上：执行器的三种回报怎么收，回复怎么写，出错怎么记（`docs/designs/02-内核.md`
//! 第六节「回复怎么收、回合怎么结束」）。
//!
//! 三种回报都带着这次请求的 `seen`，对不上的是过时的，不理。每次请求都记一条
//! `model.called`，出错的也记（`03-事件模型.md` 第三节「模型调用怎么写」）。

use super::Session;
use super::action::Action;
use super::turn::Stage;
use crate::accumulate::{Accumulator, Delta};
use crate::block::Block;
use crate::event::{
    Body, CallError, CallResult, EndReason, ErrorClass, FirstDifference, MessageAssistant,
    ModelCalled, ModelDelta, Piece, Transient, TransientBody, Usage,
};
use crate::id::{CommandId, ContentHash, Seq};
use crate::origin::{By, Model};
use crate::request::Difference;
use crate::time::Timestamp;

/// 在路上的一次请求。
#[derive(Debug)]
pub(super) struct Call {
    /// 这次请求看到了第几条为止，也是它的名字。
    seen: Seq,
    /// 统一的请求里有几条消息。
    messages: usize,
    /// 和这个会话上一次请求比，第一处不同在哪；只是接着加的、前面没有可比的，没有。
    difference: Option<Difference>,
    /// 发出去了没有。
    sent: Option<Sent>,
    /// 第一段增量到的时刻。
    first_token: Option<Timestamp>,
    /// 收到的增量。
    accumulator: Accumulator,
}

/// 请求发出去时，执行器报来的。
#[derive(Debug)]
struct Sent {
    /// 发出去的时刻，用时从这里算起。
    at: Timestamp,
    /// 发给了哪个端点的哪个模型。
    model: Model,
    /// 驱动编码以后的请求字节的哈希。
    request: ContentHash,
}

impl Call {
    /// 一次刚交给执行器、还没发出去的请求。
    pub(super) fn new(seen: Seq, messages: usize, difference: Option<Difference>) -> Call {
        Call {
            seen,
            messages,
            difference,
            sent: None,
            first_token: None,
            accumulator: Accumulator::default(),
        }
    }

    /// 收一段增量，返回要推给头的那一段：私有数据不推。还没发出去就来了增量、增量对不上，
    /// 都是出错。
    fn take(&mut self, at: Timestamp, delta: Delta) -> Result<Option<Pushed>, CallError> {
        let Some(model) = self.sent.as_ref().map(|sent| sent.model.clone()) else {
            return Err(bad_stream("请求还没发出去就来了增量".to_string()));
        };
        self.first_token.get_or_insert(at);
        let piece = match &delta {
            Delta::Start { index, kind } => Some((*index, Piece::Start(kind.clone()))),
            Delta::Text { index, text } => Some((*index, Piece::Text(text.clone()))),
            Delta::Private { .. } => None,
            Delta::End { index } => Some((*index, Piece::End)),
        };
        self.accumulator
            .apply(delta)
            .map_err(|error| bad_stream(error.to_string()))?;
        Ok(piece.map(|(index, piece)| Pushed {
            by: By::Model(model),
            index,
            piece,
        }))
    }
}

/// 要推给头的一段增量，和它的 `by`：那个模型。
struct Pushed {
    by: By,
    index: usize,
    piece: Piece,
}

impl Session {
    /// 请求发出去了：记下什么时候、发给了谁、请求字节的哈希。报两次的，只认第一次。
    pub(super) fn request_sent(
        &mut self,
        at: Timestamp,
        seen: Seq,
        model: Model,
        request: ContentHash,
    ) -> Vec<Action> {
        if let Some(call) = self.call(seen)
            && call.sent.is_none()
        {
            call.sent = Some(Sent { at, model, request });
        }
        Vec::new()
    }

    /// 模型的一段增量：交给累积器，收下了就推给头。出错的，这次请求按出错算，叫执行器
    /// 别再发了。
    pub(super) fn model_delta(&mut self, at: Timestamp, seen: Seq, delta: Delta) -> Vec<Action> {
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        let (id, cause) = (turn.id, turn.cause.clone());
        let Stage::Asking(call) = &mut turn.stage else {
            return Vec::new();
        };
        if call.seen != seen {
            return Vec::new();
        }
        match call.take(at, delta) {
            Ok(None) => Vec::new(),
            Ok(Some(Pushed { by, index, piece })) => vec![Action::PushTransient(Transient {
                at,
                turn: Some(id),
                by,
                cause,
                body: TransientBody::ModelDelta(ModelDelta { seen, index, piece }),
            })],
            Err(error) => {
                let mut actions = self.model_ended(at, seen, None, Some(error));
                actions.push(Action::CancelModel { seen });
                actions
            }
        }
    }

    /// 模型说完了：正常说完的写成回复；出错的，收到的半截不写。都记一条 `model.called`。
    /// 回复里没有工具调用、或者出错了，结束回合；有工具调用的，回合停在那里等工具（施工 2-4）。
    pub(super) fn model_ended(
        &mut self,
        at: Timestamp,
        seen: Seq,
        usage: Option<Usage>,
        error: Option<CallError>,
    ) -> Vec<Action> {
        let Some((call, cause)) = self.take_call(seen) else {
            return Vec::new();
        };
        let Call {
            seen,
            messages,
            difference,
            sent,
            first_token,
            accumulator,
        } = call;
        let mut error = error;
        let mut events = Vec::new();
        let mut calls_tools = false;
        match &sent {
            None if error.is_none() => {
                error = Some(bad_stream("请求还没发出去就说完了".to_string()));
            }
            Some(sent) if error.is_none() => {
                let blocks = accumulator.finish(self.ledger.next_seq());
                if blocks.is_empty() {
                    error = Some(CallError {
                        class: ErrorClass::EmptyReply,
                        message: "回复里一个块都没有".to_string(),
                    });
                } else {
                    calls_tools = blocks
                        .iter()
                        .any(|block| matches!(block, Block::ToolCall(_)));
                    let reply = MessageAssistant {
                        blocks,
                        seen,
                        interrupted: false,
                    };
                    let by = By::Model(sent.model.clone());
                    events.push(self.record(at, by, cause.clone(), Body::MessageAssistant(reply)));
                }
            }
            _ => {}
        }
        let called = ModelCalled {
            seen,
            endpoint: sent.as_ref().map(|sent| sent.model.endpoint.clone()),
            model: sent.as_ref().map(|sent| sent.model.model.clone()),
            request: sent.as_ref().map(|sent| sent.request.clone()),
            messages: messages as u64,
            first_difference: difference.map(FirstDifference::from),
            usage,
            first_token_ms: sent
                .as_ref()
                .zip(first_token)
                .map(|(sent, first)| millis(sent.at, first)),
            duration_ms: sent.as_ref().map(|sent| millis(sent.at, at)),
            result: if error.is_some() {
                CallResult::Error
            } else {
                CallResult::Ok
            },
            error: error.clone(),
        };
        events.push(self.record(at, By::Kernel, cause.clone(), Body::ModelCalled(called)));
        if error.is_some() {
            events.push(self.end_turn(at, cause, EndReason::Error));
        } else if !calls_tools {
            events.push(self.end_turn(at, cause, EndReason::Completed));
        }
        vec![Action::Append(events)]
    }

    /// 在路上、名字是 `seen` 的那次请求。
    fn call(&mut self, seen: Seq) -> Option<&mut Call> {
        match &mut self.turn.as_mut()?.stage {
            Stage::Asking(call) if call.seen == seen => Some(call),
            _ => None,
        }
    }

    /// 取走在路上、名字是 `seen` 的那次请求，连同回合的 `cause`。取走以后回合停在「等工具」：
    /// 说完了的请求，要么结束回合，要么等工具。
    fn take_call(&mut self, seen: Seq) -> Option<(Call, Option<CommandId>)> {
        self.call(seen)?;
        let turn = self.turn.as_mut()?;
        match std::mem::replace(&mut turn.stage, Stage::Tools) {
            Stage::Asking(call) => Some((call, turn.cause.clone())),
            _ => None,
        }
    }
}

/// 增量对不上、回报的先后不对：驱动或执行器的错。
fn bad_stream(message: String) -> CallError {
    CallError {
        class: ErrorClass::BadStream,
        message,
    }
}

/// 从 `from` 到 `to` 过了多少毫秒。时钟往回拨了，算 0。
fn millis(from: Timestamp, to: Timestamp) -> u64 {
    u64::try_from(to.unix_millis().saturating_sub(from.unix_millis())).unwrap_or(0)
}
