//! 看守查提问（`docs/designs/02-内核.md` 第六节「提问怎么走」）：
//!
//! - 题目只由在跑的调用问，`by` 是那次调用，写的就是它交上来的；没人能回答的不记，当场跳过，
//!   `by` 是内核；
//! - 回答：看守自己照规矩判该接受还是拒绝；接受的记一条回答，`by` 是回答的人；
//! - 交给工具的，是那条已经落了盘的回答，只交一次；
//! - 等人回答时来了一句话，问着人的补「没回答：你发了一句话」，`by` 是说话的人；打断时问着人的
//!   补「没回答：被打断了」；两样都叫执行器停下。

use super::approval::text_of;
use super::*;
use crate::event::{Question, Response, fits};

/// 看守替执行器记着的提问：问着人的、答完了还没交给工具的、这一次说话的人。
pub(in super::super) struct Questions {
    /// 问着人的调用，和那组题。
    pub(in super::super) asking: BTreeMap<CallId, Vec<Question>>,
    /// 答完了、还没交给工具的调用：那条回答的序号和回答。
    answered: BTreeMap<CallId, (Seq, Vec<Response>)>,
    /// 这一次送进去的是回合里的一句话：说话的人。
    speaking: Option<By>,
}

impl Questions {
    pub(super) fn new() -> Questions {
        Questions {
            asking: BTreeMap::new(),
            answered: BTreeMap::new(),
            speaking: None,
        }
    }
}

impl Watch {
    /// 送进一条输入之前：新的一句话，记下说话的人；新的回答，照规矩判出该接受还是拒绝。
    pub(super) fn before_question(&mut self, input: &Input) -> Option<Result<(), Reason>> {
        self.questions.speaking = None;
        let Input::Command(received) = input else {
            return None;
        };
        if !self.fresh(&received.id) {
            return None;
        }
        match &received.command {
            Command::Send { blocks, .. } if !blocks.is_empty() => {
                self.questions.speaking = Some(received.by.clone());
                None
            }
            Command::Answer {
                call_id,
                answer: Answer::Questions(answers),
            } => Some(match self.questions.asking.get(call_id) {
                None => Err(Reason::NotAsking),
                Some(questions) if !fits(questions, answers) => Err(Reason::BadAnswer),
                Some(_) => Ok(()),
            }),
            _ => None,
        }
    }

    /// 送进去以后：新的回答，照判出来的查回应。拒绝的只有一个回应；接受的记了一条回答。
    pub(super) fn after_question(
        &mut self,
        actions: &[Action],
        judged: Option<Result<(), Reason>>,
    ) {
        let seed = self.seed;
        match judged {
            None => {}
            Some(Err(reason)) => {
                if reason == Reason::BadAnswer {
                    self.seen_paths.insert("回答对不上被拒");
                }
                assert!(
                    matches!(actions, [Action::Reply { outcome: Outcome::Rejected { reason: got }, .. }] if *got == reason),
                    "种子 {seed}：这个回答应该被拒，原因码 {}：{actions:?}",
                    reason.code()
                );
            }
            Some(Ok(())) => assert!(
                actions
                    .iter()
                    .any(|action| matches!(action, Action::Append(events)
                    if events.iter().any(|event| matches!(event.body, Body::QuestionAnswered(_))))),
                "种子 {seed}：这个回答应该记一条回答：{actions:?}"
            ),
        }
    }

    /// 把回答交给工具：是那条已经落了盘的回答，只交一次。
    pub(super) fn handed(&mut self, call_id: CallId, answers: &[Response]) {
        let seed = self.seed;
        self.seen_paths.insert("回答交给了工具");
        let Some((seq, given)) = self.questions.answered.remove(&call_id) else {
            panic!("种子 {seed}：{call_id} 没答过，却交给了工具");
        };
        assert!(
            self.pushed.contains(&seq),
            "种子 {seed}：{call_id} 的回答还没落盘就交给了工具"
        );
        assert_eq!(given, answers, "种子 {seed}：交给工具的不是那条回答");
    }

    /// 在跑的调用被跳过，只有两种：问着人的时候来了一句话，没人能回答。
    pub(super) fn skips_running(&self, event: &Event, call_id: CallId) -> bool {
        self.questions.asking.contains_key(&call_id) || text_of(event) == "question unattended"
    }

    /// 这一批里的第 `k` 条，照提问的规矩查。
    pub(super) fn question_check(&mut self, events: &[Event], k: usize) {
        let seed = self.seed;
        let event = &events[k];
        match &event.body {
            Body::QuestionAsked(asked) => {
                self.seen_paths.insert("工具问人");
                let call_id = asked.call_id;
                assert_eq!(event.by, By::Tool(crate::origin::Tool { call_id }));
                assert!(
                    self.approvals.attended,
                    "种子 {seed}：没人能回答，不该记题目"
                );
                assert!(
                    self.questions
                        .asking
                        .insert(call_id, asked.questions.clone())
                        .is_none(),
                    "种子 {seed}：{call_id} 已经有一组在等的题"
                );
            }
            Body::QuestionAnswered(answered) => {
                self.seen_paths.insert("人回答了");
                let call_id = answered.call_id;
                assert!(
                    self.questions.asking.remove(&call_id).is_some(),
                    "种子 {seed}：{call_id} 不在等人回答，却有了回答"
                );
                assert!(
                    matches!(event.by, By::Person(_)),
                    "种子 {seed}：回答是人给的"
                );
                self.questions
                    .answered
                    .insert(call_id, (event.seq, answered.answers.clone()));
            }
            Body::ToolResult(result) => {
                let call_id = result.call_id;
                self.questions.answered.remove(&call_id);
                if text_of(event) == "question unattended" {
                    self.seen_paths.insert("没人能回答");
                    assert!(
                        !self.approvals.attended,
                        "种子 {seed}：有人能回答，却说没人"
                    );
                    assert_eq!(event.by, By::Kernel);
                }
                if self.questions.asking.remove(&call_id).is_none() {
                    return;
                }
                match text_of(event) {
                    "question voided" => {
                        self.seen_paths.insert("来了一句话作废");
                        assert_eq!(
                            Some(&event.by),
                            self.questions.speaking.as_ref(),
                            "种子 {seed}：作废的是说话的人"
                        );
                    }
                    "restarted" => {}
                    "question interrupted" => {
                        self.seen_paths.insert("打断时在等人回答");
                        assert!(
                            self.interrupting.is_some(),
                            "种子 {seed}：不是打断，却说被打断了"
                        );
                    }
                    _ => assert!(
                        matches!(event.by, By::Tool(_)),
                        "种子 {seed}：{call_id} 问着人时的结果不对：{event:?}"
                    ),
                }
            }
            _ => {}
        }
    }
}
