//! 统一的请求里的消息写成线上的消息（`05-内核接口.md` 第七节那张表）。
//!
//! - user：全是文字的拼成一个字符串，相邻两块之间补一个换行，前一块已经以换行结尾的不补；有图片、
//!   文件的分成几段，连着的文字照样拼成一段。
//! - assistant：正文、思考各自直接接上，它们本来就是一整段；工具调用的编号用供应商自己的。
//! - tool：文字照 user 的拼法；图片、PDF 挪到这一串 tool 消息后面的一条 user 消息里。

use std::collections::BTreeMap;
use std::mem;

use miyu_kernel::block::{Block, File, ToolCall};
use miyu_kernel::id::{CallId, ContentHash, MediaType};
use miyu_kernel::request::{Message, Request};
use serde::Deserialize;
use serde::de::IgnoredAny;

use super::wire::{Content, FileData, FunctionCall, Part, ToolCall as WireCall, Url, Wire};
use super::{Compat, EncodeError, FAMILY, ReasoningField, ReasoningReplay};
use crate::{BlobBytes, Call, DriverTexts, base64};

/// 写全部消息：system 在最前，每条 tool 消息串后面跟着挪出来的图片、文件。
pub(super) fn write(
    request: &Request,
    call: &Call,
    compat: &Compat,
    texts: &DriverTexts,
    blobs: &dyn BlobBytes,
) -> Result<Vec<Wire>, EncodeError> {
    let writer = Writer {
        call,
        compat,
        texts,
        blobs,
        ids: wire_ids(request),
    };
    let mut out = Vec::new();
    if !request.system.is_empty() {
        out.push(Wire::System {
            content: request.system.clone(),
        });
    }
    let mut moved = Vec::new();
    for message in &request.messages {
        if !matches!(message, Message::Tool { .. }) {
            writer.flush(&mut moved, &mut out);
        }
        out.push(match message {
            Message::User { blocks } => Wire::User {
                content: writer.user(blocks)?,
            },
            Message::Assistant { blocks } => writer.assistant(blocks),
            Message::Tool {
                call_id, blocks, ..
            } => writer.tool(call_id, blocks, &mut moved)?,
        });
    }
    writer.flush(&mut moved, &mut out);
    Ok(out)
}

/// 这是不是一个 PDF：只有 PDF 能作为文件发。
pub(super) fn is_pdf(file: &File) -> bool {
    file.media_type.as_str() == "application/pdf"
}

/// 写消息要用的：一次调用定的、供应商的开关、占位的几句、blob 的字节，和调用编号的对照。
struct Writer<'a> {
    call: &'a Call,
    compat: &'a Compat,
    texts: &'a DriverTexts,
    blobs: &'a dyn BlobBytes,
    /// 内核的调用编号到线上的编号。
    ids: BTreeMap<CallId, String>,
}

