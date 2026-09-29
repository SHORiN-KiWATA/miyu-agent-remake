//! 日志的账本：追加一条事件之前，照规矩查一遍（`docs/designs/02-内核.md` 第九节
//! 「日志追加时查的规矩」）。
//!
//! 账本只记查规矩要用的几样，不留事件本身，所以不随日志变长（`07-存储.md` 第七节）。
//! 新写的事件和从磁盘载入的事件都从这里过，规矩只有一套。

use std::collections::BTreeSet;
use std::fmt;

use crate::block::Block;
use crate::event::{Body, Event};
use crate::id::{CallId, Seq, TurnId};

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
    /// 最近一次压缩替代到哪。
    compacted: Option<Seq>,
    /// 最近一次压缩以后开过、还没撤掉的回合，撤销只能撤它们。压缩一次，更早的就丢掉；撤掉的
    /// 拿走，恢复了再放回来。
    turns: BTreeSet<TurnId>,
    /// 还能恢复的几次撤销，各撤了哪几轮，最近的一次在最后。下一轮开始、压缩了，就都不能恢复了
    /// （`02-内核.md` 第六节「撤销与恢复」）。
    undone: Vec<Vec<TurnId>>,
    /// 正在进行的回合里排着队的消息：回合中途来的 `message.user`，还没被哪次请求看到过。
    /// 只有它们能撤回（`02-内核.md` 第六节「排队的消息」）。请求看到了、回合结束了，就清掉。
    queued: BTreeSet<Seq>,
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
            compacted: None,
            turns: BTreeSet::new(),
            undone: Vec::new(),
            queued: BTreeSet::new(),
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

    /// 还在有效历史里的回合 `turn`，和它以后还在的每一轮，照先后：从它起撤销，撤的就是这些。
    /// `turn` 不在有效历史里的，没有。
    pub fn turns_from(&self, turn: TurnId) -> Option<Vec<TurnId>> {
        self.turns
            .contains(&turn)
            .then(|| self.turns.range(turn..).copied().collect())
    }

    /// 还在有效历史里的最后一轮：不写回合编号的撤销撤它（施工 4-7 下）。一轮都没有的，没有。
    pub fn last_turn(&self) -> Option<TurnId> {
        self.turns.last().copied()
    }

    /// 最近一次压缩替代到哪；没压缩过就没有。
    pub fn compacted(&self) -> Option<Seq> {
        self.compacted
    }

    /// 最近一次还能恢复的撤销，撤了哪几轮；没有能恢复的就没有。
    pub fn last_reverted(&self) -> Option<&[TurnId]> {
        self.undone.last().map(Vec::as_slice)
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
            Body::MessageAssistant(message) => {
                self.check_seen(seq, message.seen)?;
                check_call_ids(seq, &message.blocks)
            }
            Body::ToolResult(result) => self.check_pending(result.call_id),
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
            Body::ContextCompacted(compacted) => self.check_compaction(seq, compacted.upto),
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
            if started.trigger >= event.seq {
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

    /// 撤销：没有回合在进行；撤的是还在有效历史里的某一轮，和它以后还在的每一轮，照先后，一轮
    /// 不漏（`02-内核.md` 第六节「撤销与恢复」）。中间的一轮不能单独撤：后面几轮都是看着它做的。
    fn check_revert(&self, turns: &[TurnId]) -> Result<(), String> {
        if let Some(open) = self.open {
            return Err(format!(
                "turn {open} is still running; nothing can be undone"
            ));
        }
        let Some(&first) = turns.first() else {
            return Err("the list of undone turns is empty".to_string());
        };
        if let Some(turn) = turns.iter().find(|turn| !self.turns.contains(turn)) {
            return Err(format!(
                "turn {turn} is not in the current history: no such turn, before the last compaction, or already undone"
            ));
        }
        let expected: Vec<TurnId> = self.turns.range(first..).copied().collect();
        match turns == expected.as_slice() {
            true => Ok(()),
            false => Err(format!(
                "undo every turn from {first} on, in order: {}",
                listed(&expected)
            )),
        }
    }

    /// 恢复：正好是最近一次撤销的那几轮；那以后没开过回合，也没压缩过。
    fn check_unrevert(&self, turns: &[TurnId]) -> Result<(), String> {
        match self.undone.last() {
            None => Err(
                "nothing to redo: no undo yet, or a turn or a compaction came after it".to_string(),
            ),
            Some(last) if last.as_slice() != turns => Err(format!(
                "redo the turns of the latest undo: {}",
                listed(last)
            )),
            Some(_) => Ok(()),
        }
    }

    /// 压缩只前进：替代到的位置在这一条之前，而且不早于上一次。
    fn check_compaction(&self, seq: Seq, upto: Seq) -> Result<(), String> {
        if upto >= seq {
            return Err(format!("upto {upto} should come before this event"));
        }
        match self.compacted {
            Some(last) if upto < last => Err(format!(
                "upto {upto} is before the last compaction's {last}; compaction only moves forward"
            )),
            _ => Ok(()),
        }
    }

    /// 记下查过的这一条带来的变化。
    fn record(&mut self, event: &Event) {
        self.next = event.seq.next();
        match &event.body {
            Body::TurnStarted(_) => {
                let turn = TurnId::new(event.seq);
                self.open = Some(turn);
                self.turns.insert(turn);
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
            Body::ContextCompacted(compacted) => {
                self.compacted = Some(compacted.upto);
                self.turns.retain(|turn| turn.started() > compacted.upto);
                self.undone.clear();
            }
            Body::TurnReverted(reverted) => {
                for turn in &reverted.turns {
                    self.turns.remove(turn);
                }
                self.undone.push(reverted.turns.clone());
            }
            Body::TurnUnreverted(unreverted) => {
                self.undone.pop();
                self.turns.extend(unreverted.turns.iter().copied());
            }
            _ => {}
        }
    }
}

/// 只在回合里发生的种类：模型的回复、工具的结果、请人确认和人的决定、问人和人的回答、
/// 撤回排着队的消息、回合结束。
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
    )
}

/// 几个回合编号，写成「11、12」。
fn listed(turns: &[TurnId]) -> String {
    let turns: Vec<String> = turns.iter().map(ToString::to_string).collect();
    turns.join(", ")
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
