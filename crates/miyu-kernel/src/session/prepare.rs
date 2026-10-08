//! 提前压好、到线换上（`docs/blueprint/compaction.md` 第十五条，`09-压缩.md` 第十节，施工 6-11 上）：用量过了起压线（压缩线
//! 减 G），就照现在的切法切出 N、发一次摘要的旁路请求（`purpose` 是 `compaction`），主请求照发；取到的摘要只放在内存里。
//! 到线（自动压缩、手动压缩没附要求的）时手里有一份还用得上的、N 以后的尾巴放得下的，不再请求：交执行器重读，回来了写
//! `context.compacted`，推带 `prepared` 的 `compaction.done`。用不上的扔掉，照现在的办法当场压。
//!
//! 还用得上：有效历史里 N 以前的没变。起压时照检查点和 N 以前还算数的事件的序号算一个指纹（[`Session::prefix_key`]），换上前
//! 再算一次比：撤销撤到了 N 或更早、清空、中间压过一次、撤掉压缩，都会让它变。
//!
//! 出错、取不出摘要的扔掉，不重试、不数进熔断；fork 式调了工具、策略里有隔离式的，照隔离式再发一次。只在内存里：载入、
//! 重启以后没有，到了起压线再压一次。
//!
//! 到线时那一次还在路上、还用得上的（施工 6-11 下），回合进 [`Stage::Awaiting`] 等它：先推一条进度（写了的是已经收到的
//! 字数），之后它每来一段正文推一次；回来了用得上的换上，用不上的当场压。等的时候打断、重启，它接着在后台跑。

use std::hash::{DefaultHasher, Hash, Hasher};

use super::Session;
use super::action::Action;
use super::aside::{Aside, failed};
use super::compaction::{Due, expected, settle};
use super::input::Reread;
use super::summary::{Summarized, called_tool};
use super::turn::Stage;
use crate::estimate;
use crate::event::{Body, CallError, CompactionProgress, Cost, Purpose, Usage};
use crate::id::Seq;
use crate::request::Request;
use crate::time::Timestamp;

/// 提前压好的账：这一轮开没开，在路上的那一次或者压好了的那一份。只在内存里。
#[derive(Debug, Default)]
pub(super) struct Prepare {
    /// 这一轮开着（[`super::Input::TurnStartHooksDone`] 的 `prepare`）：回合开始时执行器照配置交；造会话、载入以后还没开过
    /// 回合的是关着。
    pub(super) on: bool,
    /// 在路上的那一次，或者压好了的那一份。
    state: Option<State>,
}

/// 在路上，或者压好了。
#[derive(Debug)]
enum State {
    /// 在路上：请求，起压时的指纹，是不是改走隔离式的那一次。
    Asking {
        aside: Aside,
        key: u64,
        isolated: bool,
    },
    /// 压好了。
    Ready(Prepared),
}

/// 压好了的那一份：替代到哪、起压时的指纹、摘要，和那一次摘要请求的用量、用时（换上时推给头）。
#[derive(Debug)]
struct Prepared {
    upto: Seq,
    key: u64,
    summary: String,
    usage: Option<Usage>,
    duration_ms: Option<u64>,
}

/// 到线时在等提前压的那一次（施工 6-11 下）：这次压缩压什么，进度的 `expected`。
#[derive(Debug)]
pub(super) struct Awaiting {
    due: Due,
    expected: u64,
}

impl Prepare {
    /// 在路上、名字是 `upto` 的那一次。
    pub(super) fn aside(&mut self, upto: Seq) -> Option<&mut Aside> {
        match &mut self.state {
            Some(State::Asking { aside, .. }) if aside.upto == upto => Some(aside),
            _ => None,
        }
    }
}

impl Session {
    /// 尾巴的预算 T 和提前量 G（第十五条第 1 条）：T = min(策略的尾巴, 压缩线的四分之一)，G = min(策略的提前量, 压缩线的
    /// 四分之一 − T)。没有压缩线的没有。
    fn lead(&self) -> Option<(u64, u64)> {
        let compaction = self.policy.compaction.as_ref()?;
        let quarter = self.line()? / 4;
        let tail = compaction.tail.min(quarter);
        Some((tail, compaction.lead.min(quarter - tail)))
    }

