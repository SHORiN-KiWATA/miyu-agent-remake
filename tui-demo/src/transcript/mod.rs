//! 界面这一头记着的会话：正文里的一条条、在不在跑、连没连上、用了多少。
//!
//! 只收核心推来的，不自己编：正文照推送一块块接起来，思考和调工具记进时间线的一段（`steps.rs`），
//! 用量照 `model.called` 加起来。纯状态，不碰终端，好测。

mod beat;
mod blocks;
mod cache;
mod climb;
mod compaction;
mod entry;
mod failure;
mod queue;
mod steps;
mod turn;
mod words;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::time::Instant;

use crate::config::Texts;
use crate::core::{CallError, EndReason, Level, Limits, Push, ToolStatus, Update, Usage};

pub use cache::CacheWatch;
pub use climb::Progress;
pub use entry::{Entry, JobMark, JobNote, Kind};
pub use steps::{Segment, Step, StepKind, Tally, ToolState};
pub use words::undo_counts;

/// 连核心的状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// 还在连。
    Connecting,
    /// 连上了。
    Ready,
    /// 断开了，正在重新连接（「连核心」第 7 条）。
    Reconnecting,
    /// 连不上：给人看的一句（第 8 条）。
    Down(String),
}

/// 一块字接到哪里去：回答那一条，或者时间线某一段的某一步。
#[derive(Debug, Clone, Copy)]
enum Slot {
    Reply(usize),
    Step(usize, usize),
}

/// 界面记着的会话。
#[derive(Debug)]
pub struct Transcript {
    /// 正文，从旧到新。
    pub entries: Vec<Entry>,
    /// 这一次请求里第几块接到哪里。每次请求的块从 0 数起，新块开头时覆盖。
    blocks: HashMap<u64, Slot>,
    /// 调了工具、还不知道调用编号的几步，照先后；`message.assistant` 来了按顺序配上。
    unnamed: Vec<(usize, usize)>,
    /// 调用编号到那一步。
    calls: HashMap<String, (usize, usize)>,
    /// 这一轮从什么时候开始跑；没在跑是 `None`。
    pub running: Option<Instant>,
    /// 最近一次说话的模型：端点和模型名。
    pub model: Option<(String, String)>,
    /// 连核心的状态。
    pub link: Link,
    /// 会话编号：连上以后才有。
    pub session: Option<String>,
    /// 会话名称：核心起了名字（`session.meta_changed`）才有。
    pub title: Option<String>,
    /// 实际的权限级别。新会话从工作区、不只读开始（`protocol.md` `session.create` 第 2 条）。
    pub level: Level,
    /// 这个会话从开到现在，每次请求的用量加起来。
    pub total: Usage,
    /// 这一轮每次请求的用量加起来：收尾行写它（`tui.md`「正文」第 4 条）。
    turn_usage: Usage,
    /// 这一轮开始时的权限级别：收尾行打头的图标照它，之后换了级别也不变。
    turn_level: Level,
    /// 最近一次请求占了多少上下文：输入加输出。
    pub context: u64,
    /// 压过几次、意外断过几次缓存（侧边栏写）。
    pub cache: CacheWatch,
    /// 会话的限额：核心在订阅的回应里给的窗口、压缩线（`core/limits.rs`）。
    pub limits: Limits,
    /// 正在压缩的那一行是第几条（`compaction.rs`）。
    compacting: Option<usize>,
    /// 这一轮她出过字了（来过一块）：出过就不再算在等第一个字（[`Transcript::waiting`]）。
    spoke: bool,
    /// 这一轮是手动压缩（`turn.started` 没有 `trigger`，施工 6-8）：压好了不另起收尾行。
    manual: bool,
    /// 最近一次请求出字的速度，每秒几个 token。
    pub speed: Option<f64>,
    /// 正在重试时给人看的一句。
    pub retry: Option<String>,
    /// 这一轮最后一次请求出的错；后来又成了的清掉。
    failure: Option<CallError>,
    /// 在跑的这一轮的编号：这期间收到的都记在它名下。
    turn: Option<u64>,
    /// 被退回的排队消息：字和里面的粘贴块，等输入框拿走（`take_returned`）。
    returned: Vec<(String, Vec<(String, String)>)>,
    /// 最近一次 `turn.reverted` 撤掉的几轮：撤销的回应来了，照它找你说的那句全文。
    reverted: Vec<u64>,
    /// 下一条正文的编号。
    next_id: u64,
}

impl Default for Transcript {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            blocks: HashMap::new(),
            unnamed: Vec::new(),
            calls: HashMap::new(),
            running: None,
            model: None,
            link: Link::Connecting,
            session: None,
            title: None,
            level: Level::Workspace,
            total: Usage::default(),
            turn_usage: Usage::default(),
            turn_level: Level::Workspace,
            context: 0,
            cache: CacheWatch::default(),
            limits: Limits::default(),
            compacting: None,
            spoke: false,
            manual: false,
            speed: None,
            retry: None,
            failure: None,
            turn: None,
            reverted: Vec::new(),
            next_id: 0,
            returned: Vec::new(),
        }
    }
}

