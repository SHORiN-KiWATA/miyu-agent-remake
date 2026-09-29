//! 压缩这一步（`docs/blueprint/compaction.md` 第二、三条，施工 6-2 上）：发主请求之前，用量过了压缩线，就先发一次
//! 摘要请求；取到了摘要，写 `context.compacted`，环境、权限两块事实比不到的注入，回到「准备好」再组装主请求。
//!
//! 摘要请求也是一次在路上的请求（[`super::call::Call`]），多记一格 [`Compacting`]：发出去、收增量、打断、重启都走
//! 同一条路，只有推给头的和说完了以后不一样。

use std::collections::BTreeSet;

use super::Session;
use super::action::Action;
use super::call::Call;
use super::call::Summarized;
use super::input::Reread;
use super::turn::Stage;
use crate::accumulate::{Delta, Kind};
use crate::estimate::{self, Price, WithImages};
use crate::event::{
    Body, CompactTrigger, CompactionDone, CompactionProgress, ContextCompacted, Event, Transient,
    TransientBody,
};
use crate::history::History;
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::request::Request;
use crate::time::Timestamp;

/// 进度的 `expected` 夹在这两头之间：压缩前的用量折成字数，输出约是输入的四分之一、一个 token 约四个字符，两下
/// 相抵就是用量本身（openclaude 的做法）。
const EXPECTED: (u64, u64) = (20_000, 80_000);

/// 要压：替代到哪、压之前的用量、压完很快又到线连着的第几次（施工 6-6 上，`breaker.rs`）。
pub(super) struct Due {
    pub(super) upto: Seq,
    pub(super) used: u64,
    pub(super) refills: Option<u32>,
}

/// 在路上的摘要请求多记的。
#[derive(Debug)]
pub(super) struct Compacting {
    /// 替代到哪一条，也是这次请求的 `seen`。
    upto: Seq,
    /// 哪一种压缩：记进这次请求的 `model.called`，失败照它数（施工 6-6 上）。现在只有 `auto`。
    trigger: CompactTrigger,
    /// 压完很快又到线连着的第几次，写进 `context.compacted`（施工 6-6 上）。
    refills: Option<u32>,
    /// 压之前的用量：过了线的那一次主请求算出的（施工 6-3 下，推 `compaction.done`）。
    before: u64,
    /// 估计要写多少字。
    expected: u64,
    /// 到这时收到的正文字数。
    written: u64,
    /// 正文块的编号：只数它们的字，思考不数。
    texts: BTreeSet<usize>,
    /// 交给执行器重读的候选，真实的位置（施工 6-5）；没交的是空的。
    paths: Vec<String>,
    /// 执行器送回的重读结果，和 `paths` 一个对一个；没收到的没有。
    reread: Option<Vec<Reread>>,
}

impl Compacting {
    /// 刚发出去、还没写字的进度（施工 6-3 下）。
    pub(super) fn started(&self) -> CompactionProgress {
        CompactionProgress {
            seen: self.upto,
            written: 0,
            expected: self.expected,
        }
    }

    /// 压之前的用量。
    pub(super) fn before(&self) -> u64 {
        self.before
    }

    /// 哪一种压缩。
    pub(super) fn trigger(&self) -> &CompactTrigger {
        &self.trigger
    }

    /// 压完很快又到线连着的第几次。
    pub(super) fn refills(&self) -> Option<u32> {
        self.refills
    }

    /// 执行器送回了重读结果（施工 6-5）：一个对一个的才收。
    pub(super) fn reread(&mut self, files: Vec<Reread>) {
        if files.len() == self.paths.len() {
            self.reread = Some(files);
        }
    }

    /// 交给执行器的候选，和送回来的重读结果（施工 6-5）。
    pub(super) fn rebuild_inputs(&self) -> (Vec<String>, Option<Vec<Reread>>) {
        (self.paths.clone(), self.reread.clone())
    }

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
    /// 用量。策略里没有压缩、没交限额、没有窗口的，不主动压；这一步已经压过的，照发。暂停着的由熔断先拦下（`breaker.rs`）。
    pub(super) fn compaction_due(&self, request: &Request) -> Option<(Seq, u64)> {
        if self.turn.as_ref()?.compacted {
            return None;
        }
        let compaction = self.policy.compaction.as_ref()?;
        let line = self.line()?;
        let price = self.price()?;
        let used = self.used(request)?;
        if used <= line {
            return None;
        }
        let budget = compaction.tail.min(line / 4);
        Some((self.compaction_upto(budget, &price)?, used))
    }

