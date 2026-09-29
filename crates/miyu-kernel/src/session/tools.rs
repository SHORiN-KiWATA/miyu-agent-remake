//! 这一步的工具调用（`docs/designs/02-内核.md` 第六节「工具怎么调、下一步怎么走」）：先查、
//! 修正参数，会话只读时写文件的当场拦下；回复落了盘，轮到的先过执行前的链（「确认怎么走」，
//! 在 [`super::approval`]）；只读的一起跑，别的一个接一个；跑着的时候问人（「提问怎么走」，在
//! [`super::question`]）；结果齐了请求下一次，到了步数上限就结束回合。每个调用走到了哪，记在
//! [`super::step`]。

use super::Session;
use super::action::Action;
use super::step::{Pending, State, Step};
use super::turn::{Interjection, Stage};
use crate::block::{Block, Text, ToolCall};
use crate::event::{
    Body, EndReason, Event, Permission, ToolProgress, ToolResult, ToolStatus, Transient,
    TransientBody,
};
use crate::id::{CallId, CommandId, Seq};
use crate::origin::{By, Tool};
use crate::time::Timestamp;
use crate::tool::{Access, Worded, repair};

/// 派一次调用的动作：带上这一轮的工作目录、加进来的目录，和这一刻实际生效的那一级。派出去的两条路（链当场放行的、
/// 等人决定了的）都经它，派出去的是同一个样子（施工 5-10 上）。
pub(super) fn run_tool(
    call_id: CallId,
    name: &str,
    args: &str,
    cwd: &str,
    dirs: &[String],
    permission: &Permission,
) -> Action {
    Action::RunTool {
        call_id,
        name: name.to_string(),
        args: args.to_string(),
        cwd: cwd.to_string(),
        dirs: dirs.to_vec(),
        permission: permission.clone(),
    }
}

