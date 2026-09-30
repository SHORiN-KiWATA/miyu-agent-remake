//! 日志的账本：追加一条事件之前，照规矩查一遍（`docs/designs/02-内核.md` 第九节
//! 「日志追加时查的规矩」）。
//!
//! 账本只记查规矩要用的几样，不留事件本身（`07-存储.md` 第七节）。撤销能撤掉压缩（施工 6-9），压缩以前还没撤掉的
//! 回合也记着；派出去过的任务也一个一项记着，编号不回收（施工 7-1）：账本只随回合数、任务数长，不随日志的字节长
//! （`02-内核.md` 第九节，2026-09-29 项目主人定）。
//! 新写的事件和从磁盘载入的事件都从这里过，规矩只有一套。
//!
//! 撤销、恢复的几条在 `ledger/undo.rs`，派出去的任务和子会话的几条在 `ledger/jobs.rs`（施工 7-1）。

use std::collections::BTreeSet;
use std::fmt;

use crate::block::Block;
use crate::event::{Body, CompactTrigger, ContextCompacted, Event};
use crate::id::{CallId, CommandId, JobId, Seq, SessionId, TurnId};

mod jobs;
mod undo;

use jobs::Jobs;
use undo::Undone;

/// 一个会话的日志的账本。
///
/// 每追加一条事件，先交给 [`Ledger::append`] 查；违反规矩的不追加，账本也不变。
/// 空的账本（[`Ledger::default`]）是一个还没有任何事件的会话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ledger {
    /// 下一条事件该是几号。
    next: Seq,
    /// 正在进行的回合。一个会话同一时刻最多只有一个（不变量 4）。
    open: Option<TurnId>,
    /// 上一条回复（`message.assistant`）的序号。后一次请求一定看过它，
    /// 所以下一条回复的 `seen` 不能比它早。
    last_reply: Option<Seq>,
    /// 正在进行的回合里，还没有结果的调用。回合结束时它必须是空的，
    /// 所以这里只会有这一个回合的调用。
    pending: BTreeSet<CallId>,
    /// 其中在等确认的：请人确认了，还没有决定，也还没有结果（`02-内核.md` 第六节
    /// 「确认怎么走」）。
    asking: BTreeSet<CallId>,
    /// 其中在等人回答的：问了一组题，还没有回答，也还没有结果（「提问怎么走」）。
    questioning: BTreeSet<CallId>,
    /// 还算数的几次压缩，照先后：它所在的那一轮还没撤掉（`compaction.md` 第十一条，施工 6-9）。
    compactions: Vec<Compaction>,
    /// 开过、还没撤掉的回合，照先后，压缩以前的也在：撤销能撤到压缩以前（施工 6-9）。撤销拿走后面的一截，恢复原样
    /// 放回，开一轮接在最后，所以一直是排好的。
    turns: Vec<TurnId>,
    /// 还能恢复的几次撤销，最近的一次在最后。下一轮开始、压缩了，就都不能恢复了（`02-内核.md` 第六节「撤销与恢复」）。
    undone: Vec<Undone>,
    /// 正在进行的回合里排着队的消息：回合中途来的 `message.user`，还没被哪次请求看到过。
    /// 只有它们能撤回（`02-内核.md` 第六节「排队的消息」）。请求看到了、回合结束了，就清掉。
    queued: BTreeSet<Seq>,
    /// 派出去过的任务，撤掉的回合里派的也在：编号不回收（施工 7-1）。
    jobs: Jobs,
}

impl Default for Ledger {
    fn default() -> Self {
        Ledger {
            next: Seq::FIRST,
            open: None,
            last_reply: None,
            pending: BTreeSet::new(),
            asking: BTreeSet::new(),
            questioning: BTreeSet::new(),
            compactions: Vec::new(),
            turns: Vec::new(),
            undone: Vec::new(),
            queued: BTreeSet::new(),
            jobs: Jobs::default(),
        }
    }
}

