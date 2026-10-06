//! 解码（`docs/blueprint/drivers/anthropic.md`「怎么走：解码」）：SSE 流解成内核的四种增量，说完时交出收块的增量、用量和
//! 出错。
//!
//! 一条事件照名字（`event`），没有名字的照 `data` 里的 `type`。块照第一次出现的先后编号，从 0 数起，不认的块（服务端工具
//! 这些）不占编号，它以后的增量也不理；线上的 `index` 只拿来找是哪一块。思考的签名、工具调用的编号写进私有数据，一块一份：
//! 来两次的只认第一次（不会发生；累积器一块只收一份）。`content_block_stop` 不理：块在收尾时一起收。

use std::collections::BTreeMap;
use std::mem;

use miyu_kernel::accumulate::{Delta, Kind};
use miyu_kernel::block::Private;
use miyu_kernel::event::{CallError, ErrorClass};
use miyu_kernel::id::DriverFamily;
use miyu_kernel::raw::RawJson;
use serde_json::Value;

use super::FAMILY;
use super::usage::Reported;
use crate::Ending;
use crate::classify::{Failure, classify};
use crate::sse::{Event, Sse};

/// 一次响应的解码器。
#[derive(Debug, Clone, Default)]
pub struct Decoder {
    sse: Sse,
    /// 线上的 `index` → 这一块：认得的有编号，不认的没有。
    blocks: BTreeMap<u64, Option<Opened>>,
    /// 下一块的编号。
    next: usize,
    /// 记下的用量。
    usage: Reported,
    /// 最后一次报的 `stop_reason`。
    stop_reason: Option<String>,
    /// 见到了 `message_stop`。
    stopped: bool,
    /// 流里报了错，或者流坏了。
    error: Option<CallError>,
    /// 流里报的错，供应商说要等多久。
    wait: Option<u64>,
}

/// 开了的一块。
#[derive(Debug, Clone, Copy)]
struct Opened {
    /// 编号。
    index: usize,
    /// 是思考块。
    thinking: bool,
    /// 私有数据交过了。
    private_sent: bool,
}

impl Decoder {
    /// 空的。
    pub fn new() -> Decoder {
        Decoder::default()
    }

    /// 喂一片字节，交回解出来的增量。见到 `message_stop` 或者出了错以后，再来的都不理。
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Delta> {
        let events = self.sse.feed(bytes);
        let mut out = Vec::new();
        for event in events {
            self.event(&event, false, &mut out);
        }
        out
    }

    /// 不用再读了：见到了 `message_stop`，或者出了错。
    pub fn done(&self) -> bool {
        self.stopped || self.error.is_some()
    }

    /// `stop_reason` 到了：模型说完了，只差 `message_stop`。
    pub fn finished(&self) -> bool {
        self.stop_reason.is_some()
    }

    /// 流完了，或者不再读了：交出收块的增量、用量、出错。
    pub fn finish(mut self) -> Ending {
        let mut deltas = Vec::new();
        // 流完了才冲刷出来的那一条没有以空行结束：它是断在半路上的。
        for event in mem::take(&mut self.sse).finish() {
            self.event(&event, true, &mut deltas);
        }
        let error = self.error.take().or_else(|| self.ending_error());
        if error.is_none() {
            let mut open: Vec<usize> = self
                .blocks
                .values()
                .flatten()
                .map(|block| block.index)
                .collect();
            open.sort_unstable();
            deltas.extend(open.into_iter().map(|index| Delta::End { index }));
        }
        Ending {
            deltas,
            usage: self.usage.usage(),
            error,
            retry_after_ms: self.wait,
        }
    }

    /// 照 `stop_reason` 和有没有见到 `message_stop`，看是不是正常说完的。
    fn ending_error(&self) -> Option<CallError> {
        let Some(reason) = &self.stop_reason else {
            return (!self.stopped).then(|| CallError {
                class: ErrorClass::Retryable,
                message: "流断了：没等到 stop_reason，也没等到 message_stop".to_string(),
                status: None,
            });
        };
        let class = match reason.as_str() {
            "end_turn"
            | "tool_use"
            | "stop_sequence"
            | "max_tokens"
            | "model_context_window_exceeded" => {
                return None;
            }
            "refusal" => ErrorClass::ContentPolicy,
            _ => ErrorClass::Unclassified,
        };
        Some(CallError {
            class,
            message: format!("stop_reason: {reason}"),
            status: None,
        })
    }