impl Writer<'_> {
    /// user 消息。
    fn user(&self, blocks: &[Block]) -> Result<Content, EncodeError> {
        let mut pieces = Pieces::default();
        for block in blocks {
            match block {
                Block::Text(text) => pieces.text(&text.text),
                Block::Image(image) if self.call.inputs.images => {
                    pieces.part(image_part(self.data_url(&image.media_type, &image.blob)?));
                }
                Block::Image(_) => pieces.text(&self.texts.image_omitted()),
                Block::File(file) if self.call.inputs.pdf && is_pdf(file) => {
                    pieces.part(self.file_part(file)?);
                }
                Block::File(file) => pieces.text(&self.file_omitted(file)),
                Block::Reasoning(_) | Block::ToolCall(_) | Block::Unknown(_) => {}
            }
        }
        Ok(pieces.content())
    }

    /// assistant 消息。
    fn assistant(&self, blocks: &[Block]) -> Wire {
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut tool_calls = Vec::new();
        for block in blocks {
            match block {
                Block::Text(text) => content.push_str(&text.text),
                Block::Reasoning(thought) => reasoning.push_str(&thought.text),
                Block::ToolCall(call) => tool_calls.push(WireCall {
                    id: self.id(&call.call_id),
                    kind: "function",
                    function: FunctionCall {
                        name: call.name.clone(),
                        arguments: arguments(&call.args),
                    },
                }),
                Block::Image(_) | Block::File(_) | Block::Unknown(_) => {}
            }
        }
        let content = (!content.is_empty() || tool_calls.is_empty()).then_some(content);
        let (reasoning_content, reasoning) = match self.compat.reasoning {
            ReasoningReplay::Drop => (None, None),
            ReasoningReplay::Replay { field, always } => {
                let thought = (!reasoning.is_empty() || always).then_some(reasoning);
                match field {
                    ReasoningField::ReasoningContent => (thought, None),
                    ReasoningField::Reasoning => (None, thought),
                }
            }
        };
        Wire::Assistant {
            content,
            reasoning_content,
            reasoning,
            tool_calls,
        }
    }

    /// tool 消息：图片、PDF 挪进 `moved`，等这一串 tool 消息完了一起放。
    fn tool(
        &self,
        call_id: &CallId,
        blocks: &[Block],
        moved: &mut Vec<Part>,
    ) -> Result<Wire, EncodeError> {
        let mut content = String::new();
        let mut attachments = Vec::new();
        for block in blocks {
            match block {
                Block::Text(text) => join(&mut content, &text.text),
                Block::Image(image) if self.call.inputs.images => {
                    attachments.push(image_part(self.data_url(&image.media_type, &image.blob)?));
                }
                Block::Image(_) => join(&mut content, &self.texts.image_omitted()),
                Block::File(file) if self.call.inputs.pdf && is_pdf(file) => {
                    attachments.push(self.file_part(file)?);
                }
                Block::File(file) => join(&mut content, &self.file_omitted(file)),
                Block::Reasoning(_) | Block::ToolCall(_) | Block::Unknown(_) => {}
            }
        }
        if content.is_empty() {
            content = if attachments.is_empty() {
                self.texts.no_output()
            } else {
                self.texts.tool_attachments_only()
            };
        }
        moved.append(&mut attachments);
        Ok(Wire::Tool {
            tool_call_id: self.id(call_id),
            content,
        })
    }

    /// 挪出来的图片、文件放成一条 user 消息：先一句说明，再按先后放。没有就什么都不放。
    fn flush(&self, moved: &mut Vec<Part>, out: &mut Vec<Wire>) {
        if moved.is_empty() {
            return;
        }
        let mut parts = vec![Part::Text {
            text: self.texts.tool_attachments(),
        }];
        parts.append(moved);
        out.push(Wire::User {
            content: Content::Parts(parts),
        });
    }

    /// 线上的调用编号：供应商自己的，没有就用内核分的。
    fn id(&self, call_id: &CallId) -> String {
        self.ids
            .get(call_id)
            .cloned()
            .unwrap_or_else(|| call_id.to_string())
    }

    fn file_part(&self, file: &File) -> Result<Part, EncodeError> {
        Ok(Part::File {
            file: FileData {
                filename: file.name.as_str().to_string(),
                file_data: self.data_url(&file.media_type, &file.blob)?,
            },
        })
    }

    fn file_omitted(&self, file: &File) -> String {
        self.texts
            .file_omitted(file.name.as_str(), file.media_type.as_str())
    }

    /// `data:<类型>;base64,<内容>`。
    fn data_url(&self, media_type: &MediaType, blob: &ContentHash) -> Result<String, EncodeError> {
        let bytes = self
            .blobs
            .bytes(blob)
            .ok_or_else(|| EncodeError::MissingBlob(blob.clone()))?;
        Ok(format!(
            "data:{};base64,{}",
            media_type.as_str(),
            base64::encode(bytes)
        ))
    }
}

/// 拼 user 消息：文字先攒着，碰到图片、文件才收成一段。
#[derive(Default)]
struct Pieces {
    parts: Vec<Part>,
    text: String,
}

impl Pieces {
    fn text(&mut self, text: &str) {
        join(&mut self.text, text);
    }

    fn part(&mut self, part: Part) {
        self.end_text();
        self.parts.push(part);
    }

    fn end_text(&mut self) {
        if !self.text.is_empty() {
            self.parts.push(Part::Text {
                text: mem::take(&mut self.text),
            });
        }
    }

    /// 没有图片、文件的，是一个字符串；有的，分成几段。
    fn content(mut self) -> Content {
        if self.parts.is_empty() {
            return Content::Text(self.text);
        }
        self.end_text();
        Content::Parts(self.parts)
    }
}

/// 接上一块文字：前面有字、又不是以换行结尾的，先补一个换行。空的一块什么都不接。
fn join(into: &mut String, next: &str) {
    if next.is_empty() {
        return;
    }
    if !into.is_empty() && !into.ends_with('\n') {
        into.push('\n');
    }
    into.push_str(next);
}

fn image_part(url: String) -> Part {
    Part::ImageUrl {
        image_url: Url { url },
    }
}

/// 参数原文：是一个 JSON 对象的照抄；空的、坏的写成 `{}`，有的本机服务要把它解析出来。
fn arguments(args: &str) -> String {
    if serde_json::from_str::<BTreeMap<String, IgnoredAny>>(args).is_ok() {
        args.to_string()
    } else {
        "{}".to_string()
    }
}

/// 内核的调用编号到线上的编号：私有数据是这个驱动的、里面有 `id` 的，用它。
fn wire_ids(request: &Request) -> BTreeMap<CallId, String> {
    let mut ids = BTreeMap::new();
    for message in &request.messages {
        let Message::Assistant { blocks } = message else {
            continue;
        };
        for block in blocks {
            if let Block::ToolCall(call) = block
                && let Some(id) = provider_id(call)
            {
                ids.insert(call.call_id, id);
            }
        }
    }
    ids
}

/// 私有数据里供应商自己的调用编号：`{"id":"…"}`。
fn provider_id(call: &ToolCall) -> Option<String> {
    #[derive(Deserialize)]
    struct Id {
        id: String,
    }
    let private = call
        .private
        .as_ref()
        .filter(|private| private.driver.as_str() == FAMILY)?;
    serde_json::from_str::<Id>(private.data.get())
        .ok()
        .map(|found| found.id)
}
