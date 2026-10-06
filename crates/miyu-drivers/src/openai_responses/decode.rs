//! 解码（`docs/blueprint/drivers/openai-responses.md`「怎么走：解码」）：SSE 流解成内核的四种增量，说完时交出收块的增量、
//! 用量和出错。
//!
//! 一项开一块，照项第一次出现的先后编号，从 0 数起；线上的 `output_index` 只拿来找是哪一块，不认的项（服务端工具这些）不占
//! 编号。这一家没有 `[DONE]`：说完是 `response.completed`、`response.incomplete`、`response.failed`。一项里一个字都没流过来的
//! （有的网关只在 `response.output_item.done` 给整段），照那里的整段补上。
//!
//! 工具调用的参数不照增量交，攒到这一项完了照整段交：先照 `output_item.done` 的 `arguments`，再照
//! `function_call_arguments.done` 的，都没有的照攒的增量。2026-10-03 实测，有的中转站流过来的增量丢了开头的 `{"`，整段是
//! 对的；没收全的调用本来就执行不了（被打断的只留收全了的），晚一点交不丢东西。

use std::collections::BTreeMap;
use std::mem;

use miyu_kernel::accumulate::{Delta, Kind};
use miyu_kernel::block::Private;
use miyu_kernel::event::{CallError, ErrorClass, Usage};
use miyu_kernel::id::DriverFamily;
use miyu_kernel::raw::RawJson;
use serde::Serialize;
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
    /// 线上的 `output_index` → 这一块：认得的有，不认的没有。
    blocks: BTreeMap<u64, Option<Opened>>,
    /// 下一块的编号。
    next: usize,
    /// 收尾的事件报的用量。
    usage: Option<Usage>,
    /// 见到的收尾事件。
    ended: Option<End>,
    /// 流里报了错，或者流坏了。
    error: Option<CallError>,
    /// 流里报的错，供应商说要等多久。
    wait: Option<u64>,
}

/// 收尾的事件。
#[derive(Debug, Clone, PartialEq, Eq)]
enum End {
    /// `response.completed`。
    Completed,
    /// `response.incomplete`，带原因。
    Incomplete(String),
}

/// 开了的一块。
#[derive(Debug, Clone)]
struct Opened {
    /// 编号。
    index: usize,
    /// 是哪一种项。
    item: ItemKind,
    /// 流过字了。
    streamed: bool,
    /// 上一段摘要是第几段：换了一段先交一个空行。
    summary: Option<u64>,
    /// 工具调用攒着的参数：增量接起来的，`function_call_arguments.done` 来了换成它的整段。
    arguments: String,
    /// 工具调用的参数交过了。
    settled: bool,
}

/// 认得的项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemKind {
    /// `message`：正文。
    Message,
    /// `reasoning`：思考。
    Reasoning,
    /// `function_call`：工具调用。
    FunctionCall,
}

impl Decoder {
    /// 空的。
    pub fn new() -> Decoder {
        Decoder::default()
    }

    /// 喂一片字节，交回解出来的增量。见到收尾的事件或者出了错以后，再来的都不理。
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Delta> {
        let events = self.sse.feed(bytes);
        let mut out = Vec::new();
        for event in events {
            self.event(&event, false, &mut out);
        }
        out
    }

    /// 不用再读了：见到了收尾的事件，或者出了错。
    pub fn done(&self) -> bool {
        self.ended.is_some() || self.error.is_some()
    }

