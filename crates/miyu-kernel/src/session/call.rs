//! 请求在路上：执行器的三种回报怎么收，回复怎么写，出错怎么记（`docs/designs/02-内核.md`
//! 第六节「回复怎么收、回合怎么结束」）。
//!
//! 三种回报都带着这次请求的 `seen`，对不上的是过时的，不理。每次请求都记一条
//! `model.called`，出错的也记（`03-事件模型.md` 第三节「模型调用怎么写」）。

use super::Session;
use super::action::Action;
use super::compaction::Compacting;
use super::input::Reread;
use super::spans::{Mark, Spans, millis};
use super::summary::{Summarized, called_tool};
use super::turn::Stage;
use crate::accumulate::{Accumulator, Delta};
use crate::block::{Block, ToolCall};
use crate::event::{
    Body, CallError, CallResult, CompactionProgress, EndReason, ErrorClass, Event, FirstDifference,
    MessageAssistant, ModelCalled, ModelDelta, Piece, Transient, TransientBody, Usage,
};
use crate::id::{CommandId, ContentHash, Seq};
use crate::origin::{By, Model};
use crate::request::Difference;
use crate::time::Timestamp;

/// 在路上的一次请求。
#[derive(Debug)]
pub(super) struct Call {
    /// 这次请求看到了第几条为止，也是它的名字。
    pub(super) seen: Seq,
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
    /// 每一块的起止，照流里的编号（施工 2-3 补）。
    spans: Spans,
    /// 这是压缩的摘要请求：替代到哪、进度（施工 6-2 上）。主请求没有。
    compaction: Option<Box<Compacting>>,
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
            spans: Spans::default(),
            compaction: None,
        }
    }

    /// 这一次是压缩的摘要请求。
    pub(super) fn compacting(mut self, compacting: Compacting) -> Call {
        self.compaction = Some(Box::new(compacting));
        self
    }

    /// 收一段增量，返回要推给头的那一段：私有数据不推；摘要请求推进度，正文以外的不推。还没发出去就来了增量、
    /// 增量对不上，都是出错。
    fn take(&mut self, at: Timestamp, delta: Delta) -> Result<Option<Pushed>, CallError> {
        let Some(model) = self.sent.as_ref().map(|sent| sent.model.clone()) else {
            return Err(bad_stream("请求还没发出去就来了增量".to_string()));
        };
        self.first_token.get_or_insert(at);
        let pushed = match self.compaction.as_mut() {
            Some(compacting) => compacting.take(&delta).map(Pushed::Progress),
            None => piece(&delta).map(|(index, piece)| Pushed::Delta {
                by: By::Model(model),
                index,
                piece,
            }),
        };
        let mark = Mark::of(&delta);
        self.accumulator
            .apply(delta)
            .map_err(|error| bad_stream(error.to_string()))?;
        self.spans.mark(at, mark);
        Ok(pushed)
    }
}

/// 一次请求的结局。
enum Ending {
    /// 执行器报说完了：正常说完的带用量，出错的带分类和原话。
    Said {
        usage: Option<Usage>,
        error: Option<CallError>,
    },
    /// 被人打断。
    CutOff,
}

/// 一次请求收拾完：追加的事件、写成的回复是第几条、回复里的调用、出错的分类和原话；摘要请求取到的摘要和它替代到
/// 哪。
struct Settled {
    events: Vec<Event>,
    reply: Option<Seq>,
    calls: Vec<ToolCall>,
    error: Option<CallError>,
    summary: Option<Summarized>,
    /// 摘要请求多记的，交回来：报了超长的照它截短再发（施工 6-6 中）。
    compaction: Option<Box<Compacting>>,
    /// 摘要回复里调了工具、要改走隔离式（施工 6-6 下）。
    isolating: bool,
}