impl Ledger {
    /// 下一条事件该是几号。追加的一方照这个号给事件编序号。
    pub fn next_seq(&self) -> Seq {
        self.next
    }

    /// 正在进行的回合；没有就是空闲。载入时看它，日志停在一个没结束的回合里，就是崩了。
    pub fn open_turn(&self) -> Option<TurnId> {
        self.open
    }

    /// 正在进行的回合里还没有结果的调用，照编号的先后。
    pub fn pending_calls(&self) -> Vec<CallId> {
        self.pending.iter().copied().collect()
    }

    /// 正在进行的回合里排着队的消息，照先后。
    pub fn queued(&self) -> Vec<Seq> {
        self.queued.iter().copied().collect()
    }

    /// 还没撤掉的回合 `turn`，和它以后还没撤掉的每一轮，照先后，压缩以前的也算：从它起撤销，撤的就是这些。
    /// `turn` 撤掉了、不是一轮的开头的，没有。
    pub fn turns_from(&self, turn: TurnId) -> Option<Vec<TurnId>> {
        self.turns
            .binary_search(&turn)
            .ok()
            .map(|k| self.turns[k..].to_vec())
    }

    /// 还没撤掉的最后一轮：不写回合编号的撤销撤它（施工 4-7 下）。一轮都没有的，没有。
    pub fn last_turn(&self) -> Option<TurnId> {
        self.turns.last().copied()
    }

    /// 还算数的最近一次压缩替代到哪；没有还算数的压缩就没有。
    pub fn compacted(&self) -> Option<Seq> {
        self.compactions.last().map(|compaction| compaction.upto)
    }

    /// 从还没撤掉的回合 `turn` 起撤，会撤掉还算数的压缩（它所在的那一轮不早于 `turn`）的：要从第几条读回日志
    /// （施工 6-9，`kernel/history.md`「撤掉压缩」）。是撤完以后还算数的最近一次压缩替代到的下一条，一次都没有的
    /// 是第 1 条。撤不到压缩的，没有：照以前在内存里撤。
    pub fn read_back_from(&self, turn: TurnId) -> Option<Seq> {
        let kept = self
            .compactions
            .partition_point(|compaction| compaction.turn < turn);
        if kept == self.compactions.len() {
            return None;
        }
        Some(match kept.checked_sub(1) {
            Some(k) => self.compactions[k].upto.next(),
            None => Seq::FIRST,
        })
    }

    /// 最近一次还能恢复的撤销，撤了哪几轮；没有能恢复的就没有。
    pub fn last_reverted(&self) -> Option<&[TurnId]> {
        self.undone.last().map(|undone| undone.turns.as_slice())
    }

    /// 日志里用过的任务编号最后一段最大的数（施工 7-5；照最后一段数，施工 7-1 补）：撤掉的回合里派的也算，一个都没派过的
    /// 是 0。编号不回收，执行器新派的任务从它的下一个数起（`kernel/ids.md`「任务编号」）。
    pub fn last_job_number(&self) -> u64 {
        self.jobs.last()
    }

    /// 还没报过结束的后台命令，照编号（施工 7-3：载入时给它们补 `aborted`）。
    pub fn running_commands(&self) -> Vec<JobId> {
        self.jobs.running_commands()
    }

    /// 还在跑的任务，照编号（施工 7-8）：还没报过结束的后台命令，欠着一份回报、没被停掉的子代理。撤掉的回合里派的也在。
    pub fn running_jobs(&self) -> Vec<JobId> {
        self.jobs.running()
    }

    /// 派出去、一次都还没回报过的子代理的子会话，照任务编号（施工 7-6）。
    pub fn waiting_children(&self) -> impl Iterator<Item = &SessionId> {
        self.jobs.waiting()
    }

    /// 在会话 `session` 里跑的、这个会话派的子代理的编号（施工 7-7）：被停掉的、撤掉的回合里派的也认；别的会话没有。
    pub fn subagent_in(&self, session: &SessionId) -> Option<JobId> {
        self.jobs.agent_in(session)
    }

