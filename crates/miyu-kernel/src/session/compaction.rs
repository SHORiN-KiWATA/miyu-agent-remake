//! 压缩这一步（`docs/blueprint/compaction.md` 第二、三条，施工 6-2 上）：发主请求之前，用量过了压缩线，就先发一次
//! 摘要请求；取到了摘要，写 `context.compacted`，环境、权限两块事实比不到的注入，回到「准备好」再组装主请求。
//!
//! 摘要请求也是一次在路上的请求（[`super::call::Call`]），多记一格 [`Compacting`]：发出去、收增量、打断、重启都走
//! 同一条路，只有推给头的和说完了以后不一样。

use std::collections::BTreeSet;

use super::Session;
use super::action::Action;
use super::call::Call;
use super::turn::Stage;
use crate::accumulate::{Delta, Kind};
use crate::estimate;
use crate::event::{
    Body, CompactTrigger, CompactionProgress, ContextCompacted, Event, Transient, TransientBody,
};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::request::Request;
use crate::time::Timestamp;

/// 进度的 `expected` 夹在这两头之间：压缩前的用量折成字数，输出约是输入的四分之一、一个 token 约四个字符，两下
/// 相抵就是用量本身（openclaude 的做法）。
const EXPECTED: (u64, u64) = (20_000, 80_000);

/// 在路上的摘要请求多记的。
#[derive(Debug)]
pub(super) struct Compacting {
    /// 替代到哪一条，也是这次请求的 `seen`。
    upto: Seq,
    /// 估计要写多少字。
    expected: u64,
    /// 到这时收到的正文字数。
    written: u64,
    /// 正文块的编号：只数它们的字，思考不数。
    texts: BTreeSet<usize>,
}

impl Compacting {
    /// 收到一段增量：正文块的字记上。是正文的一段字，交回要推给头的进度。
    pub(super) fn take(&mut self, delta: &Delta) -> Option<CompactionProgress> {
        match delta {
            Delta::Start {
                index,
                kind: Kind::Text,
            } => {
                self.texts.insert(*index);
                None
            }
            Delta::Text { index, text } if self.texts.contains(index) => {
                let chars = u64::try_from(text.chars().count()).unwrap_or(u64::MAX);
                self.written = self.written.saturating_add(chars);
                Some(CompactionProgress {
                    seen: self.upto,
                    written: self.written,
                    expected: self.expected,
                })
            }
            _ => None,
        }
    }
}

impl Session {
    /// 这一次主请求发之前要不要先压（`compaction.md` 第二条）：用量过了压缩线，而且有得压，交回替代到哪和压缩前的
    /// 用量。策略里没有压缩、没交限额、没有窗口的，不主动压。
    pub(super) fn compaction_due(&self, request: &Request) -> Option<(Seq, u64)> {
        let compaction = self.policy.compaction.as_ref()?;
        let limits = self.limits.as_ref()?;
        let line = estimate::line(
            limits.window,
            limits.max_output,
            compaction.reserve_cap,
            compaction.margin,
        )?;
        let anchor = estimate::anchor(&self.history);
        let used = estimate::usage(request, anchor.as_ref(), &limits.model, &compaction.price);
        if used <= line {
            return None;
        }
        Some((self.compaction_upto()?, used))
    }