    /// 压缩线（`compaction.md` 第二条第 2 条）。策略里没有压缩、没交限额、没有窗口的，没有。
    pub(super) fn line(&self) -> Option<u64> {
        let compaction = self.policy.compaction.as_ref()?;
        let limits = self.limits.as_ref()?;
        estimate::line(
            limits.window,
            limits.max_output,
            compaction.reserve_cap,
            compaction.margin,
        )
    }

    /// 估算图片、文件的办法：驱动交了图片算法的照它，别的照策略里的固定数。策略里没有压缩、没交限额的，没有。
    pub(super) fn price(&self) -> Option<WithImages<'_>> {
        Some(WithImages {
            images: self.limits.as_ref()?.images.as_deref(),
            flat: self.policy.compaction.as_ref()?.price,
        })
    }

    /// 这份请求算出的用量（`compaction.md` 第一条）。
    fn used(&self, request: &Request) -> Option<u64> {
        self.used_in(&self.history, request)
    }

    /// 照有效历史 `history` 算这份请求的用量：锚取自它（施工 6-5：压后重建先照一份试着压过的算）。
    pub(super) fn used_in(&self, history: &History, request: &Request) -> Option<u64> {
        let limits = self.limits.as_ref()?;
        let anchor = estimate::anchor(history);
        Some(estimate::usage(
            request,
            anchor.as_ref(),
            &limits.model,
            &self.price()?,
        ))
    }

    /// 替代到哪（`compaction.md` 第三条第 2 条）：尾巴一组一组地留，不超过 `budget`（[`tail_upto`]）；这一轮要回应的话
    /// 里最早的那条当边界，至多替代到它前面那一条；一组不拆，切出来的是投影的开头一段（[`settle`]）。前面没有能压的，不压。
    ///
    /// 这一轮要回应的话：以前的回合里的请求没看到过、这一轮的回复也没看到过的人的消息；这一轮还没有回复的，加上触发
    /// 它的那一条（请求出错、没回复的，她没真看到）。
    fn compaction_upto(&self, budget: u64, price: &dyn Price) -> Option<Seq> {
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
        let ordered = self.history.ordered();
        let upto = tail_upto(&ordered, budget, price).map_or(upto, |tail| tail.min(upto));
        let upto = settle(&ordered, upto)?;
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
    pub(super) fn start_compaction(&mut self, due: Due) -> Vec<Action> {
        let Due {
            upto,
            used,
            refills,
        } = due;
        let paths = self.reread_paths(upto);
        let limit = self.reread_limit();
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
            trigger: CompactTrigger::Auto,
            refills,
            before: used,
            expected: used.clamp(EXPECTED.0, EXPECTED.1),
            written: 0,
            texts: BTreeSet::new(),
            paths: paths.clone(),
            reread: None,
        };
        let call = Call::new(upto, request.messages.len(), difference).compacting(compacting);
        turn.stage = Stage::Asking(call);
        // 重读排在摘要请求前面（施工 6-5）：执行器读完再发，摘要回来时结果已经记在这次请求上了。
        let mut actions = Vec::new();
        if !paths.is_empty() {
            actions.push(Action::Reread {
                seen: upto,
                paths,
                limit,
            });
        }
        actions.push(Action::CallModel {
            seen: upto,
            request,
            changed: difference,
        });
        actions
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

    /// 取到了摘要：挑好代码写的几段、重读的文件（施工 6-5，[`Session::rebuild`]），写 `context.compacted`，重读的原文
    /// 交给有效历史；环境、权限两块事实和压缩以后的有效历史比，比不到的注入；回到「准备好」，
    /// 这一批落了盘再组装主请求。返回追加的事件，和推给头的 `compaction.done`：压前、压后的用量，摘要请求的用量、
    /// 用时（施工 6-3 下）。压后照这时的有效历史组装一次算，和这一步接着要发的主请求一样。
    pub(super) fn compacted(
        &mut self,
        at: Timestamp,
        summarized: Summarized,
        cause: Option<CommandId>,
    ) -> (Vec<Event>, Option<Transient>) {
        let Summarized {
            upto,
            trigger,
            refills,
            summary,
            before,
            usage,
            duration_ms,
            paths,
            reread,
        } = summarized;
        let rebuilt = self.rebuild(at, upto, &summary, &paths, reread.as_deref());
        let body = Body::ContextCompacted(ContextCompacted {
            upto,
            summary,
            trigger: Some(trigger),
            notes: rebuilt.notes,
            restored: rebuilt.restored,
            refills,
        });
        let mut events = vec![self.record(at, By::Kernel, cause.clone(), body)];
        self.history.recall(rebuilt.texts);
        let turn = self.turn.as_mut().map(|turn| {
            turn.stage = Stage::Ready;
            turn.refresh = true;
            turn.compacted = true;
            turn.id
        });
        events.extend(self.refresh_facts(at));
        let request = self.policy.assembler.assemble(&self.history);
        let done = self.used(&request).map(|after| Transient {
            at,
            turn,
            by: By::Kernel,
            cause,
            body: TransientBody::CompactionDone(CompactionDone {
                seen: upto,
                before,
                after,
                usage,
                duration_ms,
            }),
        });
        (events, done)
    }
}