impl Session {
    /// 回复里的工具调用，先查：工具面上没有这个名字、参数不是 JSON 对象的，当场记一条出错的
    /// 结果；只读生效时写文件的，当场记一条被拒绝的结果；`by` 都是内核。别的修正好参数，等回复
    /// 落了盘再派。都拦下了的，这一步当场就齐了。返回当场记下的事件。
    pub(super) fn start_tools(
        &mut self,
        at: Timestamp,
        reply: Seq,
        calls: Vec<ToolCall>,
        cause: Option<CommandId>,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        let mut pending = Vec::new();
        let skipping = self.turn.as_ref().and_then(|turn| {
            turn.interjected
                .as_ref()
                .map(|interjection| (interjection.by.clone(), interjection.cause.clone()))
        });
        for call in calls {
            if let Some((by, skip_cause)) = &skipping {
                let text = self.policy.tool_texts.skipped();
                events.push(self.written_result(
                    at,
                    by.clone(),
                    Some(skip_cause.clone()),
                    call.call_id,
                    ToolStatus::Skipped,
                    text,
                ));
                continue;
            }
            let checked = match self.policy.tools.get(&call.name) {
                None => Err(self.policy.tool_texts.unknown(&call.name)),
                Some(rule) => repair(&rule.parameters, &call.args)
                    .map(|args| (args, rule.access.clone()))
                    .map_err(|_| self.policy.tool_texts.not_an_object(&call.name)),
            };
            match checked {
                Ok((_, access)) if access.writes() && self.read_only_now() => {
                    let text = self.policy.tool_texts.read_only();
                    events.push(self.written_result(
                        at,
                        By::Kernel,
                        cause.clone(),
                        call.call_id,
                        ToolStatus::Denied,
                        text,
                    ));
                }
                Ok((args, access)) => pending.push(Pending {
                    id: call.call_id,
                    name: call.name,
                    args,
                    access,
                    state: State::Waiting,
                    asked: None,
                    questions: Vec::new(),
                    answers: Vec::new(),
                }),
                Err(sentence) => {
                    events.push(self.written_result(
                        at,
                        By::Kernel,
                        cause.clone(),
                        call.call_id,
                        ToolStatus::Error,
                        sentence,
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

    /// 回复落了盘以后：人允许了的，那条决定也落了盘就派去跑；人答完了的，那条回答也落了盘就
    /// 交给工具；轮到的，先交给执行前的链，带上这一轮的工作目录和实际生效的那一级。
    pub(super) fn dispatch(&mut self) -> Vec<Action> {
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        let Stage::Tools(step) = &mut turn.stage else {
            return Vec::new();
        };
        let Some(stored) = self.stored.filter(|stored| *stored >= step.reply) else {
            return Vec::new();
        };
        let mut actions = Vec::new();
        for call in &mut step.calls {
            match call.state {
                State::Approved { decided } if decided <= stored => {
                    call.state = State::Running;
                    actions.push(run_tool(
                        call.id,
                        &call.name,
                        &call.args,
                        &turn.cwd,
                        &turn.dirs,
                        &self.effective,
                    ));
                }
                State::Answered { answered } if answered <= stored => {
                    call.state = State::Running;
                    actions.push(Action::AnswerTool {
                        call_id: call.id,
                        answers: std::mem::take(&mut call.answers),
                    });
                }
                _ => {}
            }
        }
        for k in step.ready() {
            let call = &mut step.calls[k];
            call.state = State::Guarding;
            actions.push(Action::GuardTool {
                call_id: call.id,
                name: call.name.clone(),
                args: call.args.clone(),
                cwd: turn.cwd.clone(),
                dirs: turn.dirs.clone(),
                permission: self.effective.clone(),
            });
        }
        actions
    }

    /// 工具执行完了：照工具交的追加 `tool.result`（`result`，成功还是出错、内容、用时、说法、效果都在里面），
    /// `by` 是那次调用，然后派后面能派的。这一步齐了，到了步数上限就结束回合，不然等落了盘请求下一次。
    /// 问着人的也算在跑：题目跟着了结。不是这一步在跑的，不理。打断以后在等停着的，交给 [`Self::stopped_done`]。
    pub(super) fn tool_done(&mut self, at: Timestamp, result: ToolResult) -> Vec<Action> {
        let call_id = result.call_id;
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        if turn.interrupting.is_some() {
            return self.stopped_done(at, result);
        }
        let cause = turn.cause.clone();
        let Stage::Tools(step) = &mut turn.stage else {
            return Vec::new();
        };
        let Some(call) = step
            .calls
            .iter_mut()
            .find(|call| call.id == call_id && call.executing())
        else {
            return Vec::new();
        };
        call.state = State::Done;
        let finished = step.finished();
        let by = By::Tool(Tool { call_id });
        let recorded = if result.status == ToolStatus::Cancelled {
            // 没叫它停，却交回停在了改之前的：照已取消记，那一句内核写。执行器只在叫它停以后才交这种，这是兜底。
            let text = self.policy.tool_texts.cancelled_running();
            self.written_result(at, by, cause.clone(), call_id, ToolStatus::Cancelled, text)
        } else {
            self.record(at, by, cause.clone(), Body::ToolResult(result))
        };
        let mut events = vec![recorded];
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
            .any(|call| call.id == call_id && call.executing())
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

    /// 打断这一步：在跑的改文件的调用（访问类别 `write`）叫它停（`StopTool`），改成停着，先不记结果（施工 4-9
    /// 再补一）；在跑的别的叫执行器停下，补「已取消，跑到一半」，问着人的补「没回答：被打断了」；还没跑过的（没轮到、
    /// 在过链、在等人确认、人允许了还没派）补「已取消，没跑过」；有了结果的、停着的不动。补的结果 `by` 是打断的人，
    /// `cause` 是打断的命令。
    pub(super) fn cancel_step(
        &mut self,
        at: Timestamp,
        by: &By,
        cause: &CommandId,
        step: &mut Step,
    ) -> (Vec<Event>, Vec<Action>) {
        let mut events = Vec::new();
        let mut actions = Vec::new();
        for call in &mut step.calls {
            let text = match call.state {
                State::Done | State::Stopping => continue,
                State::Running if call.access == Access::Write => {
                    call.state = State::Stopping;
                    actions.push(Action::StopTool { call_id: call.id });
                    continue;
                }
                State::Questioning => {
                    actions.push(Action::CancelTool { call_id: call.id });
                    self.policy.tool_texts.question_interrupted()
                }
                State::Running | State::Answered { .. } => {
                    actions.push(Action::CancelTool { call_id: call.id });
                    self.policy.tool_texts.cancelled_running()
                }
                _ => self.policy.tool_texts.cancelled_before(),
            };
            call.state = State::Done;
            events.push(self.written_result(
                at,
                by.clone(),
                Some(cause.clone()),
                call.id,
                ToolStatus::Cancelled,
                text,
            ));
        }
        (events, actions)
    }

    /// 急着插话：记在回合上，下一次请求之前还没跑的调用都跳过。这一步还没跑过的，当场补
    /// 「已跳过」，`by` 是说话的人；这一步因此齐了的，往下走。
    pub(super) fn interject(&mut self, at: Timestamp, by: By, cause: CommandId) -> Vec<Event> {
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        turn.interjected = Some(Interjection {
            by: by.clone(),
            cause: cause.clone(),
        });
        let turn_cause = turn.cause.clone();
        let Stage::Tools(step) = &mut turn.stage else {
            return Vec::new();
        };
        let skipped = settle(step, Pending::not_run);
        let finished = step.finished();
        let text = self.policy.tool_texts.skipped();
        let mut events: Vec<Event> = skipped
            .into_iter()
            .map(|call_id| {
                self.written_result(
                    at,
                    by.clone(),
                    Some(cause.clone()),
                    call_id,
                    ToolStatus::Skipped,
                    text.clone(),
                )
            })
            .collect();
        if finished && !events.is_empty() {
            events.extend(self.finish_step(at, turn_cause));
        }
        events
    }

    /// 内核替工具写的一条结果：没执行过，没有用时。给模型看的字、给人看的说法都在 `worded` 里。
    pub(super) fn written_result(
        &mut self,
        at: Timestamp,
        by: By,
        cause: Option<CommandId>,
        call_id: CallId,
        status: ToolStatus,
        worded: Worded,
    ) -> Event {
        let result = ToolResult {
            call_id,
            status,
            blocks: vec![Block::Text(Text { text: worded.text })],
            duration_ms: None,
            human: worded.said,
            effects: Vec::new(),
        };
        self.record(at, by, cause, Body::ToolResult(result))
    }

    /// 收紧成了只读：这一步里还没跑过、要写入的调用当场拦下，`by` 是内核：写文件的调用，和请人
    /// 确认的是写入的。这一步因此齐了的，往下走。已经在跑的不动：它已经跑了，沙盒管着（M5）。
    pub(super) fn deny_waiting_writes(&mut self, at: Timestamp) -> Vec<Event> {
        if !self.read_only_now() {
            return Vec::new();
        }
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        let cause = turn.cause.clone();
        let Stage::Tools(step) = &mut turn.stage else {
            return Vec::new();
        };
        let denied = settle(step, |call| call.not_run() && call.writes());
        let finished = step.finished();
        let text = self.policy.tool_texts.read_only();
        let mut events: Vec<Event> = denied
            .into_iter()
            .map(|call_id| {
                self.written_result(
                    at,
                    By::Kernel,
                    cause.clone(),
                    call_id,
                    ToolStatus::Denied,
                    text.clone(),
                )
            })
            .collect();
        if finished && !events.is_empty() {
            events.extend(self.finish_step(at, cause));
        }
        events
    }

    /// 这一步齐了：到了步数上限，结束回合；不然这一轮里切过级别的先把事实查一遍，等追加过的
    /// 事件都落了盘，请求下一次。
    pub(super) fn finish_step(&mut self, at: Timestamp, cause: Option<CommandId>) -> Vec<Event> {
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        if self
            .policy
            .step_limit
            .is_some_and(|limit| turn.requests >= limit)
        {
            return self.finish_turn(at, By::Kernel, cause, EndReason::StepLimit);
        }
        turn.stage = Stage::Ready;
        self.refresh_facts(at)
    }
}

/// 这一步里合 `which` 的调用都记成有了结果，返回它们的编号，照调用的先后。
pub(super) fn settle(step: &mut Step, which: impl Fn(&Pending) -> bool) -> Vec<CallId> {
    step.calls
        .iter_mut()
        .filter(|call| which(call))
        .map(|call| {
            call.state = State::Done;
            call.id
        })
        .collect()
}