    /// 派出去过的子代理，照编号（施工 7-7）：编号、子会话、被停掉了没有。撤掉的回合里派的也在。
    pub fn subagents(&self) -> impl Iterator<Item = (JobId, &SessionId, bool)> {
        self.jobs.agents()
    }

    /// 子代理 `job` 最近一次回报就是命令 `id` 交来的：交回那一条的序号（施工 7-6）。
    pub fn reported_as(&self, job: &JobId, id: &CommandId) -> Option<Seq> {
        self.jobs.reported_as(job, id)
    }

    /// 查 `event` 能不能追加；能，就记下它带来的变化。
    ///
    /// # Errors
    ///
    /// 违反了追加的规矩（`02-内核.md` 第九节），返回 [`LedgerError`]，写明是第几条、
    /// 违反了哪一条。这时账本不变。
    pub fn append(&mut self, event: &Event) -> Result<(), LedgerError> {
        self.check(event).map_err(|why| LedgerError {
            seq: event.seq,
            why,
        })?;
        self.record(event);
        Ok(())
    }

    /// 照规矩查，只读不改。返回的是违反了哪一条。
    fn check(&self, event: &Event) -> Result<(), String> {
        let seq = event.seq;
        if seq != self.next {
            return Err(format!("seq should be {}", self.next));
        }
        let created = matches!(event.body, Body::SessionCreated(_));
        if seq == Seq::FIRST && !created {
            return Err("the first event should be session.created".to_string());
        }
        if seq != Seq::FIRST && created {
            return Err("session.created can only be the first event".to_string());
        }
        self.check_turn(event)?;
        match &event.body {
            Body::SessionCreated(created) => jobs::check_created(created),
            Body::MessageAssistant(message) => {
                self.check_seen(seq, message.seen)?;
                check_call_ids(seq, &message.blocks)
            }
            Body::ToolResult(result) => {
                self.check_pending(result.call_id)?;
                self.jobs.check_started(&result.effects)
            }
            Body::ApprovalRequested(requested) => {
                self.check_pending(requested.call_id)?;
                match self.asking.contains(&requested.call_id) {
                    true => Err(format!(
                        "{} already has a pending approval request",
                        requested.call_id
                    )),
                    false => Ok(()),
                }
            }
            Body::ApprovalDecided(decided) if !self.asking.contains(&decided.call_id) => {
                Err(format!(
                    "{} is not waiting for approval: never asked, already decided, or it already has a result",
                    decided.call_id
                ))
            }
            Body::QuestionAsked(asked) => {
                self.check_pending(asked.call_id)?;
                match self.questioning.contains(&asked.call_id) {
                    true => Err(format!("{} already has pending questions", asked.call_id)),
                    false => Ok(()),
                }
            }
            Body::QuestionAnswered(answered) if !self.questioning.contains(&answered.call_id) => {
                Err(format!(
                    "{} is not waiting for answers: never asked, already answered, or it already has a result",
                    answered.call_id
                ))
            }
            Body::TurnEnded(_) => match self.pending.first() {
                Some(call) => Err(format!("call {call} has no result when the turn ends")),
                None => Ok(()),
            },
            Body::ContextCompacted(compacted) => self.check_compaction(seq, compacted),
            Body::ModelCalled(called) if called.seen >= seq => Err(format!(
                "seen {} should come before this event",
                called.seen
            )),
            Body::MessageWithdrawn(withdrawn) => self.check_withdrawal(&withdrawn.messages),
            Body::TurnReverted(reverted) => self.check_revert(&reverted.turns),
            Body::TurnUnreverted(unreverted) => self.check_unrevert(&unreverted.turns),
            Body::FilesRestored(_) => match self.open {
                Some(open) => Err(format!(
                    "turn {open} is still running; files are restored only after an undo or a redo"
                )),
                None => Ok(()),
            },
            Body::JobReported(reported) => self.jobs.check_reported(reported),
            Body::ChildReported(reported) => self.jobs.check_child(reported, &event.by),
            _ => Ok(()),
        }
    }

