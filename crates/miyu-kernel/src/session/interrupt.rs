//! 打断（`docs/designs/02-内核.md` 第六节「打断和急着插话」「排队的消息」，蓝图 `kernel/session.md`「打断」）：回合
//! 走到哪一步都能打断，还没有结果的调用各补一条「已取消」（不变量 2）。排着队的消息，接着发就马上开一轮，退回就
//! 撤回来。
//!
//! 改文件的调用打断时正在跑的，先叫它停，等它停在改之前或者做完再收尾（施工 4-9 再补一）：写文件的活在阻塞线程里，
//! 当场记「已取消」的话，它照样改完了，效果却没处记，撤销就改不回来。

use super::action::{Action, Reason};
use super::input::Queued;
use super::step::{State, Step};
use super::turn::{Interrupting, Stage};
use super::{Session, accepted, rejected};
use crate::event::{Body, EndReason, Event, ToolResult, ToolStatus};
use crate::id::CommandId;
use crate::origin::{By, Tool};
use crate::time::Timestamp;

/// 打断以后等停着的改文件的调用最多多久，毫秒（「打断」第 7 条）：一般几毫秒就交回来了。
pub(super) const STOP_WAIT_MS: i64 = 10_000;

impl Session {
    /// 打断正在进行的回合。没有回合在进行，拒绝，原因码 `not_running`。
    ///
    /// - 请求在路上：叫执行器别再发了；收到的半截写成被打断的回复，里面留下的调用补「已取消，
    ///   没跑过」；`model.called` 的结果是被打断。
    /// - 工具在跑：改文件的叫它停，先不记结果；别的叫停，补「已取消，跑到一半」；还没派的补「已取消，没跑过」。
    /// - 别的阶段：什么都还没发出去，直接结束。
    ///
    /// 然后看排着队的：接着发的，马上开一轮；退回的，撤回来。补的结果、撤回和 `turn.ended`，
    /// `by` 是打断的人，`cause` 是这个命令；回应附上这一次追加的全部事件。有停着的，先不收尾（[`Self::wait`]）；
    /// 已经在等停着的，这一次打断就是不等了（[`Self::force_stop`]）。
    pub(super) fn interrupt(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        queued: Queued,
    ) -> Vec<Action> {
        let Some(turn) = self.turn.as_mut() else {
            return vec![rejected(id, Reason::NotRunning)];
        };
        if turn.interrupting.is_some() {
            return self.force_stop(at, Some((id, by, queued)));
        }
        let turn_cause = turn.cause.clone();
        let stage = std::mem::replace(&mut turn.stage, Stage::Settling);
        let mut events = Vec::new();
        let mut stops = Vec::new();
        match stage {
            Stage::Asking(call) => {
                let seen = call.seen;
                let (settled, calls) = self.cut_off(at, call, turn_cause);
                events.extend(settled);
                let text = self.policy.tool_texts.cancelled_before();
                for call in calls {
                    events.push(self.written_result(
                        at,
                        by.clone(),
                        Some(id.clone()),
                        call.call_id,
                        ToolStatus::Cancelled,
                        text.clone(),
                    ));
                }
                stops.push(Action::CancelModel { seen });
            }
            Stage::Tools(mut step) => {
                let (cancelled, cancels) = self.cancel_step(at, &by, &id, &mut step);
                events.extend(cancelled);
                stops.extend(cancels);
                if step.calls.iter().any(|call| call.state == State::Stopping) {
                    let waiting = Interrupting {
                        by,
                        cause: id.clone(),
                        queued,
                        wake: step.reply,
                    };
                    return self.wait(at, id, waiting, step, events, stops);
                }
            }
            // 等着重试的、在等转述的：请求还没发，叫醒了、转述回来了也不理，直接结束（转述回来的照样记，`sight.rs`）。
            Stage::Opening { .. }
            | Stage::Hooking
            | Stage::Ready
            | Stage::Looking { .. }
            | Stage::Waiting { .. }
            | Stage::Settling => {}
        }
        events.extend(self.close_interrupted(at, &by, &id, queued));
        self.accept(id, events.iter().map(|event| event.seq).collect());
        let mut actions = vec![Action::Append(events)];
        actions.extend(stops);
        actions
    }

    /// 有停着的调用（「打断」第 7 条）：这一步放回去，先不收尾；这一批落了盘就回应，一个都没有就当场回应；出叫停的
    /// 动作，再到点叫醒，记号是那一步回复的序号。
    fn wait(
        &mut self,
        at: Timestamp,
        id: CommandId,
        waiting: Interrupting,
        step: Step,
        events: Vec<Event>,
        stops: Vec<Action>,
    ) -> Vec<Action> {
        let wake = waiting.wake;
        if let Some(turn) = self.turn.as_mut() {
            turn.stage = Stage::Tools(step);
            turn.interrupting = Some(waiting);
        }
        let mut actions = Vec::new();
        if events.is_empty() {
            self.recent.insert(id.clone(), Vec::new());
            actions.push(accepted(id, Vec::new()));
        } else {
            self.accept(id, events.iter().map(|event| event.seq).collect());
            actions.push(Action::Append(events));
        }
        actions.extend(stops);
        actions.push(Action::Wake {
            at: later(at, STOP_WAIT_MS),
            seen: wake,
        });
        actions
    }

