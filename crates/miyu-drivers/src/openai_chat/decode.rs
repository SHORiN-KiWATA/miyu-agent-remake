//! 解码（`05-内核接口.md` 第七节「OpenAI 兼容的对话接口怎么解码」）：SSE 流解成内核的四种增量，
//! 说完时交出收块的增量、用量和出错。
//!
//! 一次响应最多一个正文块、一个思考块：正文、思考本来各是一整段，编码时照原样接回去。工具调用
//! 按 `index` 认，工具名到了才开始这一块，供应商的 `id` 写进私有数据。

use std::mem;

use miyu_kernel::accumulate::{Delta, Kind};
use miyu_kernel::block::Private;
use miyu_kernel::event::{CallError, ErrorClass, Usage};
use miyu_kernel::id::DriverFamily;
use miyu_kernel::raw::RawJson;
use serde::Deserialize;
use serde_json::Value;

use super::FAMILY;
use super::usage::usage;
use crate::Ending;
use crate::classify::{Failure, classify};
use crate::sse::{Event, Sse};

/// 一次响应的解码器。
#[derive(Debug, Clone, Default)]
pub struct Decoder {
    sse: Sse,
    /// 下一块的编号。
    next: usize,
    /// 正文块的编号。
    text: Option<usize>,
    /// 思考块的编号。
    reasoning: Option<usize>,
    /// 工具调用，照第一次出现的先后。
    calls: Vec<Call>,
    /// 最后一次报的用量。
    usage: Option<Usage>,
    /// 最后一次报的 `finish_reason`。
    finish_reason: Option<String>,
    /// 见到了 `[DONE]`。
    done: bool,
    /// 流里报了错，或者流坏了。
    error: Option<CallError>,
}

/// 一次工具调用解到哪了。
#[derive(Debug, Clone, Default)]
struct Call {
    /// 供应商给的 `index`。
    index: Option<u64>,
    /// 供应商自己的编号。
    id: Option<String>,
    /// 工具名。
    name: Option<String>,
    /// 这一块的编号：工具名到了才开始。
    block: Option<usize>,
    /// 还没开始时攒着的参数。
    pending: String,
    /// 编号写进私有数据了。
    private_sent: bool,
}

impl Decoder {
    /// 空的。
    pub fn new() -> Decoder {
        Decoder::default()
    }

    /// 喂一片字节，交回解出来的增量。见到 `[DONE]` 或者出了错以后，再来的都不理。
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Delta> {
        let events = self.sse.feed(bytes);
        let mut out = Vec::new();
        for event in events {
            self.event(event, false, &mut out);
        }
        out
    }

    /// 不用再读了：见到了 `[DONE]`，或者出了错。
    pub fn done(&self) -> bool {
        self.done || self.error.is_some()
    }

    /// 流完了，或者不再读了：交出收块的增量、用量、出错。
    pub fn finish(mut self) -> Ending {
        let mut deltas = Vec::new();
        // 流完了才冲刷出来的那一条没有以空行结束：它是断在半路上的。
        for event in mem::take(&mut self.sse).finish() {
            self.event(event, true, &mut deltas);
        }
        let error = self.error.take().or_else(|| self.ending_error());
        if error.is_none() {
            let mut open: Vec<usize> = self.text.into_iter().chain(self.reasoning).collect();
            open.extend(self.calls.iter().filter_map(|call| call.block));
            open.sort_unstable();
            deltas.extend(open.into_iter().map(|index| Delta::End { index }));
        }
        Ending {
            deltas,
            usage: self.usage,
            error,
        }
    }

    /// 照 `finish_reason` 和有没有见到 `[DONE]`，看是不是正常说完的。
    fn ending_error(&self) -> Option<CallError> {
        let Some(reason) = &self.finish_reason else {
            return (!self.done).then(|| CallError {
                class: ErrorClass::Retryable,
                message: "流断了：没等到 finish_reason，也没等到 [DONE]".to_string(),
            });
        };
        let class = match reason.as_str() {
            "stop" | "tool_calls" | "function_call" | "length" | "end" => return None,
            "content_filter" => ErrorClass::ContentPolicy,
            "network_error" => ErrorClass::Retryable,
            _ => ErrorClass::Unclassified,
        };
        Some(CallError {
            class,
            message: format!("finish_reason: {reason}"),
        })
    }

    /// 一条 SSE 事件。`cut` 的是流完了才冲刷出来的：解不开是流断在半路上，可重试。
    fn event(&mut self, event: Event, cut: bool, out: &mut Vec<Delta>) {
        if self.done() {
            return;
        }
        let data = event.data.trim();
        if data == "[DONE]" {
            self.done = true;
            return;
        }
        if data.is_empty() {
            return;
        }
        if event.event.as_deref() == Some("error") {
            self.error = Some(classify(&Failure::stream(data.as_bytes())).error);
            return;
        }
        match serde_json::from_str::<Chunk>(data) {
            Ok(chunk) => self.chunk(chunk, data, out),
            Err(_) if cut => {
                self.error = Some(CallError {
                    class: ErrorClass::Retryable,
                    message: format!("流断在半段 JSON 上：{}", clip(data)),
                });
            }
            Err(_) => {
                self.error = Some(CallError {
                    class: ErrorClass::BadStream,
                    message: format!("流里有一段不是 JSON：{}", clip(data)),
                });
            }
        }
    }