    /// 替代到哪（`compaction.md` 第三条第 2 条）：这一轮要回应的话里最早的那条当边界，替代到它前面那一条；没有的，到
    /// 落了盘的最后一条。前面没有能压的，不压。
    ///
    /// 这一轮要回应的话：以前的回合里的请求没看到过、这一轮的回复也没看到过的人的消息；这一轮还没有回复的，加上触发
    /// 它的那一条（请求出错、没回复的，她没真看到）。
    fn compaction_upto(&self) -> Option<Seq> {
        let turn = self.turn.as_ref()?;
        let started = turn.id.started();
        let events = self.history.events();
        let asked_before = events
            .iter()
            .rev()
            .filter(|event| event.seq < started)
            .find_map(|event| match &event.body {
                Body::ModelCalled(called) => Some(called.seen),
                _ => None,
            });
        // 回复的 `seen` 一次比一次大，最后一条看到的最多。
        let answered = events.iter().rev().find_map(|event| match &event.body {
            Body::MessageAssistant(reply) => Some(reply.seen),
            _ => None,
        });
        let floor = asked_before.max(answered);
        let unanswered = events
            .iter()
            .find(|event| {
                matches!(event.body, Body::MessageUser(_))
                    && floor.is_none_or(|floor| event.seq > floor)
            })
            .map(|event| event.seq);
        let replied = events.iter().any(|event| {
            event.turn == Some(turn.id) && matches!(event.body, Body::MessageAssistant(_))
        });
        // 回合开头已经压掉了的（回合中途压过），没有这一条。
        let trigger = events
            .iter()
            .find_map(|event| match &event.body {
                Body::TurnStarted(opened) if event.seq == started => Some(opened.trigger),
                _ => None,
            })
            .filter(|_| !replied);
        let upto = match unanswered.into_iter().chain(trigger).min() {
            Some(boundary) => Seq::new(boundary.get().checked_sub(1)?)?,
            None => self.stored?,
        };
        let upto = whole_groups(events, upto)?;
        // 不比上一次压缩的 `upto` 晚的，有效历史里那以前的早就替代掉了，这里一条都找不到。
        events
            .iter()
            .any(|event| {
                event.seq <= upto
                    && matches!(
                        event.body,
                        Body::MessageUser(_) | Body::MessageAssistant(_) | Body::ToolResult(_)
                    )
            })
            .then_some(upto)
    }

    /// 发摘要请求：有效历史到第 `upto` 条的投影加摘要指令，名字是 `upto`。不算步数；也记进「上一次请求」，压完的
    /// 第一次主请求照它比出前缀从哪变了。
    pub(super) fn start_compaction(&mut self, upto: Seq, used: u64) -> Vec<Action> {
        let request = self.policy.assembler.summarize(&self.history, upto);
        let fingerprint = request.fingerprint();
        let difference = self
            .last_request
            .as_ref()
            .and_then(|before| fingerprint.first_difference(before));
        self.last_request = Some(fingerprint);
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        let compacting = Compacting {
            upto,
            expected: used.clamp(EXPECTED.0, EXPECTED.1),
            written: 0,
            texts: BTreeSet::new(),
        };
        let call = Call::new(upto, request.messages.len(), difference).compacting(compacting);
        turn.stage = Stage::Asking(call);
        vec![Action::CallModel {
            seen: upto,
            request,
            changed: difference,
        }]
    }

    /// 推给头的进度：哪一回合、内核引起的。
    pub(super) fn progress(
        at: Timestamp,
        turn: Option<crate::id::TurnId>,
        cause: Option<CommandId>,
        progress: CompactionProgress,
    ) -> Transient {
        Transient {
            at,
            turn,
            by: By::Kernel,
            cause,
            body: TransientBody::CompactionProgress(progress),
        }
    }

    /// 取到了摘要：写 `context.compacted`；环境、权限两块事实和压缩以后的有效历史比，比不到的注入；回到「准备好」，
    /// 这一批落了盘再组装主请求。返回追加的事件。
    pub(super) fn compacted(
        &mut self,
        at: Timestamp,
        upto: Seq,
        summary: String,
        cause: Option<CommandId>,
    ) -> Vec<Event> {
        let body = Body::ContextCompacted(ContextCompacted {
            upto,
            summary,
            trigger: Some(CompactTrigger::Auto),
        });
        let mut events = vec![self.record(at, By::Kernel, cause, body)];
        if let Some(turn) = self.turn.as_mut() {
            turn.stage = Stage::Ready;
            turn.refresh = true;
        }
        events.extend(self.refresh_facts(at));
        events
    }
}

/// 一组不拆（`compaction.md` 第三条第 2 条）：替代到的落在一条回复和它的工具结果之间的（回合中途排着的新话在结果
/// 前面），退到这条回复前面。结果都记在下一条回复以前，退一次不会又落进更早的一组里；还是照到不动为止查。
fn whole_groups(events: &[Event], mut upto: Seq) -> Option<Seq> {
    loop {
        let split = events
            .iter()
            .filter_map(|event| match &event.body {
                Body::ToolResult(result) if event.seq > upto => Some(result.call_id.message()),
                _ => None,
            })
            .filter(|reply| *reply <= upto)
            .min();
        match split {
            Some(reply) => upto = Seq::new(reply.get().checked_sub(1)?)?,
            None => return Some(upto),
        }
    }
}

impl Compacting {
    /// 替代到哪一条。
    pub(super) fn upto(&self) -> Seq {
        self.upto
    }
}