    /// 回合的几条：回合开始时没有别的回合在进行；带 `turn` 的事件属于正在进行的回合；
    /// 只在回合里发生的种类必须带 `turn`。
    fn check_turn(&self, event: &Event) -> Result<(), String> {
        if let Body::TurnStarted(started) = &event.body {
            if event.turn != Some(TurnId::new(event.seq)) {
                return Err("turn.started should have its own seq as turn".to_string());
            }
            if let Some(open) = self.open {
                return Err(format!("turn {open} has not ended"));
            }
            // 手动压缩单开的那一轮没有触发（施工 6-8），不查。
            if started.trigger.is_some_and(|trigger| trigger >= event.seq) {
                return Err("trigger should be an event before the turn started".to_string());
            }
            return Ok(());
        }
        match event.turn {
            Some(turn) if Some(turn) != self.open => {
                Err(format!("turn {turn} is not the running turn"))
            }
            None if in_turn_only(&event.body) => Err(format!(
                "{} happens only in a turn and needs turn",
                event.body.kind()
            )),
            _ => Ok(()),
        }
    }

    /// 这个调用还在等结果。
    fn check_pending(&self, call: CallId) -> Result<(), String> {
        match self.pending.contains(&call) {
            true => Ok(()),
            false => Err(format!(
                "{call} is not a call waiting for a result: no such call, or it already has a result"
            )),
        }
    }

    /// 回复看到的在它自己之前，而且不早于上一条回复：后一次请求一定看过前一条回复
    /// （03 第六节）。所以 `seen` 一次比一次大，投影才切得了段。
    fn check_seen(&self, seq: Seq, seen: Seq) -> Result<(), String> {
        if seen >= seq {
            return Err(format!("seen {seen} should come before this reply"));
        }
        match self.last_reply {
            Some(last) if seen < last => Err(format!(
                "seen {seen} is before the previous reply {last}: a later request always sees the earlier reply"
            )),
            _ => Ok(()),
        }
    }

    /// 撤回的都是正在进行的回合里排着队的消息，一条不重复：听到过的撤了，发出去过的请求
    /// 前缀就断。
    fn check_withdrawal(&self, messages: &[Seq]) -> Result<(), String> {
        if messages.is_empty() {
            return Err("the list of withdrawn messages is empty".to_string());
        }
        let mut seen = BTreeSet::new();
        for message in messages {
            if !self.queued.contains(message) || !seen.insert(*message) {
                return Err(format!(
                    "event {message} is not a queued message of the running turn: not a message, already seen by a request, not in this turn, or already withdrawn"
                ));
            }
        }
        Ok(())
    }

    /// 压缩只前进：替代到的位置在这一条之前，而且不早于还算数的最近一次。撤掉的压缩不算：撤掉以后再压，可以比它早。
    /// 摘要是空的只许清空（施工 6-8 补）：别的压缩取不到摘要算失败，写不成检查点。
    fn check_compaction(&self, seq: Seq, compacted: &ContextCompacted) -> Result<(), String> {
        let upto = compacted.upto;
        if upto >= seq {
            return Err(format!("upto {upto} should come before this event"));
        }
        if let Some(last) = self.compacted()
            && upto < last
        {
            return Err(format!(
                "upto {upto} is before the last compaction's {last}; compaction only moves forward"
            ));
        }
        match compacted.summary.is_empty() && compacted.trigger != Some(CompactTrigger::Clear) {
            true => Err("the summary is empty; only a clear has an empty summary".to_string()),
            false => Ok(()),
        }
    }

