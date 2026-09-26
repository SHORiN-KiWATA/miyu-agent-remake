//! 有计划的重启（`docs/designs/02-内核.md` 第六节「载入、崩溃、重启」第 3 条）：关之前先把正在
//! 进行的那一轮收拾好。再起来时接着干，在 [`super::load`]。

use super::Session;
use super::action::Action;
use super::step::State;
use super::turn::Stage;
use crate::event::{EndReason, ToolStatus};
use crate::id::CallId;
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 要重启了：正在进行的回合照打断收拾。请求在路上的截下半截，叫执行器别再发；在跑的叫执行器
    /// 停下；没有结果的调用都补一条「已取消：Miyu 重启了，没跑完」；`turn.ended` 的原因是
    /// `restarted`。`by` 都是内核，`cause` 是那一轮的。排着队的不接着开：要关了。没有回合在进行，
    /// 什么都不做。
    pub(super) fn restart(&mut self, at: Timestamp) -> Vec<Action> {
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        let cause = turn.cause.clone();
        let stage = std::mem::replace(&mut turn.stage, Stage::Settling);
        let mut events = Vec::new();
        let mut stops = Vec::new();
        let unfinished: Vec<CallId> = match stage {
            Stage::Asking(call) => {
                stops.push(Action::CancelModel { seen: call.seen });
                let (settled, calls) = self.cut_off(at, call, cause.clone());
                events.extend(settled);
                calls.into_iter().map(|call| call.call_id).collect()
            }
            Stage::Tools(step) => step
                .calls
                .into_iter()
                .filter(|call| call.state != State::Done)
                .map(|call| {
                    if call.executing() {
                        stops.push(Action::CancelTool { call_id: call.id });
                    }
                    call.id
                })
                .collect(),
            Stage::Opening { .. }
            | Stage::Hooking
            | Stage::Ready
            | Stage::Waiting { .. }
            | Stage::Settling => Vec::new(),
        };
        let text = self.policy.tool_texts.restarted();
        for call_id in unfinished {
            events.push(self.written_result(
                at,
                By::Kernel,
                cause.clone(),
                call_id,
                ToolStatus::Cancelled,
                text.clone(),
            ));
        }
        events.push(self.end_turn(at, By::Kernel, cause, EndReason::Restarted));
        let mut actions = vec![Action::Append(events)];
        actions.extend(stops);
        actions
    }
}