    /// 一条 SSE 事件。`cut` 的是流完了才冲刷出来的：解不开是流断在半路上，可重试。
    fn event(&mut self, event: &Event, cut: bool, out: &mut Vec<Delta>) {
        if self.done() {
            return;
        }
        let data = event.data.trim();
        if data.is_empty() {
            return;
        }
        let value: Value = match serde_json::from_str(data) {
            Ok(value) => value,
            Err(_) => {
                let (class, what) = match cut {
                    true => (ErrorClass::Retryable, "流断在半段 JSON 上"),
                    false => (ErrorClass::BadStream, "流里有一段不是 JSON"),
                };
                self.error = Some(CallError {
                    class,
                    message: format!("{what}：{}", clip(data)),
                    status: None,
                });
                return;
            }
        };
        let name = event
            .event
            .as_deref()
            .or_else(|| value.get("type").and_then(Value::as_str))
            .unwrap_or_default();
        match name {
            "message_start" => {
                if let Some(usage) = value.pointer("/message/usage") {
                    self.usage.update(usage);
                }
            }
            "content_block_start" => self.start(&value, out),
            "content_block_delta" => self.delta(&value, out),
            "message_delta" => {
                if let Some(reason) = value.pointer("/delta/stop_reason").and_then(Value::as_str) {
                    self.stop_reason = Some(reason.to_string());
                }
                if let Some(usage) = value.get("usage") {
                    self.usage.update(usage);
                }
            }
            "message_stop" => self.stopped = true,
            "error" => {
                let classified = classify(&Failure::stream(data.as_bytes()));
                self.error = Some(classified.error);
                self.wait = classified.retry_after_ms;
            }
            _ => {}
        }
    }

    /// `content_block_start`：照 `content_block.type` 开一块，别的种类记下不认。
    fn start(&mut self, value: &Value, out: &mut Vec<Delta>) {
        let Some(wire) = value.get("index").and_then(Value::as_u64) else {
            return;
        };
        if self.blocks.contains_key(&wire) {
            return;
        }
        let block = &value["content_block"];
        let field = |name: &str| block.get(name).and_then(Value::as_str).unwrap_or_default();
        let kind = match field("type") {
            "text" => Kind::Text,
            "thinking" | "redacted_thinking" => Kind::Reasoning,
            "tool_use" => Kind::ToolCall {
                name: field("name").to_string(),
            },
            _ => {
                self.blocks.insert(wire, None);
                return;
            }
        };
        let index = self.next;
        self.next += 1;
        let mut opened = Opened {
            index,
            thinking: kind == Kind::Reasoning,
            private_sent: false,
        };
        out.push(Delta::Start { index, kind });
        match field("type") {
            "text" => push_text(out, index, field("text")),
            "thinking" => {
                push_text(out, index, field("thinking"));
                opened.private(out, "signature", field("signature"));
            }
            "redacted_thinking" => opened.private(out, "redacted", field("data")),
            _ => opened.private(out, "id", field("id")),
        }
        self.blocks.insert(wire, Some(opened));
    }

    /// `content_block_delta`：字、思考、参数的一段交 `Text`，签名交私有数据；没开的块、不认的块、别的种类不理。
    fn delta(&mut self, value: &Value, out: &mut Vec<Delta>) {
        let Some(Some(opened)) = value
            .get("index")
            .and_then(Value::as_u64)
            .and_then(|wire| self.blocks.get_mut(&wire))
        else {
            return;
        };
        let delta = &value["delta"];
        let field = |name: &str| delta.get(name).and_then(Value::as_str).unwrap_or_default();
        match field("type") {
            "text_delta" => push_text(out, opened.index, field("text")),
            "thinking_delta" => push_text(out, opened.index, field("thinking")),
            "input_json_delta" => push_text(out, opened.index, field("partial_json")),
            "signature_delta" if opened.thinking => {
                opened.private(out, "signature", field("signature"));
            }
            _ => {}
        }
    }
}

impl Opened {
    /// 交这一块的私有数据 `{"<key>":<value>}`：空的不交，交过的不再交。
    fn private(&mut self, out: &mut Vec<Delta>, key: &str, value: &str) {
        if value.is_empty() || self.private_sent {
            return;
        }
        self.private_sent = true;
        out.push(Delta::Private {
            index: self.index,
            private: private(key, value),
        });
    }
}

/// 一段字：空的不交。
fn push_text(out: &mut Vec<Delta>, index: usize, text: &str) {
    if !text.is_empty() {
        out.push(Delta::Text {
            index,
            text: text.to_string(),
        });
    }
}

/// 私有数据 `{"<key>":"<value>"}`：编码时照它原样回传。
fn private(key: &str, value: &str) -> Private {
    let data =
        serde_json::to_string(&BTreeMap::from([(key, value)])).expect("两个字符串写得成 JSON");
    Private {
        driver: DriverFamily::parse(FAMILY).expect("驱动家族合写法"),
        data: serde_json::from_str::<RawJson>(&data).expect("刚写出来的是 JSON"),
    }
}

/// 报错里带上的原文，最长 200 个字。
fn clip(text: &str) -> String {
    text.chars().take(200).collect()
}