/// 要推给头的：主请求的一段增量，和它的 `by`，那个模型；摘要请求的进度。
enum Pushed {
    Delta { by: By, index: usize, piece: Piece },
    Progress(CompactionProgress),
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
        let Some(turn) = self.turn.as_ref() else {
            return Vec::new();
        };
        let (id, cause) = (turn.id, turn.cause.clone());
        let Some(call) = self.call(seen) else {
            return Vec::new();
        };
        if call.sent.is_some() {
            return Vec::new();
        }
        call.sent = Some(Sent { at, model, request });
        // 摘要请求发出去了：先推一条还没写字的进度，头一收到就能印「正在压缩」（施工 6-3 下）。
        match call.compaction.as_ref() {
            Some(compacting) => vec![Action::PushTransient(Session::progress(
                at,
                Some(id),
                cause,
                compacting.started(),
            ))],
            None => Vec::new(),
        }
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
            Ok(Some(Pushed::Delta { by, index, piece })) => {
                vec![Action::PushTransient(Transient {
                    at,
                    turn: Some(id),
                    by,
                    cause,
                    body: TransientBody::ModelDelta(ModelDelta { seen, index, piece }),
                })]
            }
            Ok(Some(Pushed::Progress(progress))) => vec![Action::PushTransient(Session::progress(
                at,
                Some(id),
                cause,
                progress,
            ))],
            Err(error) => {
                let mut actions = self.model_ended(at, seen, None, Some(error), None, None);
                actions.push(Action::CancelModel { seen });
                actions
            }
        }
    }

    /// 模型说完了：正常说完的写成回复；出错的，收到的半截也写成回复，只留思考和正文（施工 3-5 下）。
    /// 都记一条 `model.called`。出了可以重试的错，等着再来（`retry.rs`）；不能重试的，结束回合；
    /// 回复里没有工具调用的，结束回合；有工具调用的，接着调工具。
    pub(super) fn model_ended(
        &mut self,
        at: Timestamp,
        seen: Seq,
        usage: Option<Usage>,
        error: Option<CallError>,
        wait_ms: Option<u64>,
        excess: Option<u64>,
    ) -> Vec<Action> {
        let Some((call, cause)) = self.take_call(seen) else {
            return Vec::new();
        };
        let compacting = call.compaction.is_some();
        let settled = self.settle(at, call, cause.clone(), Ending::Said { usage, error });
        let mut events = settled.events;
        if let Some(error) = settled.error {
            // 摘要请求自己超长：截掉最老的几组，落了盘再发（施工 6-6 中，`shorten.rs`）。
            if error.class == ErrorClass::ContextTooLong
                && settled
                    .compaction
                    .as_deref()
                    .is_some_and(|compacting| self.shorten(compacting, excess))
            {
                // 和到点再来一样，发之前这一轮切过的权限、环境照查一遍。
                events.extend(self.refresh_facts(at));
                return vec![Action::Append(events)];
            }
            // fork 式的摘要回复里调了工具：改走隔离式，落了盘再发（施工 6-6 下）。
            if settled.isolating
                && settled
                    .compaction
                    .as_deref()
                    .is_some_and(|compacting| self.isolate(compacting))
            {
                events.extend(self.refresh_facts(at));
                return vec![Action::Append(events)];
            }
            // 主请求报超长：被动压缩，一个字都没收到的落了盘先压再重发；别的照出错结束（施工 6-7，`overflow.rs`）。
            if error.class == ErrorClass::ContextTooLong && !compacting {
                let replied = settled.reply.is_some();
                match self.overflow(at, cause.clone(), replied) {
                    None => events.extend(self.refresh_facts(at)),
                    Some(before) => {
                        events.extend(before);
                        events.extend(self.finish_turn(at, By::Kernel, cause, EndReason::Error));
                    }
                }
                return vec![Action::Append(events)];
            }
            if let Some(wait) = self.retry_wait(&error, wait_ms) {
                // 再来的是摘要请求，不标「下一次是重试」：它后面那一次主请求照常算一步。被动压缩的摘要请求连压什么也记回去，
                // 到点了照它再压（施工 6-7）：它不看压缩线。截到哪、截了几次、是不是隔离式也记回去，照它再发（施工 6-6 补）。
                let compaction = settled.compaction.as_deref();
                let passive = compaction.and_then(super::shorten::passive);
                let again = compaction.map(super::shorten::as_before);
                if let Some(turn) = self.turn.as_mut() {
                    turn.retrying |= !compacting;
                    turn.passive = passive.map(super::overflow::Passive::Again);
                    turn.again = again.or(turn.again);
                }
                let cut = settled.reply.is_some();
                return self.wait_to_retry(at, seen, cause, events, cut, error, wait);
            }
            // 摘要请求不再来了，是一次压缩失败：连着数到了次数，暂停排在 `turn.ended` 前面（施工 6-6 上）；手动的不数。
            if let Some(compacting) = settled.compaction.as_deref() {
                events.extend(self.after_failure(at, cause.clone(), Some(compacting.trigger())));
            }
            events.extend(self.finish_turn(at, By::Kernel, cause, EndReason::Error));
            return vec![Action::Append(events)];
        }
        // 摘要请求说完了不清零：重试次数和这一步的主请求合用一个计数（施工 6-2 下）。
        if let Some(summarized) = settled.summary {
            let (compacted, done) = self.compacted(at, summarized, cause);
            events.extend(compacted);
            let mut actions = vec![Action::Append(events)];
            actions.extend(done.map(Action::PushTransient));
            return actions;
        }
        if let Some(turn) = self.turn.as_mut() {
            turn.retries = 0;
            turn.overflowed = false;
        }
        match settled.reply {
            Some(reply) if !settled.calls.is_empty() => {
                events.extend(self.start_tools(at, reply, settled.calls, cause));
            }
            _ => events.extend(self.finish_turn(at, By::Kernel, cause, EndReason::Completed)),
        }
        vec![Action::Append(events)]
    }

    /// 打断在路上的请求：收到的半截照累积器留下，不是空的写成回复，多写一格被打断；
    /// 记一条结果是被打断的 `model.called`。返回追加的事件和半截回复里留下的调用。
    pub(super) fn cut_off(
        &mut self,
        at: Timestamp,
        call: Call,
        cause: Option<CommandId>,
    ) -> (Vec<Event>, Vec<ToolCall>) {
        let settled = self.settle(at, call, cause, Ending::CutOff);
        (settled.events, settled.calls)
    }

    /// 这次请求有了结局：该写的回复写上，记一条 `model.called`。
    fn settle(
        &mut self,
        at: Timestamp,
        call: Call,
        cause: Option<CommandId>,
        ending: Ending,
    ) -> Settled {
        let Call {
            seen,
            messages,
            difference,
            sent,
            first_token,
            accumulator,
            spans,
            compaction,
        } = call;
        let (usage, mut error, cut) = match ending {
            Ending::Said { usage, error } => (usage, error, false),
            Ending::CutOff => (None, None, true),
        };
        let mut events = Vec::new();
        let mut reply = None;
        let mut calls = Vec::new();
        let mut summary = None;
        let mut isolating = false;
        let mut times = None;
        match &sent {
            None if !cut && error.is_none() => {
                error = Some(bad_stream("请求还没发出去就说完了".to_string()));
            }
            // 摘要请求不写回复，说完了的取出摘要（施工 6-2 上）。
            Some(_) if compaction.is_some() => {
                if !cut && error.is_none() {
                    let blocks = accumulator.finish(self.ledger.next_seq());
                    let isolates = compaction
                        .as_deref()
                        .is_some_and(|compacting| self.can_isolate(compacting));
                    match self.summary_of(&blocks, isolates) {
                        Ok(text) => summary = Some(text),
                        Err(bad) => {
                            isolating = isolates && called_tool(&blocks);
                            error = Some(bad);
                        }
                    }
                }
            }
            Some(sent) => {
                let seq = self.ledger.next_seq();
                // 出错断了的，照打断的规矩留下半截，工具调用一个不留：没收全的执行不了，收全了的
                // 也不派，回复没说完（02 第六节「回复怎么收」第 4 条）。
                let failed = error.is_some();
                let mut numbered = accumulator.numbered(seq, !cut && !failed);
                if failed {
                    numbered.retain(|(_, block)| !matches!(block, Block::ToolCall(_)));
                }
                let (kept, blocks): (Vec<usize>, Vec<Block>) = numbered.into_iter().unzip();
                if blocks.is_empty() {
                    if !cut && !failed {
                        error = Some(CallError {
                            class: ErrorClass::EmptyReply,
                            message: "回复里一个块都没有".to_string(),
                            status: None,
                        });
                    }
                } else {
                    calls = blocks
                        .iter()
                        .filter_map(|block| match block {
                            Block::ToolCall(call) => Some(call.clone()),
                            _ => None,
                        })
                        .collect();
                    let body = MessageAssistant {
                        blocks,
                        seen,
                        interrupted: cut || failed,
                    };
                    let by = By::Model(sent.model.clone());
                    events.push(self.record(at, by, cause.clone(), Body::MessageAssistant(body)));
                    reply = Some(seq);
                    times = Some(spans.of(sent.at, kept));
                }
            }
            None => {}
        }
        let result = if cut {
            CallResult::Interrupted
        } else if error.is_some() {
            CallResult::Error
        } else {
            CallResult::Ok
        };
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
            blocks: times,
            result,
            error: error.clone(),
            compaction: compaction
                .as_ref()
                .map(|compacting| compacting.trigger().clone()),
        };
        let summary = summary
            .zip(compaction.as_ref())
            .map(|(summary, compacting)| {
                let (paths, reread) = compacting.rebuild_inputs();
                Summarized {
                    upto: compacting.upto(),
                    trigger: compacting.trigger().clone(),
                    instructions: compacting.instructions(),
                    refills: compacting.refills(),
                    cut: compacting.shortened().0,
                    summary,
                    before: compacting.before(),
                    usage: called.usage,
                    duration_ms: called.duration_ms,
                    paths,
                    reread,
                }
            });
        events.push(self.record(at, By::Kernel, cause, Body::ModelCalled(called)));
        Settled {
            events,
            reply,
            calls,
            error,
            summary,
            compaction,
            isolating,
        }
    }

    /// 执行器送回了第 `seen` 次摘要请求的重读结果（施工 6-5）：记在那次请求上。不是在路上的那一次的，不理。
    pub(super) fn reread_done(&mut self, seen: Seq, files: Vec<Reread>) -> Vec<Action> {
        if let Some(compacting) = self.call(seen).and_then(|call| call.compaction.as_mut()) {
            compacting.reread(files);
        }
        Vec::new()
    }

    /// 在路上、名字是 `seen` 的那次请求。
    fn call(&mut self, seen: Seq) -> Option<&mut Call> {
        match &mut self.turn.as_mut()?.stage {
            Stage::Asking(call) if call.seen == seen => Some(call),
            _ => None,
        }
    }

    /// 取走在路上、名字是 `seen` 的那次请求，连同回合的 `cause`。取走以后回合在收拾：
    /// 说完了的请求，要么结束回合，要么接着调工具。
    fn take_call(&mut self, seen: Seq) -> Option<(Call, Option<CommandId>)> {
        self.call(seen)?;
        let turn = self.turn.as_mut()?;
        match std::mem::replace(&mut turn.stage, Stage::Settling) {
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
        status: None,
    }
}

/// 主请求推给头的那一段：私有数据不推。
fn piece(delta: &Delta) -> Option<(usize, Piece)> {
    match delta {
        Delta::Start { index, kind } => Some((*index, Piece::Start(kind.clone()))),
        Delta::Text { index, text } => Some((*index, Piece::Text(text.clone()))),
        Delta::Private { .. } => None,
        Delta::End { index } => Some((*index, Piece::End)),
    }
}
