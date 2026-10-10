//! 一轮开始、结束，每次请求记下的（终端蓝图 `tui.md`「正文」第 4 条、「时间线」第 13 条）：开轮时开它的几条归到这一轮；
//! 结束时没结果的步记成打断了，写收尾那一条（手动压缩、清空那一轮不写）；每次主请求的用量加到这一轮上。

use miyu_kernel::event::{
    EndReason, Event, ModelCalled, Purpose, TurnEnded, TurnJoined, TurnStarted, Usage,
};
use miyu_kernel::id::TurnId;

use super::{Projector, Running};
use crate::entry::{Body, End, Entry, EntryId, ToolState};
use crate::explain::explain;

impl Projector {
    /// 一轮开始了。
    pub(super) fn started(&mut self, event: &Event, started: &TurnStarted) {
        let id = TurnId::new(event.seq);
        self.turns.push(id);
        self.turn = Some(Running {
            id,
            started: event.at,
            usage: None,
            level: self.level.clone(),
            failure: None,
            // 手动压缩、清空那一轮没有触发（施工 6-8）。
            manual: started.trigger.is_none() && started.triggers.is_empty(),
            spoke: false,
        });
        let mut triggers = started.triggers.clone();
        if let Some(trigger) = started.trigger
            && !triggers.contains(&trigger)
        {
            triggers.push(trigger);
        }
        triggers.sort_unstable();
        // 开它的那一句和排在它前面、还排着的几条一起开这一轮（两下 Esc 打断以后排着的一起发）。
        if let Some(&last) = triggers.last() {
            let earlier: Vec<_> = self.queued.range(..last).map(|(seq, _)| *seq).collect();
            triggers.extend(earlier);
            triggers.sort_unstable();
            triggers.dedup();
        }
        self.bring_in(&triggers);
    }

    /// 照记下的几条并进正在跑的这一轮。
    pub(super) fn joined(&mut self, _event: &Event, joined: &TurnJoined) {
        self.bring_in(&joined.triggers);
    }

    /// 一轮结束了：收起在写的块、在进行的那一段，没结果的步记成打断了，写收尾那一条。
    pub(super) fn ended(&mut self, event: &Event, ended: &TurnEnded) {
        let Some(running) = self.turn.take() else {
            return;
        };
        self.close_blocks();
        self.settle_tools(event);
        self.close_group();
        self.drop_compacting();
        let took =
            u64::try_from(event.at.unix_millis() - running.started.unix_millis()).unwrap_or(0);
        if running.manual {
            self.manual_ended(took, running.usage);
            return;
        }
        let error = match ended.reason {
            EndReason::Error => running.failure,
            _ => None,
        };
        let end = End {
            reason: ended.reason.clone(),
            endpoint: self.model.0.clone(),
            model: self.model.1.clone(),
            took_ms: took,
            usage: running.usage,
            level: running.level,
            explain: error
                .as_ref()
                .map(|error| explain(error, self.texts.local.as_ref())),
            error,
        };
        self.add(Entry {
            id: EntryId::event(event.seq),
            body: Body::End(end),
            turn: Some(running.id),
            hidden: false,
            at: event.at,
        });
    }

    /// 这一轮结束时还没结果的步：停表，记成打断了。
    fn settle_tools(&mut self, event: &Event) {
        let Some(turn) = self.turn_of_last() else {
            return;
        };
        let busy: Vec<EntryId> = self
            .entries
            .iter()
            .filter(|e| e.turn == Some(turn))
            .filter(|e| matches!(&e.body, Body::Tool(tool) if tool.state.busy()))
            .map(|e| e.id.clone())
            .collect();
        for id in busy {
            self.ended.insert(id.clone(), event.at);
            self.touch(&id, |entry| {
                if let Body::Tool(tool) = &mut entry.body {
                    tool.state = ToolState::Cancelled;
                }
            });
        }
    }

    /// 刚结束的这一轮：`self.turn` 已经拿走了，照开过的最后一轮。
    fn turn_of_last(&self) -> Option<TurnId> {
        self.turns.last().copied()
    }

    /// 一次模型请求记下了。辅助请求（回顾、起标题）不算；摘要请求记在压缩那一条上。
    pub(super) fn called(&mut self, event: &Event, called: &ModelCalled) {
        if called.purpose.is_some() {
            // 后台提前压的那一次（施工 6-11 三补）：用量、用时记下，换上时接在压缩那一条上。
            if called.purpose == Some(Purpose::Compaction) {
                self.prepared
                    .insert(called.seen, (called.usage, called.duration_ms));
            }
            return;
        }
        if called.compaction.is_some() {
            self.compaction_called(event, called);
            return;
        }
        self.discard_stream(called.seen);
        self.spans(event, called);
        if called.endpoint.is_some() {
            self.model.0.clone_from(&called.endpoint);
        }
        if called.model.is_some() {
            self.model.1.clone_from(&called.model);
        }
        if let Some(running) = self.turn.as_mut() {
            if let Some(usage) = &called.usage {
                running.usage = Some(add(running.usage.take(), usage));
            }
            // 后来又成了：前面报过的错不算。
            running.failure = called.error.clone();
        }
        self.heard(called.seen);
    }
}

/// 两份用量加起来。
fn add(total: Option<Usage>, more: &Usage) -> Usage {
    let Some(total) = total else {
        return *more;
    };
    let reasoning = match (total.reasoning, more.reasoning) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
    };
    Usage {
        uncached: total.uncached + more.uncached,
        cache_read: total.cache_read + more.cache_read,
        cache_write: total.cache_write + more.cache_write,
        output: total.output + more.output,
        reasoning,
    }
}
