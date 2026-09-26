//! 提问（`docs/designs/02-内核.md` 第六节「提问怎么走」）：在跑的工具问人一组题，人回答，
//! 回答落了盘交给工具；等人回答的时候来了一句话，等的那个作废。
//!
//! 不回答就是打断：头上的「不回答」发的是 `session.interrupt`，在 [`super::interrupt`]。

use super::action::{Action, Reason};
use super::step::{Pending, State};
use super::tools::settle;
use super::turn::Stage;
use super::{Session, rejected};
use crate::event::{
    Body, Event, Question, QuestionAnswered, QuestionAsked, Response, ToolStatus, fits,
};
use crate::id::{CallId, CommandId};
use crate::origin::{By, Tool};
use crate::time::Timestamp;

impl Session {
    /// 在跑的调用问人一组题：记一条 `question.asked`，`by` 是那次调用，这个调用停下来等人。
    /// 没人能回答的会话，不记题目，叫执行器停下这个调用，当场记一条「已跳过」，`by` 是内核，
    /// 写明这里没有人能回答。不是这一步在跑的调用问的，不理。
    pub(super) fn tool_asks(
        &mut self,
        at: Timestamp,
        call_id: CallId,
        questions: Vec<Question>,
    ) -> Vec<Action> {
        let attended = self.policy.attended;
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        let cause = turn.cause.clone();
        let Stage::Tools(step) = &mut turn.stage else {
            return Vec::new();
        };
        let Some(call) = step.find(call_id, State::Running) else {
            return Vec::new();
        };
        if !attended {
            call.state = State::Done;
            let finished = step.finished();
            let text = self.policy.tool_texts.question_unattended();
            let mut events = vec![self.written_result(
                at,
                By::Kernel,
                cause.clone(),
                call_id,
                ToolStatus::Skipped,
                text,
            )];
            if finished {
                events.extend(self.finish_step(at, cause));
            }
            let mut actions = vec![Action::Append(events), Action::CancelTool { call_id }];
            actions.extend(self.dispatch());
            return actions;
        }
        call.state = State::Questioning;
        call.questions = questions.clone();
        let body = Body::QuestionAsked(QuestionAsked { call_id, questions });
        let by = By::Tool(Tool { call_id });
        vec![Action::Append(vec![self.record(at, by, cause, body)])]
    }

    /// 人回答了一组题：记一条 `question.answered`，`by` 是回答的人，`cause` 是这个命令；这一条
    /// 落了盘，把回答交给工具。不在等人回答的（没问过、答过了、已经有了结果，或者等的是确认），
    /// 拒绝，原因码 `not_asking`；对不上题目的，拒绝，原因码 `bad_answer`。空的自己写当没写。
    pub(super) fn answer_question(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        call_id: CallId,
        answers: Vec<Response>,
    ) -> Vec<Action> {
        let answers: Vec<Response> = answers
            .into_iter()
            .map(|answer| Response {
                text: answer.text.filter(|text| !text.trim().is_empty()),
                ..answer
            })
            .collect();
        let Some(call) = self.questioning(call_id) else {
            return vec![rejected(id, Reason::NotAsking)];
        };
        if !fits(&call.questions, &answers) {
            return vec![rejected(id, Reason::BadAnswer)];
        }
        let body = Body::QuestionAnswered(QuestionAnswered {
            call_id,
            answers: answers.clone(),
        });
        let answered = self.record(at, by, Some(id.clone()), body);
        if let Some(call) = self.questioning(call_id) {
            call.state = State::Answered {
                answered: answered.seq,
            };
            call.answers = answers;
        }
        self.accept(id, vec![answered.seq]);
        vec![Action::Append(vec![answered])]
    }

    /// 等人回答的时候来了一句话：在等人回答的都作废，`by` 是说话的人，`cause` 是这句话的命令。
    /// 问着人的叫执行器停下，补「没回答：你发了一句话」；等确认的补「已跳过」。这一步因此齐了的，
    /// 往下走。返回追加的事件，和叫停的动作。
    pub(super) fn void_waiting(
        &mut self,
        at: Timestamp,
        by: &By,
        cause: &CommandId,
    ) -> (Vec<Event>, Vec<Action>) {
        let Some(turn) = self.turn.as_mut() else {
            return (Vec::new(), Vec::new());
        };
        let turn_cause = turn.cause.clone();
        let Stage::Tools(step) = &mut turn.stage else {
            return (Vec::new(), Vec::new());
        };
        let questioning: Vec<CallId> = step
            .calls
            .iter()
            .filter(|call| call.state == State::Questioning)
            .map(|call| call.id)
            .collect();
        let voided = settle(step, Pending::awaiting_person);
        let finished = step.finished();
        let texts = &self.policy.tool_texts;
        let (voided_question, skipped) = (texts.question_voided(), texts.skipped());
        let mut events: Vec<Event> = voided
            .into_iter()
            .map(|call_id| {
                let text = match questioning.contains(&call_id) {
                    true => voided_question.clone(),
                    false => skipped.clone(),
                };
                self.written_result(
                    at,
                    by.clone(),
                    Some(cause.clone()),
                    call_id,
                    ToolStatus::Skipped,
                    text,
                )
            })
            .collect();
        if finished && !events.is_empty() {
            events.extend(self.finish_step(at, turn_cause));
        }
        let stops = questioning
            .into_iter()
            .map(|call_id| Action::CancelTool { call_id })
            .collect();
        (events, stops)
    }

    /// 这一步里在等人回答那组题的 `call_id`。
    fn questioning(&mut self, call_id: CallId) -> Option<&mut Pending> {
        let Stage::Tools(step) = &mut self.turn.as_mut()?.stage else {
            return None;
        };
        step.find(call_id, State::Questioning)
    }
}