    /// 发主请求之前（`turn.rs` 的 `ask`，熔断说照发以后）：该起压就起（第十五条第 1 条），交回旁路请求，排在主请求后面。
    /// 开关关着、要关了、暂停着、G 是 0、用量没过起压线或者已经过了压缩线、有一次在路上、手里那一份还用得上的，不起；
    /// 手里那一份用不上了的扔掉。
    pub(super) fn prepare_up(&mut self, request: &Request) -> Option<Action> {
        if !self.prepare.on || self.restarting || self.paused() {
            return None;
        }
        let (tail, lead) = self.lead()?;
        let line = self.line()?;
        let used = self.used(request)?;
        // G 是 0 的照这一条不起：用量没过线就不到起压线。
        if used <= line - lead || used > line {
            return None;
        }
        match &self.prepare.state {
            Some(State::Asking { .. }) => return None,
            Some(State::Ready(prepared))
                if self.usable(prepared.upto, prepared.key, tail + lead) =>
            {
                return None;
            }
            _ => self.prepare.state = None,
        }
        let price = self.price()?;
        let upto = self.compaction_upto(tail, &price, false)?;
        Some(self.prepare_ask(upto, self.prefix_key(upto), false))
    }

    /// 发提前压好的摘要请求：和当场压的一样（有效历史到 `upto` 的投影加摘要指令，看不了图的照样换成转述），`isolated` 的
    /// 照隔离式；走旁路，名字是 `upto`，不碰「上一次请求」。
    fn prepare_ask(&mut self, upto: Seq, key: u64, isolated: bool) -> Action {
        let assembler = &self.policy.assembler;
        let request = match isolated {
            true => assembler.summarize_isolated(&self.history, upto, None, None),
            false => assembler.summarize(&self.history, upto, None, None),
        };
        let request = self.with_descriptions(request);
        let aside = Aside::new(upto, request.messages.len());
        self.prepare.state = Some(State::Asking {
            aside,
            key,
            isolated,
        });
        Action::Aside {
            purpose: Purpose::Compaction,
            upto,
            request,
        }
    }

    /// 提前压好的摘要请求说完了（第十五条第 2 条）：记 `model.called`（不带回合编号，`purpose` 是 `compaction`，不带
    /// `compaction`）；取到了摘要的放着；fork 式调了工具、策略里有隔离式、N 以前还没变的，照隔离式再发一次；别的扔掉。
    /// 不是在路上的那一次的不理。
    pub(super) fn prepare_ended(
        &mut self,
        at: Timestamp,
        upto: Seq,
        spent: (Option<Usage>, Option<Cost>),
        error: Option<CallError>,
    ) -> Vec<Action> {
        let asking =
            |state: &mut State| matches!(state, State::Asking { aside, .. } if aside.upto == upto);
        let Some(State::Asking {
            aside,
            key,
            isolated,
        }) = self.prepare.state.take_if(asking)
        else {
            return Vec::new();
        };
        let next = self.ledger.next_seq();
        let (mut called, blocks) = aside.end(at, Purpose::Compaction, spent, error, next);
        let isolates = !isolated
            && !self.restarting
            && self.policy.compaction.as_ref().is_some_and(|c| c.isolate);
        let again = isolates && blocks.as_deref().is_ok_and(called_tool);
        let summary = blocks.and_then(|blocks| self.summary_of(&blocks, isolates));
        if let Err(error) = &summary {
            failed(&mut called, error);
        }
        let (usage, duration_ms) = (called.usage, called.duration_ms);
        let event = self.record_aside(at, None, Body::ModelCalled(called));
        let mut actions = vec![Action::Append(vec![event])];
        // 回合在等它（施工 6-11 下）：用得上的换上；别的（出错、取不出、调了工具、用不上了）不再补发隔离式的，回到「准备
        // 好」，这条 `model.called` 落了盘再组装、照现在的办法当场压（请求要等事件都落了盘才发）。回到「准备好」照别处一样
        // 先查事实：等的时候切了级别的，这时注入。
        if let Some(awaiting) = self.take_awaiting() {
            let room = self.lead().map(|(tail, lead)| tail + lead);
            match summary {
                Ok(summary) if room.is_some_and(|room| self.usable(upto, key, room)) => {
                    let prepared = Prepared {
                        upto,
                        key,
                        summary,
                        usage,
                        duration_ms,
                    };
                    actions.extend(self.swap(at, awaiting.due, prepared));
                }
                _ => {
                    if let Some(turn) = self.turn.as_mut() {
                        turn.stage = Stage::Ready;
                    }
                    let facts = self.refresh_facts(at);
                    if !facts.is_empty() {
                        actions.push(Action::Append(facts));
                    }
                }
            }
            return actions;
        }
        match summary {
            Ok(summary) => {
                self.prepare.state = Some(State::Ready(Prepared {
                    upto,
                    key,
                    summary,
                    usage,
                    duration_ms,
                }));
            }
            Err(_) if again && self.prefix_key(upto) == key => {
                actions.push(self.prepare_ask(upto, key, true));
            }
            Err(_) => {}
        }
        actions
    }