impl Transcript {
    /// 你说了一句。
    pub fn user(&mut self, text: String, pasted: Vec<(String, String)>) {
        self.push(Kind::User, text);
        if let Some(entry) = self.entries.last_mut() {
            entry.pasted = pasted;
        }
    }

    /// 在正文末尾写一句旁白，不属于哪一轮。
    pub fn note(&mut self, kind: Kind, text: String) {
        let id = self.fresh_id();
        self.entries.push(Entry {
            id,
            kind,
            text,
            segment: None,
            turn: None,
            hidden: false,
            queued: false,
            seq: None,
            undo: None,
            open: false,
            level: None,
            job: None,
            pasted: Vec::new(),
            details: Vec::new(),
            progress: None,
            mark: None,
        });
    }

    /// 正文末尾一条后台任务结束的通知（蓝图「后台命令、子代理和侧边栏」第 5 条）。
    pub fn job(&mut self, mark: JobMark, text: String, detail: String) {
        self.note(Kind::Job, text);
        if let Some(entry) = self.entries.last_mut() {
            entry.job = Some(JobNote { mark, detail });
        }
    }

    /// 换到 `order` 里的下一档，到头回到第一档；现在的不在里面的，换到第一档。
    ///
    /// 只改界面上的样子：协议还没有改权限的方法，核心不知道（蓝图 `tui.md`「权限级别」第 2 条）。
    pub fn next_level(&mut self, order: &[Level]) {
        let at = order.iter().position(|l| *l == self.level);
        let next = at.map_or(0, |i| (i + 1) % order.len().max(1));
        if let Some(level) = order.get(next) {
            self.level = *level;
        }
    }

    /// 有没有还在进行的步骤、正在压缩：有就要转圈。
    pub fn busy(&self) -> bool {
        self.compacting.is_some()
            || self
                .entries
                .iter()
                .filter_map(|e| e.segment.as_ref())
                .any(|s| s.steps.iter().any(Step::busy))
    }

    /// 开新会话（`/new`）：正文、用量、撤销记录清掉，权限级别回到工作区；连接、模型、窗口照旧。条目编号接着往上数：
    /// 按条缓存排好的行认编号，不能重用。
    pub fn fresh(&mut self) {
        let old = std::mem::take(self);
        self.link = old.link;
        self.model = old.model;
        self.limits = old.limits;
        self.next_id = old.next_id;
    }

    /// 收一条核心那边的消息。
    pub fn update(&mut self, update: Update, texts: &Texts) {
        match update {
            Update::Limits(limits) => self.limits = limits,
            Update::Ready(session) => {
                self.link = Link::Ready;
                self.session = Some(session);
            }
            Update::NoCoreBin => self.link = Link::Down(texts.no_core_bin.clone()),
            Update::Missing(path) => {
                self.link = Link::Down(texts.missing_core.replace("{path}", &path));
            }
            Update::Reconnected => self.link = Link::Ready,
            Update::Failed(reason) => {
                self.link = Link::Down(texts.core_failed.replace("{reason}", &reason));
            }
            // 核心断开了：在进行的那一轮当场收尾，马上重连（「连核心」第 7 条）。
            Update::Disconnected => {
                self.cut_off(texts);
                self.link = Link::Reconnecting;
            }
            // 认得的原因码写一句短话，认不得的照核心的原话（`tui.md`「正文」第 6 条）。
            Update::Refused { reason, message } => {
                let short = reason.as_deref().and_then(|r| texts.refusals.get(r));
                let text = short
                    .cloned()
                    .unwrap_or_else(|| texts.refused.replace("{reason}", &message));
                self.note(Kind::Error, text);
            }
            Update::Push(push) => self.apply(push, texts),
            // 撤销：记一行说明，全文照这一次撤掉的第一轮里你说的话，没有的照核心给的第一行。
            Update::Undone {
                restore: false,
                report,
            } => {
                let said = self
                    .entries
                    .iter()
                    .find(|e| {
                        e.kind == Kind::User && e.turn.is_some_and(|t| self.reverted.contains(&t))
                    })
                    .map(|e| e.text.clone())
                    .or_else(|| report.said.clone())
                    .unwrap_or_default();
                self.note(Kind::Undo, said);
                if let Some(last) = self.entries.last_mut() {
                    last.undo = Some(report);
                }
            }
            // 恢复：那几轮已经照 `turn.unreverted` 显示回来了，去掉最近的那一行撤销说明，不另写一句。
            Update::Undone { restore: true, .. } => {
                if let Some(i) = self.entries.iter().rposition(|e| e.kind == Kind::Undo) {
                    self.entries.remove(i);
                }
            }
        }
    }