/// 留尾巴（`compaction.md` 第三条第 2 条，施工 6-2 下）：照投影的先后（`History::ordered`）看，一组从一条人的消息或者
/// 一条回复开始，两组之间的别的事件跟着前面那一组。从最新的一组往回，一组一组加进尾巴，加上就超过 `budget` 的停在它
/// 后面，交回尾巴前面那一条；最新的一组本身就超的，不留尾巴，是 `None`。
///
/// 只在「切得开」的地方切：前面的序号都比后面的小（[`cuts`]）。请求在路上时来的话，序号比回复小，投影里却排在回复后面，
/// 那里切不开，这一组并进前面那一组。
fn tail_upto(ordered: &[&Event], budget: u64, price: &dyn Price) -> Option<Seq> {
    let cuts = cuts(ordered);
    let mut size = 0u64;
    let mut upto = None;
    for (index, event) in ordered.iter().enumerate().rev() {
        size = size.saturating_add(estimate::event(event, price));
        if !matches!(event.body, Body::MessageUser(_) | Body::MessageAssistant(_)) {
            continue;
        }
        if size > budget {
            break;
        }
        if let Some(cut) = cuts[index] {
            upto = Some(cut);
        }
    }
    upto
}

/// 投影里第 `i` 条前面能不能切：前面的序号都比从它起的小，能的交回切在哪（从它起最小的序号前面那一条）。
fn cuts(ordered: &[&Event]) -> Vec<Option<Seq>> {
    let mut before = 0u64;
    let prefix: Vec<u64> = ordered
        .iter()
        .map(|event| {
            let max = before;
            before = before.max(event.seq.get());
            max
        })
        .collect();
    let mut after = u64::MAX;
    let mut cuts = vec![None; ordered.len()];
    for (index, event) in ordered.iter().enumerate().rev() {
        after = after.min(event.seq.get());
        if prefix[index] < after {
            cuts[index] = after.checked_sub(1).and_then(Seq::new);
        }
    }
    cuts
}

/// 定下切在哪（`compaction.md` 第三条第 2 条）：一组不拆，落在一条回复和它的工具结果之间的，退到这条回复前面；切出来的
/// 前一段要是投影的开头一段，不是的往前退到切得开的地方。两样都照到不动为止。
fn settle(ordered: &[&Event], mut upto: Seq) -> Option<Seq> {
    loop {
        let split = ordered
            .iter()
            .filter_map(|event| match &event.body {
                Body::ToolResult(result) if event.seq > upto => Some(result.call_id.message()),
                _ => None,
            })
            .filter(|reply| *reply <= upto)
            .min();
        let mut next = match split {
            Some(reply) => Seq::new(reply.get().checked_sub(1)?)?,
            None => upto,
        };
        if let Some(first) = ordered.iter().position(|event| event.seq > next)
            && ordered[first..].iter().any(|event| event.seq <= next)
        {
            let lowest = ordered[first..].iter().map(|event| event.seq).min()?;
            next = Seq::new(lowest.get().checked_sub(1)?)?;
        }
        if next == upto {
            return Some(upto);
        }
        upto = next;
    }
}

impl Compacting {
    /// 替代到哪一条。
    pub(super) fn upto(&self) -> Seq {
        self.upto
    }
}