    /// 停着的调用交回来了（「打断」第 7 条）：停在改之前的（状态是已取消）补「已取消，跑到一半」，`by`、`cause` 是
    /// 打断的；改完了的照交的记，和平常的结果一样。都交回来了，收尾。不是停着的，不理。
    pub(super) fn stopped_done(&mut self, at: Timestamp, result: ToolResult) -> Vec<Action> {
        let call_id = result.call_id;
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        let turn_cause = turn.cause.clone();
        let Some(waiting) = &turn.interrupting else {
            return Vec::new();
        };
        let (by, cause, queued) = (waiting.by.clone(), waiting.cause.clone(), waiting.queued);
        let Stage::Tools(step) = &mut turn.stage else {
            return Vec::new();
        };
        let Some(call) = step.find(call_id, State::Stopping) else {
            return Vec::new();
        };
        call.state = State::Done;
        let left = step.calls.iter().any(|call| call.state == State::Stopping);
        let settled = if result.status == ToolStatus::Cancelled {
            let text = self.policy.tool_texts.cancelled_running();
            self.written_result(
                at,
                by.clone(),
                Some(cause.clone()),
                call_id,
                ToolStatus::Cancelled,
                text,
            )
        } else {
            let by = By::Tool(Tool { call_id });
            self.record(at, by, turn_cause, Body::ToolResult(result))
        };
        let mut events = vec![settled];
        if !left {
            if let Some(turn) = self.turn.as_mut() {
                turn.interrupting = None;
                turn.stage = Stage::Settling;
            }
            events.extend(self.close_interrupted(at, &by, &cause, queued));
        }
        vec![Action::Append(events)]
    }

    /// 不等停着的了（「打断」第 7 条第 4 款）：又打断了一次（`again`：命令、打断的人、排着队的怎么办），到点了，或者
    /// 要重启了。还没交回来的补「已取消，跑到一半」，出 `CancelTool`，收尾。又打断的，补的、收尾的照这一次；别的照
    /// 原来那一次。没在等停着的，什么都不做。
    pub(super) fn force_stop(
        &mut self,
        at: Timestamp,
        again: Option<(CommandId, By, Queued)>,
    ) -> Vec<Action> {
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        let Some(waiting) = turn.interrupting.take() else {
            return Vec::new();
        };
        let stage = std::mem::replace(&mut turn.stage, Stage::Settling);
        let (id, by, queued) = match again {
            Some((id, by, queued)) => (Some(id), by, queued),
            None => (None, waiting.by, waiting.queued),
        };
        let cause = id.clone().unwrap_or(waiting.cause);
        let mut events = Vec::new();
        let mut stops = Vec::new();
        if let Stage::Tools(step) = stage {
            let text = self.policy.tool_texts.cancelled_running();
            for call in step.calls {
                if call.state != State::Stopping {
                    continue;
                }
                stops.push(Action::CancelTool { call_id: call.id });
                events.push(self.written_result(
                    at,
                    by.clone(),
                    Some(cause.clone()),
                    call.id,
                    ToolStatus::Cancelled,
                    text.clone(),
                ));
            }
        }
        events.extend(self.close_interrupted(at, &by, &cause, queued));
        if let Some(id) = id {
            self.accept(id, events.iter().map(|event| event.seq).collect());
        }
        let mut actions = vec![Action::Append(events)];
        actions.extend(stops);
        actions
    }

    /// 收尾（「打断」第 5 条）：退回的先撤回排着的，再 `turn.ended`（`interrupted`）；接着发的，队里有的接着开下一轮。
    fn close_interrupted(
        &mut self,
        at: Timestamp,
        by: &By,
        cause: &CommandId,
        queued: Queued,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        match queued {
            Queued::Return => events.extend(self.withdraw_queued(at, by, cause)),
            // 留着：清掉队，回合结束时就不接着开；排着的那几条照样在日志里。
            Queued::Keep => {
                if let Some(turn) = self.turn.as_mut() {
                    turn.queued.clear();
                }
            }
            Queued::Send => {}
        }
        events.extend(self.finish_turn(
            at,
            by.clone(),
            Some(cause.clone()),
            EndReason::Interrupted,
        ));
        events
    }
}

/// `at` 以后 `wait` 毫秒；算不出来的（快到时间的尽头了）就是 `at`。
fn later(at: Timestamp, wait: i64) -> Timestamp {
    at.unix_millis()
        .checked_add(wait)
        .and_then(Timestamp::from_unix_millis)
        .unwrap_or(at)
}
