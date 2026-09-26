//! 确认（`docs/designs/02-内核.md` 第六节「确认怎么走」）：执行前的链交回的结论，人的回答。
//!
//! 链放行的派；拒绝的当场记下；要问人的记一条请求，这个调用停下来等人。人允许的，决定落了盘
//! 再派；拒绝的，记一条被拒绝的结果，这一步照常往下走，她接着干（`11-权限与沙盒.md` A13）。

use super::action::{Action, Reason};
use super::input::Verdict;
use super::step::{Asked, State};
use super::turn::Stage;
use super::{Session, rejected};
use crate::event::{ApprovalDecided, ApprovalRequested, Body, Decision, Event, ToolStatus};
use crate::id::{CallId, CommandId};
use crate::origin::{By, Module};
use crate::time::Timestamp;

/// 链的结论落到一个调用上以后，要记什么。
enum Settled {
    /// 被拒绝了：谁拒的，写给模型的那一句。
    Denied(By, String),
    /// 要问人：谁问的，请求。
    Asked(By, ApprovalRequested),
}

impl Session {
    /// 执行前的链判完了。放行的派；拒绝的记一条被拒绝的结果，`by` 是拒绝的模块；要问人的，记一条
    /// 请求，`by` 是提问的模块，这个调用停下来等人。没人能确认的、只读时要的是写入的，不记请求，
    /// 当场拒绝，`by` 是内核。不是这一步在等结论的调用，不理：被打断、跳过、拦下以后迟到的就是这种。
    pub(super) fn tool_guarded(
        &mut self,
        at: Timestamp,
        call_id: CallId,
        verdict: Verdict,
    ) -> Vec<Action> {
        let read_only = self.read_only_now();
        let attended = self.policy.attended;
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        let cause = turn.cause.clone();
        let cwd = turn.cwd.clone();
        let Stage::Tools(step) = &mut turn.stage else {
            return Vec::new();
        };
        let Some(call) = step.find(call_id, State::Guarding) else {
            return Vec::new();
        };
        let texts = &self.policy.tool_texts;
        let settled = match verdict {
            Verdict::Allow => {
                call.state = State::Running;
                return vec![Action::RunTool {
                    call_id,
                    name: call.name.clone(),
                    args: call.args.clone(),
                    cwd,
                }];
            }
            Verdict::Deny { module, text } => {
                Settled::Denied(By::Module(Module { id: module }), text)
            }
            Verdict::Ask { .. } if !attended => Settled::Denied(By::Kernel, texts.unattended()),
            Verdict::Ask { access, .. } if read_only && access.writes() => {
                Settled::Denied(By::Kernel, texts.read_only())
            }
            Verdict::Ask {
                module,
                access,
                rule,
                detail,
            } => {
                call.asked = Some(Asked {
                    access: access.clone(),
                    rule: rule.is_some(),
                });
                let request = ApprovalRequested {
                    call_id,
                    access,
                    rule,
                    detail,
                };
                Settled::Asked(By::Module(Module { id: module }), request)
            }
        };
        call.state = match settled {
            Settled::Denied(..) => State::Done,
            Settled::Asked(..) => State::Asking,
        };
        let finished = step.finished();
        let mut events = match settled {
            Settled::Denied(by, text) => {
                vec![self.written_result(at, by, cause.clone(), call_id, ToolStatus::Denied, text)]
            }
            Settled::Asked(by, request) => {
                vec![self.record(at, by, cause.clone(), Body::ApprovalRequested(request))]
            }
        };
        if finished {
            events.extend(self.finish_step(at, cause));
        }
        let mut actions = vec![Action::Append(events)];
        actions.extend(self.dispatch());
        actions
    }

    /// 人回答了一次确认：记一条决定，`by` 是回答的人，`cause` 是这个命令。允许的，决定落了盘
    /// 再派；拒绝的，再记一条被拒绝的结果，写了理由的带上理由，这一步照常往下走。回应附上这一次
    /// 追加的全部事件的序号。
    ///
    /// 不在等确认的、选项不认识的、请求没提规则却选了记住的、允许却带了理由的，拒绝这个命令。
    /// 空的理由当没写。
    pub(super) fn answer(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        call_id: CallId,
        decision: Decision,
        reason: Option<String>,
    ) -> Vec<Action> {
        let reason = reason.filter(|reason| !reason.trim().is_empty());
        let Some(rule) = self.asking(call_id) else {
            return vec![rejected(id, Reason::NotAsking)];
        };
        let deny = match decision {
            Decision::Other(_) => return vec![rejected(id, Reason::UnknownDecision)],
            Decision::Session | Decision::Workspace if !rule => {
                return vec![rejected(id, Reason::NoRule)];
            }
            Decision::Deny => true,
            _ if reason.is_some() => return vec![rejected(id, Reason::UnexpectedReason)],
            _ => false,
        };
        let body = ApprovalDecided {
            call_id,
            decision,
            reason: reason.clone(),
        };
        let decided = self.record(
            at,
            by.clone(),
            Some(id.clone()),
            Body::ApprovalDecided(body),
        );
        let seq = decided.seq;
        let mut events = vec![decided];
        if deny {
            events.extend(self.denied_by(at, by, &id, call_id, reason.as_deref()));
        } else {
            self.set_state(call_id, State::Approved { decided: seq });
        }
        self.accept(id, events.iter().map(|event| event.seq).collect());
        let mut actions = vec![Action::Append(events)];
        actions.extend(self.dispatch());
        actions
    }

    /// 人拒绝了：这个调用有了结果，记一条被拒绝的结果，`by` 是拒绝的人，`cause` 是回答的命令。
    /// 这一步因此齐了的，往下走。
    fn denied_by(
        &mut self,
        at: Timestamp,
        by: By,
        id: &CommandId,
        call_id: CallId,
        reason: Option<&str>,
    ) -> Vec<Event> {
        let finished = self.set_state(call_id, State::Done);
        let text = self.policy.tool_texts.denied(reason);
        let mut events =
            vec![self.written_result(at, by, Some(id.clone()), call_id, ToolStatus::Denied, text)];
        if finished {
            let cause = self.turn.as_ref().and_then(|turn| turn.cause.clone());
            events.extend(self.finish_step(at, cause));
        }
        events
    }

    /// 这一步里在等人回答的 `call_id`：它的请求提没提放行规则。不在等的，没有。
    fn asking(&self, call_id: CallId) -> Option<bool> {
        let Stage::Tools(step) = &self.turn.as_ref()?.stage else {
            return None;
        };
        step.calls
            .iter()
            .find(|call| call.id == call_id && call.state == State::Asking)
            .map(|call| call.asked.as_ref().is_some_and(|asked| asked.rule))
    }

    /// 把这一步里的 `call_id` 换成 `state`，返回这一步齐了没有。
    fn set_state(&mut self, call_id: CallId, state: State) -> bool {
        let Some(Stage::Tools(step)) = self.turn.as_mut().map(|turn| &mut turn.stage) else {
            return false;
        };
        if let Some(call) = step.calls.iter_mut().find(|call| call.id == call_id) {
            call.state = state;
        }
        step.finished()
    }
}
