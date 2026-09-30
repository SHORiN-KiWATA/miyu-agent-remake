//! 有效历史：投影要用的那一段（`docs/designs/03-事件模型.md` 第七节「有效历史」）。
//!
//! 从最近一次压缩算起，去掉撤销掉的回合。账本（[`crate::ledger`]）查过的事件才交给这里，
//! 这里只留还要发给模型的那些。压缩一次就丢掉更早的；撤销一次，撤掉的先放在一边，下一轮开始、
//! 压缩了才丢（`history/undo.rs`）。所以占的内存随上下文窗口走，不随日志走（`07-存储.md` 第七节）。
//!
//! 撤销能撤掉压缩（施工 6-9）：更早的那一段从日志读回来，从留着一切的一份（[`History::whole`]）收起，收完落到
//! 检查点上（[`History::settle`]，`docs/blueprint/kernel/history.md`「从日志的一段重建」）。载入也这样重建。

use std::collections::{BTreeMap, BTreeSet};

use crate::event::{Body, Event};
use crate::id::{ContentHash, JobId, Seq, SessionId, TurnId};

mod jobs;
mod undo;

pub use jobs::Dispatched;

/// 一个会话的有效历史：最近一次压缩的检查点，加上它之后还有效的事件。
///
/// 投影时检查点排在最前面，后面是 [`History::events`]，照日志的先后。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    /// 最近一次压缩的检查点（`context.compacted`）。日志里它排在被动压缩保下来的尾巴后面，
    /// 投影里却要排在最前，所以单独放。
    checkpoint: Option<Event>,
    /// 检查点之后还有效的事件，照日志的先后。被动压缩保下来的尾巴也在这里。
    events: Vec<Event>,
    /// 还能恢复的几次撤销，各拿走了哪些事件（照日志的先后），最近的一次在最后。
    undone: Vec<Vec<Event>>,
    /// 留着一切的那一份（[`History::whole`]）：压缩替代掉的不丢。
    whole: bool,
    /// 最近一个检查点里重读的文件的原文，照 blob 找（施工 6-5）：原文不进日志，由执行器交进来。
    recalled: BTreeMap<ContentHash, String>,
    /// 派出去过的任务（施工 7-2，`history/jobs.rs`）：压缩不丢，撤销、恢复跟着标。父会话也记在那里（施工 C-2）。
    jobs: jobs::Jobs,
}

impl History {
    /// 留着一切的一份（施工 6-4）：压缩替代掉的不丢，`context.compacted` 自己也照先后留在 [`History::events`] 里，
    /// 没有检查点；撤销、恢复、撤回照同一套规矩算。`history` 照它算日志里哪些还算数。
    pub fn whole() -> History {
        History {
            whole: true,
            ..History::default()
        }
    }

    /// 放进现在这个检查点里重读的文件的原文（施工 6-5）：压完时照执行器交回的，别的时候照 `Input::Recalled`（施工
    /// 6-9）。
    pub fn recall(&mut self, texts: BTreeMap<ContentHash, String>) {
        self.recalled.extend(texts);
    }

    /// 重读的文件 `blob` 的原文；没交进来的没有（施工 6-5）。
    pub fn recalled(&self, blob: &ContentHash) -> Option<&str> {
        self.recalled.get(blob).map(String::as_str)
    }

    /// 派出去过的编号是 `job` 的任务（施工 7-2）：标题、种类、派它的那一轮撤掉了没有。压缩换掉了派它的那一条也在；没派过
    /// 的没有。
    pub fn dispatched(&self, job: &JobId) -> Option<&Dispatched> {
        self.jobs.get(job)
    }

    /// 在这几轮里派出去过的任务，照编号（施工 7-8）：撤销这几轮时停掉还在跑的。
    pub fn dispatched_in(&self, turns: &[TurnId]) -> Vec<JobId> {
        self.jobs.in_turns(turns).collect()
    }

    /// 在会话 `session` 里跑的子代理：编号和派它时记下的（施工 7-7）。子代理发来的留言照发消息的会话认出是哪一个，标签里
    /// 写它的编号、标题。不是这个会话派的子代理的没有。
    pub fn subagent(&self, session: &SessionId) -> Option<(JobId, &Dispatched)> {
        self.jobs.in_session(session)
    }

