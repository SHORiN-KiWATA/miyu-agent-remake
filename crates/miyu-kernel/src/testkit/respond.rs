//! 替身怎么回动作（`docs/designs/02-内核.md` 第四节「执行器怎么回动作」）：照那张表，一个动作一个
//! 动作地回，返回要送回会话的输入。

use super::Stage;
use super::script::{Line, Play};
use crate::accumulate::{Delta, Kind};
use crate::block::{Block, Text};
use crate::event::{Response, Usage};
use crate::id::{CallId, ModelName, ProviderId, Seq};
use crate::origin::Model;
use crate::request::Request;
use crate::session::{Action, Input, Verdict};

impl Stage {
    /// 照 02 第四节的表回一个动作，返回要送回去的输入。
    pub(super) fn act(&mut self, action: Action) -> Vec<Input> {
        match action {
            Action::Append(events) => {
                let upto = events.last().map(|event| event.seq);
                self.log.extend(events);
                upto.map(|upto| Input::Stored { upto })
                    .into_iter()
                    .collect()
            }
            Action::Reply { id, outcome } => {
                self.replies.push((id, outcome));
                Vec::new()
            }
            Action::Push(_) | Action::RunTurnEndHooks { .. } => Vec::new(),
            Action::PushTransient(transient) => {
                self.transients.push(transient);
                Vec::new()
            }
            Action::RunTurnStartHooks { turn } => vec![Input::TurnStartHooksDone {
                at: self.tick(),
                turn,
                injected: self.injections.pop_front().unwrap_or_default(),
            }],
            Action::CallModel { seen, request } => self.call(seen, request),
            Action::CancelModel { seen } => {
                self.held_model.take_if(|(held, _)| *held == seen);
                Vec::new()
            }
            Action::GuardTool { call_id, .. } => vec![Input::ToolGuarded {
                at: self.tick(),
                call_id,
                verdict: self.verdicts.pop_front().unwrap_or(Verdict::Allow),
            }],
            Action::RunTool {
                call_id,
                name,
                args,
                ..
            } => {
                self.ran.push((call_id, name, args));
                let play = self
                    .plays
                    .pop_front()
                    .unwrap_or_else(|| panic!("剧本里没排 {call_id} 怎么回"));
                self.play(call_id, play)
            }
            Action::AnswerTool { call_id, answers } => {
                vec![self.done(call_id, false, &answered(&answers))]
            }
            Action::CancelTool { call_id } => {
                self.held_tools.retain(|(held, _)| *held != call_id);
                Vec::new()
            }
        }
    }

    /// 请求模型：先报发出去了，再一块块送增量；不停住的，最后送说完了。
    fn call(&mut self, seen: Seq, request: Request) -> Vec<Input> {
        let hash = request.hash();
        self.requests.push((seen, request));
        let line = self.lines.pop_front().unwrap_or_else(|| {
            panic!(
                "剧本里没排第 {} 次请求模型说什么（seen {seen}）",
                self.requests.len()
            )
        });
        let mut inputs = vec![Input::RequestSent {
            at: self.tick(),
            seen,
            model: model(),
            request: hash,
        }];
        if line.error.is_none() {
            for delta in deltas(&line) {
                inputs.push(Input::ModelDelta {
                    at: self.tick(),
                    seen,
                    delta,
                });
            }
        }
        if line.hold {
            self.held_model = Some((seen, line));
        } else {
            inputs.push(self.ended(seen, &line));
        }
        inputs
    }

    /// 请求 `seen` 说完了：出错的带上分类和原话，说完了的带上用量。
    pub(super) fn ended(&mut self, seen: Seq, line: &Line) -> Input {
        Input::ModelEnded {
            at: self.tick(),
            seen,
            usage: line.error.is_none().then_some(Usage {
                uncached: 100,
                cache_read: 0,
                cache_write: 0,
                output: 10,
            }),
            error: line.error.clone(),
        }
    }

    /// 照排好的回一次调用。
    pub(super) fn play(&mut self, call_id: CallId, play: Play) -> Vec<Input> {
        match play {
            Play::Done(text) => vec![self.done(call_id, false, &text)],
            Play::Fails(text) => vec![self.done(call_id, true, &text)],
            Play::Asks(questions) => vec![Input::ToolAsks {
                at: self.tick(),
                call_id,
                questions,
            }],
            Play::Held(play) => {
                self.held_tools.push((call_id, *play));
                Vec::new()
            }
        }
    }

    /// 调用 `call_id` 执行完了。
    fn done(&mut self, call_id: CallId, error: bool, text: &str) -> Input {
        Input::ToolDone {
            at: self.tick(),
            call_id,
            error,
            blocks: vec![Block::Text(Text {
                text: text.to_string(),
            })],
            duration_ms: Some(5),
        }
    }
}

/// 替身的模型：deepseek 的 deepseek-v4。
fn model() -> Model {
    Model {
        endpoint: ProviderId::parse("deepseek").unwrap_or_else(|e| panic!("{e}")),
        model: ModelName::parse("deepseek-v4").unwrap_or_else(|e| panic!("{e}")),
    }
}

/// 一次回复的增量：正文一块，每个调用一块，每块一次送完（开始、全文、收全）。
fn deltas(line: &Line) -> Vec<Delta> {
    let mut deltas = Vec::new();
    let mut index = 0;
    if !line.text.is_empty() {
        deltas.extend(block(index, Kind::Text, &line.text));
        index += 1;
    }
    for (name, args) in &line.calls {
        let kind = Kind::ToolCall { name: name.clone() };
        deltas.extend(block(index, kind, args));
        index += 1;
    }
    deltas
}

/// 第 `index` 块：开始、全文、收全。
fn block(index: usize, kind: Kind, text: &str) -> [Delta; 3] {
    [
        Delta::Start { index, kind },
        Delta::Text {
            index,
            text: text.to_string(),
        },
        Delta::End { index },
    ]
}

/// `ask_user` 拿到回答以后写的结果（真的写法随 M4 的 `ask_user` 定）：照题目的先后，选了的写标题，
/// 自己写的照原文。
fn answered(answers: &[Response]) -> String {
    let parts: Vec<String> = answers
        .iter()
        .map(|answer| {
            let mut said = answer.picked.clone();
            said.extend(answer.text.clone());
            said.join(", ")
        })
        .collect();
    format!("The user answered: {}", parts.join("; "))
}
