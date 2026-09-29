//! 界面这一头记着的会话：正文里的一条条、在不在跑、连没连上、用了多少。
//!
//! 只收核心推来的，不自己编：正文照推送一块块接起来，思考和调工具记进时间线的一段（`steps.rs`），
//! 用量照 `model.called` 加起来。纯状态，不碰终端，好测。

mod beat;
mod blocks;
mod steps;
mod turn;
mod words;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::time::Instant;

use crate::config::Texts;
use crate::core::{EndReason, Level, Push, Report, ToolStatus, Update, Usage};

pub use steps::{Segment, Step, StepKind, Tally, ToolState};
pub use words::undo_counts;

/// 正文里一条的种类，决定怎么画。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// 你说的话。
    User,
    /// 她的回答。
    Reply,
    /// 时间线的一段：思考、调工具。
    Steps,
    /// 打断了、撤销了这类旁白。
    Note,
    /// 出错了。
    Error,
    /// 一轮做完的收尾行：模型、用时、做完的时刻。
    Done,
    /// 撤销了几轮：一行说明，点开看你说的那句的全文（`text`）。
    Undo,
    /// 一条后台任务结束的通知（`job` 里是记号和能点开看的全文）。
    Job,
}

/// 后台任务结束的通知：成败决定记号的颜色（蓝图「后台命令、子代理和侧边栏」第 5 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobMark {
    /// 做完了：绿 `✓`。
    Done,
    /// 失败了：红 `✗`。
    Failed,
    /// 停掉了：没有记号，整行暗。
    Stopped,
}

/// 通知里的记号，和点开看的全文（子代理的报告；没有的是空的）。
#[derive(Debug, Clone)]
pub struct JobNote {
    /// 记号。
    pub mark: JobMark,
    /// 点开看的全文。
    pub detail: String,
}

/// 正文里的一条。
#[derive(Debug, Clone)]
pub struct Entry {
    /// 种类。
    pub kind: Kind,
    /// 字；时间线那一条没有字。
    pub text: String,
    /// 时间线那一条的一段；别的是 `None`。
    pub segment: Option<Segment>,
    /// 属于哪一轮；你刚说、还没开轮的，和撤销说明这类不属于哪一轮的，是 `None`。
    pub turn: Option<u64>,
    /// 这一轮被撤掉了：不画，恢复时再画。
    pub hidden: bool,
    /// 你说的话发出去时正在回答：先排着，列在运行状态行下面；开了它那一轮才进正文（`tui.md`「运行状态行和排队的消息」第 5 条）。
    pub queued: bool,
    /// 你说的话落盘以后的序号（`message.user`）：认它开了哪一轮、被退回的是哪一句。别的条是 `None`。
    pub seq: Option<u64>,
    /// 撤销那一行：核心回应里给人看的几样。别的条是 `None`。
    pub undo: Option<Report>,
    /// 人点开了：撤销那一行下面铺底色写全文。
    pub open: bool,
    /// 你说的话发出去那一刻的权限级别：竖线照它上色，之后换了级别也不变。别的条是 `None`。
    pub level: Option<Level>,
    /// 后台任务结束的通知：记号和全文。别的条是 `None`。
    pub job: Option<JobNote>,
}

/// 连核心的状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// 还在连。
    Connecting,
    /// 连上了。
    Ready,
    /// 连不上、断开了：给人看的一句。
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
    /// 最近一次请求出字的速度，每秒几个 token。
    pub speed: Option<f64>,
    /// 正在重试时给人看的一句。
    pub retry: Option<String>,
    /// 这一轮最后一次请求的出错：分类和原话。
    failure: Option<(String, String)>,
    /// 在跑的这一轮的编号：这期间收到的都记在它名下。
    turn: Option<u64>,
    /// 被退回的排队消息的字，等输入框拿走（`take_returned`）。
    returned: Vec<String>,
    /// 最近一次 `turn.reverted` 撤掉的几轮：撤销的回应来了，照它找你说的那句全文。
    reverted: Vec<u64>,
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
            speed: None,
            retry: None,
            failure: None,
            turn: None,
            reverted: Vec::new(),
            returned: Vec::new(),
        }
    }
}

impl Transcript {
    /// 你说了一句。
    pub fn user(&mut self, text: String) {
        self.push(Kind::User, text);
    }

    /// 在正文末尾写一句旁白，不属于哪一轮。
    pub fn note(&mut self, kind: Kind, text: String) {
        self.entries.push(Entry {
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

    /// 有没有还在进行的步骤：有就要转圈。
    pub fn busy(&self) -> bool {
        self.entries
            .iter()
            .filter_map(|e| e.segment.as_ref())
            .any(|s| s.steps.iter().any(Step::busy))
    }

    /// 收一条核心那边的消息。
    pub fn update(&mut self, update: Update, texts: &Texts) {
        match update {
            Update::Ready(session) => {
                self.link = Link::Ready;
                self.session = Some(session);
            }
            Update::NoCoreBin => self.link = Link::Down(texts.no_core_bin.clone()),
            Update::Failed(reason) => {
                self.link = Link::Down(texts.core_failed.replace("{reason}", &reason));
            }
            Update::Disconnected => {
                self.link = Link::Down(texts.disconnected.clone());
                self.running = None;
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
                redo: false,
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
            Update::Undone { redo: true, .. } => {
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
                        .map(|e| e.text.clone()),
                );
                self.entries.retain(|e| !back(e));
            }
            Push::Title(title) => self.title = Some(title),
            Push::Policy { level, read_only } => {
                self.level = if read_only { Level::ReadOnly } else { level };
            }
            Push::Reverted(turns) => {
                self.hide(&turns, true);
                self.reverted = turns;
            }
            Push::Unreverted(turns) => self.hide(&turns, false),
            Push::Model { endpoint, model } => self.model = Some((model, endpoint)),
            Push::BlockStart { index, block } => {
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
            Push::Speed { output, ms } => {
                self.speed = Some(output as f64 * 1000.0 / ms as f64);
            }
            Push::CallFailed { class, message } => self.failure = Some((class, message)),
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
    pub fn take_returned(&mut self) -> Vec<String> {
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
            Kind::Done => Some(self.turn_level),
            _ => None,
        };
        self.entries.push(Entry {
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
        });
    }
}