    /// 一段 JSON。
    fn chunk(&mut self, chunk: Chunk, data: &str, out: &mut Vec<Delta>) {
        if chunk.error.as_ref().is_some_and(has_content) {
            self.error = Some(classify(&Failure::stream(data.as_bytes())).error);
            return;
        }
        if let Some(found) = chunk.usage.as_ref().and_then(usage) {
            self.usage = Some(found);
        }
        let Some(choice) = chunk.choices.and_then(|choices| choices.into_iter().next()) else {
            return;
        };
        if let Some(found) = choice.usage.as_ref().and_then(usage) {
            self.usage = Some(found);
        }
        if let Some(delta) = choice.delta {
            let reasoning = [
                delta.reasoning_content,
                delta.reasoning,
                delta.reasoning_text,
            ]
            .into_iter()
            .flatten()
            .find(|text| !text.is_empty());
            if let Some(text) = reasoning {
                let index = open(&mut self.reasoning, &mut self.next, Kind::Reasoning, out);
                out.push(Delta::Text { index, text });
            }
            if let Some(text) = delta.content.filter(|text| !text.is_empty()) {
                let index = open(&mut self.text, &mut self.next, Kind::Text, out);
                out.push(Delta::Text { index, text });
            }
            for piece in delta.tool_calls.into_iter().flatten() {
                self.tool_call(piece, out);
            }
        }
        if let Some(reason) = choice.finish_reason.filter(|reason| !reason.is_empty()) {
            self.finish_reason = Some(reason);
        }
    }

    /// 工具调用的一片。
    fn tool_call(&mut self, piece: ToolCallDelta, out: &mut Vec<Delta>) {
        let id = piece.id.filter(|id| !id.is_empty());
        let slot = self.slot(piece.index, id.as_deref());
        let call = &mut self.calls[slot];
        if call.id.is_none() {
            call.id = id;
        }
        if let Some(function) = piece.function {
            if call.name.is_none() {
                call.name = function.name.filter(|name| !name.is_empty());
            }
            if let Some(arguments) = function.arguments {
                call.pending.push_str(&arguments);
            }
        }
        if call.block.is_none()
            && let Some(name) = call.name.clone()
        {
            let index = self.next;
            self.next += 1;
            call.block = Some(index);
            out.push(Delta::Start {
                index,
                kind: Kind::ToolCall { name },
            });
        }
        let Some(index) = call.block else {
            return;
        };
        if !call.private_sent
            && let Some(id) = &call.id
        {
            call.private_sent = true;
            out.push(Delta::Private {
                index,
                private: private_id(id),
            });
        }
        if !call.pending.is_empty() {
            out.push(Delta::Text {
                index,
                text: mem::take(&mut call.pending),
            });
        }
    }

    /// 这一片属于哪一次调用：先按 `index`，没有再按 `id`，都没有就是最近的那一次。认不出的，
    /// 是新的一次。
    fn slot(&mut self, index: Option<u64>, id: Option<&str>) -> usize {
        let found = match (index, id) {
            (Some(index), _) => self.calls.iter().position(|call| call.index == Some(index)),
            (None, Some(id)) => self
                .calls
                .iter()
                .position(|call| call.id.as_deref() == Some(id)),
            (None, None) => self.calls.len().checked_sub(1),
        };
        found.unwrap_or_else(|| {
            self.calls.push(Call {
                index,
                ..Call::default()
            });
            self.calls.len() - 1
        })
    }
}

/// 没开始的块开始：交回它的编号。
fn open(slot: &mut Option<usize>, next: &mut usize, kind: Kind, out: &mut Vec<Delta>) -> usize {
    if let Some(index) = *slot {
        return index;
    }
    let index = *next;
    *next += 1;
    *slot = Some(index);
    out.push(Delta::Start { index, kind });
    index
}

/// 私有数据 `{"id":"…"}`：编码时照它把供应商的编号用回去。
fn private_id(id: &str) -> Private {
    let data = serde_json::to_string(&IdOnly { id }).expect("一个字符串写得成 JSON");
    Private {
        driver: DriverFamily::parse(FAMILY).expect("驱动家族合写法"),
        data: serde_json::from_str::<RawJson>(&data).expect("刚写出来的是 JSON"),
    }
}

#[derive(serde::Serialize)]
struct IdOnly<'a> {
    id: &'a str,
}

/// `error` 是不是真有内容：`{"error":""}`、`{"error":{}}`、`null` 是网关的噪声。
fn has_content(error: &Value) -> bool {
    match error {
        Value::Null => false,
        Value::String(text) => !text.is_empty(),
        Value::Object(map) => !map.is_empty(),
        _ => true,
    }
}

/// 报错里带上的原文，最长 200 个字。
fn clip(text: &str) -> String {
    text.chars().take(200).collect()
}

/// 流里的一段。
#[derive(Debug, Deserialize)]
struct Chunk {
    choices: Option<Vec<Choice>>,
    usage: Option<Value>,
    error: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    delta: Option<ChunkDelta>,
    finish_reason: Option<String>,
    /// Moonshot 把用量放在这里。
    usage: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    content: Option<String>,
    reasoning_content: Option<String>,
    reasoning: Option<String>,
    reasoning_text: Option<String>,
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct ToolCallDelta {
    index: Option<u64>,
    id: Option<String>,
    function: Option<FunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct FunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}