    /// 记下查过的这一条带来的变化。
    fn record(&mut self, event: &Event) {
        self.next = event.seq.next();
        self.jobs.record(event);
        match &event.body {
            Body::TurnStarted(_) => {
                let turn = TurnId::new(event.seq);
                self.open = Some(turn);
                self.turns.push(turn);
                self.undone.clear();
                self.queued.clear();
            }
            Body::MessageUser(_) if event.turn.is_some() && event.turn == self.open => {
                self.queued.insert(event.seq);
            }
            Body::MessageWithdrawn(withdrawn) => {
                for message in &withdrawn.messages {
                    self.queued.remove(message);
                }
            }
            Body::ModelCalled(called) => self.queued.retain(|queued| *queued > called.seen),
            Body::MessageAssistant(message) => {
                self.last_reply = Some(event.seq);
                self.queued.retain(|queued| *queued > message.seen);
                self.pending
                    .extend(message.blocks.iter().filter_map(tool_call_id));
            }
            Body::ToolResult(result) => {
                self.pending.remove(&result.call_id);
                self.asking.remove(&result.call_id);
                self.questioning.remove(&result.call_id);
            }
            Body::QuestionAsked(asked) => {
                self.questioning.insert(asked.call_id);
            }
            Body::QuestionAnswered(answered) => {
                self.questioning.remove(&answered.call_id);
            }
            Body::ApprovalRequested(requested) => {
                self.asking.insert(requested.call_id);
            }
            Body::ApprovalDecided(decided) => {
                self.asking.remove(&decided.call_id);
            }
            Body::TurnEnded(_) => {
                self.open = None;
                self.queued.clear();
            }
            // 查过了：压缩带着正在进行的回合。
            Body::ContextCompacted(compacted) => {
                if let Some(turn) = event.turn {
                    self.compactions.push(Compaction {
                        turn,
                        upto: compacted.upto,
                    });
                }
                self.undone.clear();
            }
            Body::TurnReverted(reverted) => self.record_revert(&reverted.turns),
            Body::TurnUnreverted(_) => self.record_unrevert(),
            _ => {}
        }
    }
}

/// 只在回合里发生的种类：模型的回复、工具的结果、请人确认和人的决定、问人和人的回答、
/// 撤回排着队的消息、回合结束、暂停自动压缩、压缩。压缩跟着它所在的回合撤（施工 6-9）：不在回合里的压缩没有哪一轮撤得掉，
/// 它替代掉的回合也就撤不了。
fn in_turn_only(body: &Body) -> bool {
    matches!(
        body,
        Body::MessageAssistant(_)
            | Body::ToolResult(_)
            | Body::ApprovalRequested(_)
            | Body::ApprovalDecided(_)
            | Body::QuestionAsked(_)
            | Body::QuestionAnswered(_)
            | Body::MessageWithdrawn(_)
            | Body::TurnEnded(_)
            | Body::CompactionPaused(_)
            | Body::ContextCompacted(_)
    )
}

/// 一次还算数的压缩：在哪一轮、替代到哪。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Compaction {
    turn: TurnId,
    upto: Seq,
}

/// 块是工具调用的话，它的调用编号。
fn tool_call_id(block: &Block) -> Option<CallId> {
    match block {
        Block::ToolCall(call) => Some(call.call_id),
        _ => None,
    }
}

/// 助手消息里第 k 个工具调用，编号是 `call_<这一条的序号>_<k>`（`03-事件模型.md` 第二节）。
fn check_call_ids(seq: Seq, blocks: &[Block]) -> Result<(), String> {
    for (index, call) in (1..).zip(blocks.iter().filter_map(tool_call_id)) {
        if CallId::new(seq, index) != Some(call) {
            return Err(format!(
                "tool call {index} should have id call_{seq}_{index}, got {call}"
            ));
        }
    }
    Ok(())
}

/// 一条事件违反了追加的规矩：它是第几条，违反了哪一条。
///
/// 报错是中文，给查问题的人看。违反规矩只会是内核自己的 bug，或者坏了的日志文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerError {
    /// 那条事件写着的序号。
    pub seq: Seq,
    /// 违反了哪一条规矩。
    pub why: String,
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "event {} cannot be appended: {}", self.seq, self.why)
    }
}

impl std::error::Error for LedgerError {}

#[cfg(test)]
mod tests;