    /// 收尾的事件到了：模型说完了。
    pub fn finished(&self) -> bool {
        self.ended.is_some()
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
            // 没等到这一项完了的工具调用：照攒的交。
            for opened in self.blocks.values_mut().flatten() {
                settle(&mut deltas, opened, None);
            }
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
            usage: self.usage,
            error,
            retry_after_ms: self.wait,
        }
    }

    /// 照收尾的事件看是不是正常说完的。
    fn ending_error(&self) -> Option<CallError> {
        let reason = match &self.ended {
            None => {
                return Some(CallError {
                    class: ErrorClass::Retryable,
                    message: "流断了：没等到 response.completed".to_string(),
                    status: None,
                });
            }
            Some(End::Completed) => return None,
            Some(End::Incomplete(reason)) => reason,
        };
        let class = match reason.as_str() {
            "max_output_tokens" => return None,
            "content_filter" => ErrorClass::ContentPolicy,
            _ => ErrorClass::Unclassified,
        };
        Some(CallError {
            class,
            message: format!("incomplete: {reason}"),
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
        let text = |field: &str| value.get(field).and_then(Value::as_str).unwrap_or_default();
        match name {
            "response.output_item.added" => {
                self.opened(&value, out);
            }
            "response.output_text.delta" | "response.refusal.delta" => {
                if let Some(opened) = self.block(&value) {
                    push(out, opened, text("delta"));
                }
            }
            "response.function_call_arguments.delta" => {
                if let Some(opened) = self.block(&value) {
                    opened.arguments.push_str(text("delta"));
                }
            }
            "response.function_call_arguments.done" => {
                if let Some(opened) = self.block(&value)
                    && !text("arguments").is_empty()
                {
                    opened.arguments = text("arguments").to_string();
                }
            }
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                let part = value.get("summary_index").and_then(Value::as_u64);
                if let Some(opened) = self.block(&value) {
                    if let (Some(part), Some(before)) = (part, opened.summary)
                        && part != before
                        && opened.streamed
                    {
                        push(out, opened, "\n\n");
                    }
                    if part.is_some() {
                        opened.summary = part;
                    }
                    push(out, opened, text("delta"));
                }
            }
            "response.output_item.done" => self.item_done(&value, out),
            "response.completed" | "response.incomplete" => {
                self.usage = value.pointer("/response/usage").and_then(usage);
                self.ended = Some(match name {
                    "response.completed" => End::Completed,
                    _ => End::Incomplete(
                        value
                            .pointer("/response/incomplete_details/reason")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown")
                            .to_string(),
                    ),
                });
            }
            "response.failed" => {
                // 共用的分类先找 `error` 一格：`response` 那个对象里正好有。
                let failed = value.get("response").map_or_else(
                    || data.as_bytes().to_vec(),
                    |response| response.to_string().into_bytes(),
                );
                self.stream_error(&failed);
            }
            "error" => self.stream_error(data.as_bytes()),
            _ => {}
        }
    }

    /// 流里报的错：照出错分类分，供应商说要等多久的一起留下。
    fn stream_error(&mut self, body: &[u8]) {
        let classified = classify(&Failure::stream(body));
        self.error = Some(classified.error);
        self.wait = classified.retry_after_ms;
    }

    /// `output_index` 指的那一块；没开的、不认的没有。
    fn block(&mut self, value: &Value) -> Option<&mut Opened> {
        let at = value.get("output_index").and_then(Value::as_u64)?;
        self.blocks.get_mut(&at)?.as_mut()
    }

    /// 照 `item.type` 开一块；开过的、不认的不再开。交回这一块。
    fn opened(&mut self, value: &Value, out: &mut Vec<Delta>) -> Option<&mut Opened> {
        let at = value.get("output_index").and_then(Value::as_u64)?;
        if !self.blocks.contains_key(&at) {
            let item = &value["item"];
            let field = |name: &str| item.get(name).and_then(Value::as_str).unwrap_or_default();
            let (kind, delta_kind) = match field("type") {
                "message" => (ItemKind::Message, Kind::Text),
                "reasoning" => (ItemKind::Reasoning, Kind::Reasoning),
                "function_call" => (
                    ItemKind::FunctionCall,
                    Kind::ToolCall {
                        name: field("name").to_string(),
                    },
                ),
                _ => {
                    self.blocks.insert(at, None);
                    return None;
                }
            };
            let index = self.next;
            self.next += 1;
            out.push(Delta::Start {
                index,
                kind: delta_kind,
            });
            if kind == ItemKind::FunctionCall && !field("call_id").is_empty() {
                out.push(Delta::Private {
                    index,
                    private: private(&CallIdOnly {
                        call_id: field("call_id"),
                    }),
                });
            }
            self.blocks.insert(
                at,
                Some(Opened {
                    index,
                    item: kind,
                    streamed: false,
                    summary: None,
                    arguments: String::new(),
                    settled: false,
                }),
            );
        }
        self.blocks.get_mut(&at)?.as_mut()
    }

    /// `response.output_item.done`：一个字都没流过来的照整段补上；思考有加密内容的交私有数据。
    fn item_done(&mut self, value: &Value, out: &mut Vec<Delta>) {
        let item = value["item"].clone();
        let Some(opened) = self.opened(value, out) else {
            return;
        };
        let texts = |list: &str, field: &str| -> Vec<String> {
            item.get(list)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|part| part.get(field).and_then(Value::as_str))
                .filter(|text| !text.is_empty())
                .map(str::to_string)
                .collect()
        };
        match opened.item {
            ItemKind::FunctionCall => {
                settle(out, opened, item.get("arguments").and_then(Value::as_str));
            }
            ItemKind::Message if !opened.streamed => {
                let mut said = texts("content", "text");
                said.extend(texts("content", "refusal"));
                push(out, opened, &said.concat());
            }
            ItemKind::Reasoning if !opened.streamed => {
                push(out, opened, &texts("summary", "text").join("\n\n"));
            }
            ItemKind::Message | ItemKind::Reasoning => {}
        }
        if opened.item == ItemKind::Reasoning
            && let (Some(id), Some(encrypted)) = (
                item.get("id").and_then(Value::as_str),
                item.get("encrypted_content").and_then(Value::as_str),
            )
            && !encrypted.is_empty()
        {
            out.push(Delta::Private {
                index: opened.index,
                private: private(&Encrypted {
                    id,
                    encrypted_content: encrypted,
                }),
            });
        }
    }
}

/// 工具调用的参数交一整段（只交一次）：`whole` 是 `output_item.done` 给的整段，空的、没有的照攒着的。别的项什么都不做。
fn settle(out: &mut Vec<Delta>, opened: &mut Opened, whole: Option<&str>) {
    if opened.item != ItemKind::FunctionCall || opened.settled {
        return;
    }
    opened.settled = true;
    let arguments = whole
        .filter(|whole| !whole.is_empty())
        .map_or_else(|| mem::take(&mut opened.arguments), str::to_string);
    push(out, opened, &arguments);
}

/// 交一段字：空的不交。
fn push(out: &mut Vec<Delta>, opened: &mut Opened, text: &str) {
    if !text.is_empty() {
        opened.streamed = true;
        out.push(Delta::Text {
            index: opened.index,
            text: text.to_string(),
        });
    }
}

/// 工具调用的私有数据。
#[derive(Serialize)]
struct CallIdOnly<'a> {
    call_id: &'a str,
}

/// 思考的私有数据：`id` 在前。
#[derive(Serialize)]
struct Encrypted<'a> {
    id: &'a str,
    encrypted_content: &'a str,
}

/// 这个驱动的私有数据：编码时照它原样回传。
fn private(data: &impl Serialize) -> Private {
    let data = serde_json::to_string(data).expect("几个字符串写得成 JSON");
    Private {
        driver: DriverFamily::parse(FAMILY).expect("驱动家族合写法"),
        data: serde_json::from_str::<RawJson>(&data).expect("刚写出来的是 JSON"),
    }
}

/// 报错里带上的原文，最长 200 个字。
fn clip(text: &str) -> String {
    text.chars().take(200).collect()
}