    /// 会话 `session` 发来的话是别的会话发来的（施工 C-2，`docs/blueprint/cross-session.md` 第四条第 1 款）：它不是这个
    /// 会话的父会话，也不是这个会话派的子代理（派它的那一轮撤掉了的也算派过）。渲染、`history` 照它写标签、「谁」。
    pub fn is_peer(&self, session: &SessionId) -> bool {
        !self.jobs.is_parent(session) && self.jobs.in_session(session).is_none()
    }

    /// 只记派出去的任务，不留这一条（施工 7-2）：载入时，有效历史重建的那一段以前的事件照它过一遍，派出去过的任务才是
    /// 全的（`kernel/history.md`「从日志的一段重建」）。
    pub fn note(&mut self, event: &Event) {
        self.jobs.note(event);
    }

    /// 派出去过的任务照 `before` 那一份的（施工 7-2）：撤掉压缩时从读回的一段重建了有效历史，那一段以前派的只有原来那份
    /// 记着。
    pub fn jobs_from(&mut self, before: &History) {
        self.jobs = before.jobs.clone();
    }

    /// 最近一次压缩的检查点；没压缩过就没有。
    pub fn checkpoint(&self) -> Option<&Event> {
        self.checkpoint.as_ref()
    }

    /// 检查点之后还有效的事件，照日志的先后。
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// 最近一次撤销拿走的事件，照日志的先后；没有能恢复的撤销就是空的（施工 4-7 上：撤销、恢复时照它算要改回的
    /// 文件）。
    pub fn last_undone(&self) -> &[Event] {
        self.undone.last().map_or(&[], Vec::as_slice)
    }

    /// 有效历史的前一段：检查点照留，之后的事件只留第 `upto` 条及以前的，放在一边的撤销不要。压缩的摘要请求照它
    /// 组装（`compaction.md` 第三条第 3 条）：第 `upto` 条刚写下时的有效历史就是这样，后来撤掉、撤回的照样不在。
    pub fn until(&self, upto: Seq) -> History {
        History {
            checkpoint: self.checkpoint.clone(),
            events: self
                .events
                .iter()
                .filter(|event| event.seq <= upto)
                .cloned()
                .collect(),
            undone: Vec::new(),
            whole: self.whole,
            recalled: self.recalled.clone(),
            jobs: self.jobs.clone(),
        }
    }

    /// 有效历史的后一段：检查点照留，之后的事件只留第 `cut` 条以后的，放在一边的撤销不要（施工 6-6 中）。摘要请求超长
    /// 截掉最老的几组再试时，照 `until(N)` 再截它组装（`compaction.md` 第三条第 10 条）。
    pub fn after(&self, cut: Seq) -> History {
        History {
            checkpoint: self.checkpoint.clone(),
            events: self
                .events
                .iter()
                .filter(|event| event.seq > cut)
                .cloned()
                .collect(),
            undone: Vec::new(),
            whole: self.whole,
            recalled: self.recalled.clone(),
            jobs: self.jobs.clone(),
        }
    }

    /// 检查点之后还有效的事件，照每次请求当时看到的样子排好（03 第六节「照每次请求
    /// 看到的范围排」）。投影照这个先后一条条渲染。
    ///
    /// 以回复为界切段：一段是上一次请求看到的之后、下一次请求看到的为止。每一段里，
    /// 先是这一段开头的那条回复，接着是它的工具结果，按调用的先后；然后是这一段里
    /// 别的事件，照日志的先后。第一条回复之前的那一段照日志的先后。
    pub fn ordered(&self) -> Vec<&Event> {
        // 每条回复看到了第几条为止。账本保证它一次比一次大（02 第九节），所以可以当段的边界。
        let seen: Vec<Seq> = self
            .events
            .iter()
            .filter_map(|event| match &event.body {
                Body::MessageAssistant(reply) => Some(reply.seen),
                _ => None,
            })
            .collect();
        // 段 0 是第一条回复看到的那些；段 k 从第 k 条回复看到的之后开始。
        let mut segments: Vec<Vec<&Event>> = vec![Vec::new(); seen.len() + 1];
        for event in &self.events {
            let segment = seen.partition_point(|&boundary| boundary < event.seq);
            segments[segment].push(event);
        }
        segments.into_iter().flat_map(reply_first).collect()
    }