    /// 到线了（第十五条第 3、4、5 条）：自动压缩、手动压缩没附要求的，手里那一份用得上就换上；在路上的那一次用得上就等它
    /// （施工 6-11 下）；不然当场压（[`Session::start_compaction`]）。压好了用不上的扔掉；在路上用不上的留着，晚到了照
    /// 「还用得上」再判。
    pub(super) fn begin_compaction(&mut self, at: Timestamp, due: Due) -> Vec<Action> {
        if let Some(prepared) = self.take_prepared(&due) {
            return self.swap(at, due, prepared);
        }
        match self.awaitable(&due) {
            Some(written) => self.await_prepared(at, due, written),
            None => self.start_compaction(due),
        }
    }

    /// 在路上的那一次等不等（施工 6-11 下）：这一轮开着、没附要求，它起压时的指纹和这时的一样、那里还切得开、以后的尾巴
    /// 不超过 T + G 的等，交回它的名字和已经收到的正文字数。
    fn awaitable(&self, due: &Due) -> Option<(Seq, u64)> {
        let Some(State::Asking { aside, key, .. }) = &self.prepare.state else {
            return None;
        };
        let (tail, lead) = self.lead()?;
        let wanted = self.prepare.on && due.instructions.is_none();
        (wanted && self.usable(aside.upto, *key, tail + lead))
            .then(|| (aside.upto, aside.written()))
    }

    /// 等在路上的那一次（施工 6-11 下）：回合进 [`Stage::Awaiting`]，推一条进度：`seen` 是它的名字，写了的是已经收到的。
    fn await_prepared(
        &mut self,
        at: Timestamp,
        due: Due,
        (seen, written): (Seq, u64),
    ) -> Vec<Action> {
        let expected = expected(due.used);
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        turn.stage = Stage::Awaiting(Box::new(Awaiting { due, expected }));
        let progress = CompactionProgress {
            seen,
            written,
            expected,
        };
        vec![Action::PushTransient(Session::progress(
            at,
            Some(turn.id),
            turn.cause.clone(),
            progress,
        ))]
    }

    /// 回合在等的那一次来了一段正文（施工 6-11 下，`aside.rs` 的 `aside_delta` 调它）：推一次进度。没在等的什么都不推。
    pub(super) fn awaited_progress(&mut self, at: Timestamp, upto: Seq) -> Vec<Action> {
        let Some(written) = self.prepare.aside(upto).map(|aside| aside.written()) else {
            return Vec::new();
        };
        let Some(turn) = self.turn.as_ref() else {
            return Vec::new();
        };
        let Stage::Awaiting(awaiting) = &turn.stage else {
            return Vec::new();
        };
        let progress = CompactionProgress {
            seen: upto,
            written,
            expected: awaiting.expected,
        };
        vec![Action::PushTransient(Session::progress(
            at,
            Some(turn.id),
            turn.cause.clone(),
            progress,
        ))]
    }

    /// 回合在等提前压的那一次的，取走等的那一格，回合先放在「收拾」上（接着换上或者当场压）。没在等的没有。
    fn take_awaiting(&mut self) -> Option<Box<Awaiting>> {
        let turn = self.turn.as_mut()?;
        if !matches!(turn.stage, Stage::Awaiting(_)) {
            return None;
        }
        match std::mem::replace(&mut turn.stage, Stage::Settling) {
            Stage::Awaiting(awaiting) => Some(awaiting),
            _ => None,
        }
    }

