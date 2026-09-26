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
    /// 最近一次压缩替代到哪。
    compacted: Option<Seq>,
    /// 最近一次压缩以后开过的回合，撤销只能撤它们。压缩一次，更早的就丢掉。
    turns: BTreeSet<TurnId>,
}

impl Default for Ledger {
    fn default() -> Self {
        Ledger {
            next: Seq::FIRST,
            open: None,
            last_reply: None,
            pending: BTreeSet::new(),
            compacted: None,
            turns: BTreeSet::new(),
        }
    }
}

impl Ledger {
    /// 下一条事件该是几号。追加的一方照这个号给事件编序号。
    pub fn next_seq(&self) -> Seq {
        self.next
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
            return Err(format!("序号应该是 {}", self.next));
        }
        let created = matches!(event.body, Body::SessionCreated(_));
        if seq == Seq::FIRST && !created {
            return Err("第 1 条应该是会话创建 session.created".to_string());
        }
        if seq != Seq::FIRST && created {
            return Err("会话创建只能是第 1 条".to_string());
        }
        self.check_turn(event)?;
        match &event.body {
            Body::MessageAssistant(message) => {
                self.check_seen(seq, message.seen)?;
                check_call_ids(seq, &message.blocks)
            }
            Body::ToolResult(result) if !self.pending.contains(&result.call_id) => Err(format!(
                "{} 不是一个还在等结果的调用：没有这个调用，或者它已经有了结果",
                result.call_id
            )),
            Body::TurnEnded(_) => match self.pending.first() {
                Some(call) => Err(format!("回合结束时，调用 {call} 还没有结果")),
                None => Ok(()),
            },
            Body::ContextCompacted(compacted) => self.check_compaction(seq, compacted.upto),
            Body::TurnReverted(reverted) => {
                match reverted
                    .turns
                    .iter()
                    .find(|turn| !self.turns.contains(turn))
                {
                    Some(turn) => Err(format!("回合 {turn} 不存在，或者在最近一次压缩之前")),
                    None => Ok(()),
                }
            }
            _ => Ok(()),
        }
    }

    /// 回合的几条：回合开始时没有别的回合在进行；带 `turn` 的事件属于正在进行的回合；
    /// 只在回合里发生的种类必须带 `turn`。
    fn check_turn(&self, event: &Event) -> Result<(), String> {
        if let Body::TurnStarted(started) = &event.body {
            if event.turn != Some(TurnId::new(event.seq)) {
                return Err("回合开始的 turn 应该是它自己的序号".to_string());
            }
            if let Some(open) = self.open {
                return Err(format!("回合 {open} 还没有结束"));
            }
            if started.trigger >= event.seq {
                return Err("trigger 应该是回合开始之前的一条".to_string());
            }
            return Ok(());
        }
        match event.turn {
            Some(turn) if Some(turn) != self.open => Err(format!("回合 {turn} 不是正在进行的回合")),
            None if in_turn_only(&event.body) => {
                Err(format!("{} 只在回合里发生，要带上 turn", event.body.kind()))
            }
            _ => Ok(()),
        }
    }

    /// 回复看到的在它自己之前，而且不早于上一条回复：后一次请求一定看过前一条回复
    /// （03 第六节）。所以 `seen` 一次比一次大，投影才切得了段。
    fn check_seen(&self, seq: Seq, seen: Seq) -> Result<(), String> {
        if seen >= seq {
            return Err(format!("seen {seen} 应该在这条回复之前"));
        }
        match self.last_reply {
            Some(last) if seen < last => Err(format!(
                "seen {seen} 早于上一条回复 {last}：后一次请求一定看过前一条回复"
            )),
            _ => Ok(()),
        }
    }

    /// 压缩只前进：替代到的位置在这一条之前，而且不早于上一次。
    fn check_compaction(&self, seq: Seq, upto: Seq) -> Result<(), String> {
        if upto >= seq {
            return Err(format!("upto {upto} 应该在这一条之前"));
        }
        match self.compacted {
            Some(last) if upto < last => {
                Err(format!("upto {upto} 早于上一次压缩的 {last}，压缩只前进"))
            }
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
            }
            Body::MessageAssistant(message) => {
                self.last_reply = Some(event.seq);
                self.pending
                    .extend(message.blocks.iter().filter_map(tool_call_id));
            }
            Body::ToolResult(result) => {
                self.pending.remove(&result.call_id);
            }
            Body::TurnEnded(_) => self.open = None,
            Body::ContextCompacted(compacted) => {
                self.compacted = Some(compacted.upto);
                self.turns.retain(|turn| turn.started() > compacted.upto);
            }
            _ => {}
        }
    }
}

/// 只在回合里发生的种类：模型的回复、工具的结果、回合结束。
fn in_turn_only(body: &Body) -> bool {
    matches!(
        body,
        Body::MessageAssistant(_) | Body::ToolResult(_) | Body::TurnEnded(_)
    )
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
                "第 {index} 个工具调用的编号应该是 call_{seq}_{index}，写的是 {call}"
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
        write!(f, "第 {} 条事件不能追加：{}", self.seq, self.why)
    }
}

impl std::error::Error for LedgerError {}

#[cfg(test)]
mod tests;
