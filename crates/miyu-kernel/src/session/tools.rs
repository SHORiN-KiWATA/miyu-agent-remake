//! 这一步的工具调用（`docs/designs/02-内核.md` 第六节「工具怎么调、下一步怎么走」）：先查、
//! 修正参数；回复落了盘才派；只读的一起跑，别的一个接一个；结果齐了请求下一次，到了步数
//! 上限就结束回合。

use super::Session;
use super::action::Action;
use super::turn::Stage;
use crate::block::{Block, Text, ToolCall};
use crate::event::{
    Body, EndReason, Event, ToolProgress, ToolResult, ToolStatus, Transient, TransientBody,
};
use crate::id::{CallId, CommandId, Seq};
use crate::origin::{By, Tool};
use crate::time::Timestamp;
use crate::tool::{Access, repair};

/// 这一步要跑的调用，照调用的先后。内核当场拦下的不在里面，它们已经有了结果。
#[derive(Debug)]
pub(super) struct Step {
    /// 回复的序号：它落了盘才派。
    reply: Seq,
    calls: Vec<Pending>,
}

/// 一个要跑的调用。
#[derive(Debug)]
struct Pending {
    id: CallId,
    name: String,
    /// 修正过的参数。
    args: String,
    access: Access,
    state: State,
}

/// 一个调用走到了哪。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// 还没派。
    Waiting,
    /// 派出去了，等结果。
    Running,
    /// 有了结果。
    Done,
}

impl Step {
    /// 现在能派的，照调用的先后：连着的只读调用一起派；不是只读的，等它前面的都有了结果，
    /// 它没有结果，后面的都等着。
    fn ready(&self) -> Vec<usize> {
        let mut ready = Vec::new();
        let mut earlier_pending = false;
        let mut earlier_exclusive = false;
        for (k, call) in self.calls.iter().enumerate() {
            if call.state == State::Done {
                continue;
            }
            let exclusive = call.access != Access::Read;
            let free = if exclusive {
                !earlier_pending
            } else {
                !earlier_exclusive
            };
            if call.state == State::Waiting && free {
                ready.push(k);
            }
            earlier_pending = true;
            earlier_exclusive |= exclusive;
        }
        ready
    }

    /// 这一步的调用都有了结果。
    fn finished(&self) -> bool {
        self.calls.iter().all(|call| call.state == State::Done)
    }
}

impl Session {
    /// 回复里的工具调用，先查：工具面上没有这个名字、参数不是 JSON 对象的，当场记一条出错的
    /// 结果，`by` 是内核；别的修正好参数，等回复落了盘再派。都拦下了的，这一步当场就齐了。
    /// 返回当场记下的事件。
    pub(super) fn start_tools(
        &mut self,
        at: Timestamp,
        reply: Seq,
        calls: Vec<ToolCall>,
        cause: Option<CommandId>,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        let mut pending = Vec::new();
        for call in calls {
            let checked = match self.policy.tools.get(&call.name) {
                None => Err(self.policy.tool_texts.unknown(&call.name)),
                Some(rule) => repair(&rule.parameters, &call.args)
                    .map(|args| (args, rule.access))
                    .map_err(|_| self.policy.tool_texts.not_an_object(&call.name)),
            };
            match checked {
                Ok((args, access)) => pending.push(Pending {
                    id: call.call_id,
                    name: call.name,
                    args,
                    access,
                    state: State::Waiting,
                }),
                Err(sentence) => {
                    let result = ToolResult {
                        call_id: call.call_id,
                        status: ToolStatus::Error,
                        blocks: vec![Block::Text(Text { text: sentence })],
                        duration_ms: None,
                    };
                    events.push(self.record(
                        at,
                        By::Kernel,
                        cause.clone(),
                        Body::ToolResult(result),
                    ));
                }
            }
        }
        let step = Step {
            reply,
            calls: pending,
        };
        let finished = step.finished();
        if let Some(turn) = self.turn.as_mut() {
            turn.stage = Stage::Tools(step);
        }
        if finished {
            events.extend(self.finish_step(at, cause));
        }
        events
    }

    /// 回复落了盘，派现在能派的，带上这一轮的工作目录。
    pub(super) fn dispatch(&mut self) -> Vec<Action> {
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        let Stage::Tools(step) = &mut turn.stage else {
            return Vec::new();
        };
        if self.stored.is_none_or(|stored| stored < step.reply) {
            return Vec::new();
        }
        let mut actions = Vec::new();
        for k in step.ready() {
            let call = &mut step.calls[k];
            call.state = State::Running;
            actions.push(Action::RunTool {
                call_id: call.id,
                name: call.name.clone(),
                args: call.args.clone(),
                cwd: turn.cwd.clone(),
            });
        }
        actions
    }

    /// 工具执行完了：追加 `tool.result`，`by` 是那次调用，然后派后面能派的。这一步齐了，
    /// 到了步数上限就结束回合，不然等落了盘请求下一次。不是这一步在跑的，不理。
    pub(super) fn tool_done(
        &mut self,
        at: Timestamp,
        call_id: CallId,
        error: bool,
        blocks: Vec<Block>,
        duration_ms: Option<u64>,
    ) -> Vec<Action> {
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        let cause = turn.cause.clone();
        let Stage::Tools(step) = &mut turn.stage else {
            return Vec::new();
        };
        let Some(call) = step
            .calls
            .iter_mut()
            .find(|call| call.id == call_id && call.state == State::Running)
        else {
            return Vec::new();
        };
        call.state = State::Done;
        let finished = step.finished();
        let result = ToolResult {
            call_id,
            status: if error {
                ToolStatus::Error
            } else {
                ToolStatus::Ok
            },
            blocks,
            duration_ms,
        };
        let by = By::Tool(Tool { call_id });
        let mut events = vec![self.record(at, by, cause.clone(), Body::ToolResult(result))];
        if finished {
            events.extend(self.finish_step(at, cause));
        }
        let mut actions = vec![Action::Append(events)];
        actions.extend(self.dispatch());
        actions
    }

    /// 工具执行中的一段输出：推给头，`by` 是那次调用。不是在跑的调用的，不理。
    pub(super) fn tool_progress(
        &mut self,
        at: Timestamp,
        call_id: CallId,
        text: String,
    ) -> Vec<Action> {
        let Some(turn) = self.turn.as_ref() else {
            return Vec::new();
        };
        let Stage::Tools(step) = &turn.stage else {
            return Vec::new();
        };
        if !step
            .calls
            .iter()
            .any(|call| call.id == call_id && call.state == State::Running)
        {
            return Vec::new();
        }
        vec![Action::PushTransient(Transient {
            at,
            turn: Some(turn.id),
            by: By::Tool(Tool { call_id }),
            cause: turn.cause.clone(),
            body: TransientBody::ToolProgress(ToolProgress { call_id, text }),
        })]
    }

    /// 这一步齐了：到了步数上限，结束回合；不然等追加过的事件都落了盘，请求下一次。
    fn finish_step(&mut self, at: Timestamp, cause: Option<CommandId>) -> Vec<Event> {
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        if self
            .policy
            .step_limit
            .is_some_and(|limit| turn.requests >= limit)
        {
            return vec![self.end_turn(at, cause, EndReason::StepLimit)];
        }
        turn.stage = Stage::Ready;
        Vec::new()
    }
}
