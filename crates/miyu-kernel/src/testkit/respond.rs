//! 替身怎么回动作（`docs/designs/02-内核.md` 第四节「执行器怎么回动作」）：照那张表，一个动作一个
//! 动作地回，返回要送回会话的输入。

use super::Stage;
use super::script::{Line, Play};
use crate::accumulate::{Delta, Kind};
use crate::block::{Block, Text};
use crate::event::{Effect, FileRead, JobMessaged, Response, Usage};
use crate::id::{CallId, ContentHash, ModelName, ProviderId, Seq};
use crate::origin::Model;
use crate::request::{Message, Request};
use crate::session::{Action, Input, Verdict};
use crate::time::Timestamp;

impl Stage {
    /// 照 02 第四节的表回一个动作，返回要送回去的输入。
    pub(super) fn act(&mut self, action: Action) -> Vec<Input> {
        match action {
            Action::Append(events) => {
                let upto = events.last().map(|event| event.seq);
                self.log.extend(events);
                upto.map(|upto| Input::Stored { at: self.now, upto })
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
            Action::CallModel { seen, request, .. } => self.call(seen, request),
            // 回顾（施工 3-8 四补）、起标题（五补）：照各自的剧本回，`aside.rs`。
            Action::Aside {
                purpose,
                upto,
                request,
            } => self.aside_call(purpose, upto, request),
            Action::Wake { at, seen } if self.hold_wakes => {
                self.held_wake = Some((at, seen));
                Vec::new()
            }
            Action::Wake { at, seen } => self.wake(at, seen),
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
            // 叫它停（施工 4-9 再补一）：替身记下来，停住的调用照旧停着，等测试定它怎么交回。
            Action::StopTool { call_id } => {
                self.stops.push(call_id);
                Vec::new()
            }
            Action::CancelTool { call_id } => {
                self.held_tools.retain(|(held, _)| *held != call_id);
                Vec::new()
            }
            // 压完重读（施工 6-5）：照「磁盘」回。
            Action::Reread { seen, paths, limit } => self.reread(seen, paths, limit),
            // 读回日志、取回原文（施工 6-9）：照「磁盘」回。
            Action::ReadBack { from } => self.read_back(from),
            Action::Recall { blobs } => self.recall(blobs),
            // 向上回报（施工 7-6）：替身记下来，不送回。
            Action::Report(upward) => {
                self.upward.push(upward);
                Vec::new()
            }
            // 停掉撤掉的那几轮派出去的（施工 7-8）：替身记下来，不送回；停好了的回报由测试交。
            stop @ Action::StopJobs { .. } => {
                self.stopping.push(stop);
                Vec::new()
            }
            // 改回文件（施工 4-7 上）：替身不碰文件，每一步都当改回了。
            Action::Restore { steps } => vec![Input::Restored {
                at: self.tick(),
                files: steps.iter().map(super::restored).collect(),
            }],
        }
    }

    /// 到点叫醒：时钟拨到那一刻（已经过了的，就是现在），送回到点了。
    pub(super) fn wake(&mut self, at: Timestamp, seen: Seq) -> Vec<Input> {
        if at > self.now {
            self.now = at;
        }
        vec![Input::Woke { at: self.now, seen }]
    }

    /// 请求模型：先报发出去了，再一块块送增量；不停住的，最后送说完了。
    fn call(&mut self, seen: Seq, request: Request) -> Vec<Input> {
        let hash = request.hash();
        let summary = self.summary_line(&request);
        self.marks
            .push(self.log.last().map_or(seen, |event| event.seq));
        self.requests.push((seen, request));
        let line = summary
            .or_else(|| self.lines.pop_front())
            .unwrap_or_else(|| {
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
        if line.error.is_none() || line.says_something() {
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

    /// 是摘要请求（最后一块是 [`Stage::summarize_with`] 给的指令）的，照给的那一句回。
    fn summary_line(&self, request: &Request) -> Option<Line> {
        let (instruction, line) = self.summaries.as_ref()?;
        let last = match request.messages.last()? {
            Message::User { blocks } => blocks.last()?,
            _ => return None,
        };
        matches!(last, Block::Text(text) if text.text == *instruction).then(|| line.clone())
    }

    /// 请求 `seen` 说完了：出错的带上分类和原话，说完了的带上用量。
    pub(super) fn ended(&mut self, seen: Seq, line: &Line) -> Input {
        Input::ModelEnded {
            at: self.tick(),
            seen,
            usage: line.error.is_none().then(|| {
                line.usage.unwrap_or(Usage {
                    uncached: 100,
                    cache_read: 0,
                    cache_write: 0,
                    output: 10,
                })
            }),
            error: line.error.clone(),
            wait_ms: line.wait_ms,
            excess: line.excess,
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
            Play::Read { path, text } => {
                let mut done = self.done(call_id, false, &text);
                if let Input::ToolDone { effects, .. } = &mut done {
                    effects.push(Effect::FileRead(FileRead {
                        path,
                        lines: Some([1, text.lines().count().max(1) as u64]),
                        hash: ContentHash::of(text.as_bytes()),
                    }));
                }
                vec![done]
            }
            Play::Starts { text, started } => {
                let mut done = self.done(call_id, false, &text);
                if let Input::ToolDone { effects, .. } = &mut done {
                    effects.push(Effect::JobStarted(started));
                }
                vec![done]
            }
            Play::Messages(job) => {
                let mut done = self.done(call_id, false, &format!("Message sent to {job}."));
                if let Input::ToolDone { effects, .. } = &mut done {
                    effects.push(Effect::JobMessaged(JobMessaged { job }));
                }
                vec![done]
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
            human: None,
            effects: Vec::new(),
            stopped: false,
        }
    }

    /// 调用 `call_id` 叫它停以后停在了改之前（施工 4-9 再补一）。
    pub(super) fn stopped(&mut self, call_id: CallId) -> Input {
        Input::ToolDone {
            at: self.tick(),
            call_id,
            error: false,
            blocks: Vec::new(),
            duration_ms: Some(5),
            human: None,
            effects: Vec::new(),
            stopped: true,
        }
    }
}

/// 替身的模型：deepseek 的 deepseek-v4。
///
/// # Panics
///
/// 实际不会：两个名字都合写法。
pub fn model() -> Model {
    Model {
        endpoint: ProviderId::parse("deepseek").unwrap_or_else(|e| panic!("{e}")),
        model: ModelName::parse("deepseek-v4").unwrap_or_else(|e| panic!("{e}")),
    }
}

/// 一次回复的增量：正文一块，每个调用一块，每块一次送完（开始、全文、收全）。
pub(super) fn deltas(line: &Line) -> Vec<Delta> {
    // 说了一半断了的，一块都不收全。
    let ends = line.error.is_none();
    let mut deltas = Vec::new();
    let mut index = 0;
    if !line.reasoning.is_empty() {
        deltas.extend(block(index, Kind::Reasoning, &line.reasoning, ends));
        index += 1;
    }
    if !line.text.is_empty() {
        deltas.extend(block(index, Kind::Text, &line.text, ends));
        index += 1;
    }
    for (name, args) in &line.calls {
        let kind = Kind::ToolCall { name: name.clone() };
        deltas.extend(block(index, kind, args, ends));
        index += 1;
    }
    deltas
}

/// 第 `index` 块：开始、全文；`ends` 的再收全。
fn block(index: usize, kind: Kind, text: &str, ends: bool) -> Vec<Delta> {
    let mut deltas = vec![
        Delta::Start { index, kind },
        Delta::Text {
            index,
            text: text.to_string(),
        },
    ];
    if ends {
        deltas.push(Delta::End { index });
    }
    deltas
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