    /// 取走压好了的那一份：这一轮开着、没附要求、还用得上的才交回。
    fn take_prepared(&mut self, due: &Due) -> Option<Prepared> {
        let Some(State::Ready(prepared)) = self
            .prepare
            .state
            .take_if(|state| matches!(state, State::Ready(_)))
        else {
            return None;
        };
        // 被动压缩不走这里（`overflow.rs` 照它自己的切法当场压）。
        let wanted = self.prepare.on && due.instructions.is_none();
        let (tail, lead) = self.lead()?;
        (wanted && self.usable(prepared.upto, prepared.key, tail + lead)).then_some(prepared)
    }

    /// 替代到 `upto`、起压时指纹是 `key` 的那一份还用得上：有效历史里 `upto` 以前没变，那里还切得开，以后的尾巴不超过
    /// `room`（T + G）。压好了的、在路上的（施工 6-11 下）一样判。
    fn usable(&self, upto: Seq, key: u64, room: u64) -> bool {
        if self.prefix_key(upto) != key {
            return false;
        }
        let Some(price) = self.price() else {
            return false;
        };
        let ordered = self.history.ordered();
        if settle(&ordered, upto) != Some(upto) {
            return false;
        }
        let tail = ordered
            .iter()
            .filter(|event| event.seq > upto)
            .map(|event| estimate::event(event, &price))
            .fold(0, u64::saturating_add);
        tail <= room
    }

    /// 有效历史里第 `upto` 条以前的指纹：检查点的序号，和它后面第 `upto` 条及以前还算数的事件的序号。
    fn prefix_key(&self, upto: Seq) -> u64 {
        let mut hasher = DefaultHasher::new();
        let checkpoint = self
            .history
            .checkpoint()
            .map(|checkpoint| checkpoint.seq.get());
        checkpoint.hash(&mut hasher);
        for event in self.history.events() {
            if event.seq <= upto {
                event.seq.get().hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    /// 换上压好的那一份：交执行器重读（和当场压一样，候选照 N 算），回来了再写（[`Session::swapped`]）；没有候选的当场写。
    /// `trigger`、`refills`、压之前的用量照这一次的。
    fn swap(&mut self, at: Timestamp, due: Due, prepared: Prepared) -> Vec<Action> {
        let Prepared {
            upto,
            summary,
            usage,
            duration_ms,
            ..
        } = prepared;
        let paths = self.reread_paths(upto);
        let summarized = Summarized {
            upto,
            trigger: due.trigger,
            instructions: None,
            refills: due.refills,
            cut: None,
            summary,
            before: due.used,
            usage,
            duration_ms,
            paths: paths.clone(),
            reread: None,
            prepared: true,
        };
        if paths.is_empty() {
            return self.swapped(at, summarized);
        }
        let limit = self.reread_limit();
        if let Some(turn) = self.turn.as_mut() {
            turn.stage = Stage::Swapping(Box::new(summarized));
        }
        vec![Action::Reread {
            seen: upto,
            paths,
            limit,
        }]
    }

    /// 执行器送回了换上的那一份的重读：一个对一个的收下，写压缩。不在换上、名字对不上的，没有。
    pub(super) fn swap_reread(
        &mut self,
        at: Timestamp,
        seen: Seq,
        files: Vec<Reread>,
    ) -> Option<Vec<Action>> {
        let turn = self.turn.as_mut()?;
        if !matches!(&turn.stage, Stage::Swapping(summarized) if summarized.upto == seen) {
            return None;
        }
        let Stage::Swapping(mut summarized) = std::mem::replace(&mut turn.stage, Stage::Settling)
        else {
            return None;
        };
        if files.len() == summarized.paths.len() {
            summarized.reread = Some(files);
        }
        Some(self.swapped(at, *summarized))
    }

    /// 写压缩、推压好了（[`Session::compacted`]），照当场压取到了摘要的一样往下走。
    fn swapped(&mut self, at: Timestamp, summarized: Summarized) -> Vec<Action> {
        let cause = self.turn.as_ref().and_then(|turn| turn.cause.clone());
        let (events, done) = self.compacted(at, summarized, cause);
        let mut actions = vec![Action::Append(events)];
        actions.extend(done.map(Action::PushTransient));
        actions
    }
}
