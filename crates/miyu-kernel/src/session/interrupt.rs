//! 打断（`docs/designs/02-内核.md` 第六节「打断和急着插话」「排队的消息」）：回合走到哪一步都能
//! 打断，还没有结果的调用各补一条「已取消」（不变量 2）。排着队的消息，接着发就马上开一轮，
//! 退回就撤回来。

use super::action::{Action, Reason};
use super::input::Queued;
use super::turn::Stage;
use super::{Session, rejected};
use crate::event::{EndReason, ToolStatus};
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 打断正在进行的回合。没有回合在进行，拒绝，原因码 `not_running`。
    ///
    /// - 请求在路上：叫执行器别再发了；收到的半截写成被打断的回复，里面留下的调用补「已取消，
    ///   没跑过」；`model.called` 的结果是被打断。
    /// - 工具在跑：在跑的叫停，补「已取消，跑到一半」；还没派的补「已取消，没跑过」。
    /// - 别的阶段：什么都还没发出去，直接结束。
    ///
    /// 然后看排着队的：接着发的，马上开一轮；退回的，撤回来。补的结果、撤回和 `turn.ended`，
    /// `by` 是打断的人，`cause` 是这个命令；回应附上这一次追加的全部事件。
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
            Stage::Tools(step) => {
                let (cancelled, cancels) = self.cancel_step(at, &by, &id, step);
                events.extend(cancelled);
                stops.extend(cancels);
            }
            Stage::Opening { .. } | Stage::Hooking | Stage::Ready | Stage::Settling => {}
        }
        if queued == Queued::Return {
            events.extend(self.withdraw_queued(at, &by, &id));
        }
        events.extend(self.finish_turn(at, by, Some(id.clone()), EndReason::Interrupted));
        self.accept(id, events.iter().map(|event| event.seq).collect());
        let mut actions = vec![Action::Append(events)];
        actions.extend(stops);
        actions
    }
}