    fn apply(&mut self, push: Push, texts: &Texts) {
        match push {
            Push::TurnStarted(turn, trigger) => self.start(turn, trigger),
            // 你发的话照先后落盘：配给最早那句还没有序号的。
            Push::UserMessage(seq) => {
                let first = self
                    .entries
                    .iter_mut()
                    .find(|e| e.kind == Kind::User && e.seq.is_none());
                if let Some(entry) = first {
                    entry.seq = Some(seq);
                }
            }
            Push::Withdrawn(seqs) => {
                let back = |e: &Entry| e.seq.is_some_and(|s| seqs.contains(&s));
                self.returned.extend(
                    self.entries
                        .iter()
                        .filter(|e| back(e))
                        .map(|e| (e.text.clone(), e.pasted.clone())),
                );
                self.entries.retain(|e| !back(e));
            }
            Push::Title(title) => self.title = Some(title),
            Push::Policy { level, read_only } => {
                self.level = if read_only { Level::ReadOnly } else { level };
            }
            Push::Reverted(turns) => {
                self.cache.reverted();
                self.hide(&turns, true);
                self.reverted = turns;
            }
            Push::Unreverted(turns) => self.hide(&turns, false),
            Push::Model { endpoint, model } => self.model = Some((model, endpoint)),
            Push::Heard(seen) => self.heard(seen),
            Push::BlockStart { index, block } => {
                self.spoke = true;
                self.retry = None;
                self.close_open_blocks();
                let slot = self.open_block(block);
                self.blocks.insert(index, slot);
            }
            Push::Delta { index, text } => self.delta(index, &text),
            Push::BlockEnd(index) => self.block_end(index),
            Push::Calls(ids) => {
                for (id, at) in ids.into_iter().zip(std::mem::take(&mut self.unnamed)) {
                    if let Some(step) = self.step_mut(at) {
                        step.call_id = Some(id.clone());
                    }
                    self.calls.insert(id, at);
                }
            }
            Push::ToolResult {
                call_id,
                status,
                text,
                said: result_said,
                ..
            } => {
                let at = self.calls.get(&call_id).copied();
                if let Some(step) = at.and_then(|at| self.step_mut(at)) {
                    step.stop();
                    if let StepKind::Tool {
                        state,
                        output,
                        said,
                        ..
                    } = &mut step.kind
                    {
                        *state = ToolState::Done(status);
                        *output = text;
                        *said = result_said;
                    }
                }
            }
            Push::Usage(usage) => {
                self.total.uncached += usage.uncached;
                self.total.cache_read += usage.cache_read;
                self.total.cache_write += usage.cache_write;
                self.total.output += usage.output;
                self.turn_usage.uncached += usage.uncached;
                self.turn_usage.cache_read += usage.cache_read;
                self.turn_usage.cache_write += usage.cache_write;
                self.turn_usage.output += usage.output;
                self.context = usage.input() + usage.output;
            }
            Push::Sent {
                seen,
                changed,
                summary,
            } => self.cache.sent(seen, changed, summary),
            Push::Compaction(push) => self.compaction(push, texts),
            Push::Compacted => self.cache.compacted(),
            Push::Speed { output, ms } => {
                self.speed = Some(output as f64 * 1000.0 / ms as f64);
            }
            Push::CallFailed(error) => self.failure = Some(error),
            // 后来又成了：前面报过的错不算（蓝图「正文」第 4 条）。
            Push::CallOk => self.failure = None,
            Push::Retry {
                attempt,
                limit,
                message,
            } => {
                self.retry = Some(
                    texts
                        .retry
                        .replace("{attempt}", &attempt.to_string())
                        .replace("{limit}", &limit.to_string())
                        .replace("{message}", &message),
                );
            }
            Push::TurnEnded(reason) => self.end(reason, texts),
        }
    }

    /// 一共收起了几段（做完的时间线）：变多了就是刚收起一段，界面照它放开一次视口（蓝图「正文」第 1 条）。
    pub fn folds(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.segment.as_ref().is_some_and(|s| s.finished))
            .count()
    }

    /// 拿走被退回的排队消息的字，照先后。
    pub fn take_returned(&mut self) -> Vec<(String, Vec<(String, String)>)> {
        std::mem::take(&mut self.returned)
    }

    fn hide(&mut self, turns: &[u64], hidden: bool) {
        for entry in &mut self.entries {
            if entry.turn.is_some_and(|t| turns.contains(&t)) {
                entry.hidden = hidden;
            }
        }
    }

    /// 记一条。你说的话等开轮时再归；别的归到在跑的这一轮。
    fn push(&mut self, kind: Kind, text: String) {
        let user = kind == Kind::User;
        let queued = user && self.running.is_some();
        let turn = if user { None } else { self.turn };
        // 你说的话记发出去时的级别（竖线的颜色）；收尾行记这一轮开始时的（打头的图标）。
        let level = match kind {
            Kind::User => Some(self.level),
            Kind::Done | Kind::Cut => Some(self.turn_level),
            _ => None,
        };
        let id = self.fresh_id();
        self.entries.push(Entry {
            id,
            kind,
            text,
            segment: None,
            turn,
            hidden: false,
            queued,
            seq: None,
            undo: None,
            open: false,
            level,
            job: None,
            pasted: Vec::new(),
            details: Vec::new(),
            progress: None,
            mark: None,
        });
    }

    /// 发一个新编号。
    fn fresh_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }
}