    /// 落到检查点上（施工 6-9）：事件里有 `context.compacted` 的，最近的那一条当检查点，换掉原来的；事件只留序号大于
    /// 它的 `upto`、不是 `context.compacted` 的；重读的原文清掉。交回检查点换了没有。放在一边的不动：里面的压缩，等
    /// 恢复放回来再落。
    ///
    /// 留着一切的那一份（[`History::whole`]）调过它，就成了平时那一份：从日志的一段重建就是这样收完的。平时那一份的
    /// 事件里只有恢复放回来的才有压缩。
    pub fn settle(&mut self) -> bool {
        self.whole = false;
        let Some(k) = self
            .events
            .iter()
            .rposition(|event| matches!(event.body, Body::ContextCompacted(_)))
        else {
            return false;
        };
        let checkpoint = self.events.remove(k);
        if let Body::ContextCompacted(compacted) = &checkpoint.body {
            let upto = compacted.upto;
            self.events
                .retain(|kept| kept.seq > upto && !matches!(kept.body, Body::ContextCompacted(_)));
        }
        self.checkpoint = Some(checkpoint);
        self.recalled.clear();
        true
    }

    /// 追加一条账本查过的事件。
    ///
    /// 压缩：换上新的检查点，序号在它 `upto` 之前的事件和旧的检查点一起丢掉，
    /// 新摘要里已经包着它们；留着一切的那一份（[`History::whole`]）什么都不丢，压缩照先后留成一条。撤销：撤掉的回合连同跟着撤的话拿走，先放在一边；恢复：放回原处
    /// （`history/undo.rs`）；下一轮开始、压缩了，放在一边的就丢掉。撤回：丢掉撤回的消息。`turn.reverted`、
    /// `turn.unreverted`、`message.withdrawn` 本身用过就丢，它们不进上下文。其余的照先后留着。放回来的里面有压缩的（撤掉
    /// 压缩的那一次撤销放在一边的），再落到检查点上（[`History::settle`]）。每一条都先记派出去的任务（[`History::note`]）。
    pub fn append(&mut self, event: Event) {
        self.jobs.note(&event);
        match &event.body {
            Body::ContextCompacted(_) if self.whole => {
                self.events.push(event);
                self.undone.clear();
            }
            Body::ContextCompacted(compacted) => {
                let upto = compacted.upto;
                self.events.retain(|kept| kept.seq > upto);
                self.checkpoint = Some(event);
                self.undone.clear();
                self.recalled.clear();
            }
            Body::TurnReverted(reverted) => self.revert(&reverted.turns),
            Body::TurnUnreverted(_) => self.unrevert(),
            Body::MessageWithdrawn(withdrawn) => {
                let messages: BTreeSet<Seq> = withdrawn.messages.iter().copied().collect();
                self.events.retain(|kept| {
                    !(messages.contains(&kept.seq) && matches!(kept.body, Body::MessageUser(_)))
                });
            }
            Body::TurnStarted(_) => {
                self.undone.clear();
                self.events.push(event);
            }
            _ => self.events.push(event),
        }
    }
}

/// 一段里的先后：这一段开头的那条回复，它的工具结果（按调用的先后），然后别的事件
/// （照日志的先后）。段 0 没有开头的回复，照日志的先后。
fn reply_first(segment: Vec<&Event>) -> Vec<&Event> {
    let Some(reply) = segment
        .iter()
        .copied()
        .find(|event| matches!(event.body, Body::MessageAssistant(_)))
    else {
        return segment;
    };
    let result_of_reply = |event: &Event| match &event.body {
        Body::ToolResult(result) if result.call_id.message() == reply.seq => {
            Some(result.call_id.index())
        }
        _ => None,
    };
    let mut results: Vec<&Event> = segment
        .iter()
        .copied()
        .filter(|event| result_of_reply(event).is_some())
        .collect();
    results.sort_by_key(|event| result_of_reply(event));
    let rest = segment
        .iter()
        .copied()
        .filter(|event| event.seq != reply.seq && result_of_reply(event).is_none());
    std::iter::once(reply).chain(results).chain(rest).collect()
}

#[cfg(test)]
mod tests;
