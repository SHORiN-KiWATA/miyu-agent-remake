//! 统一的请求里的消息写成线上的消息（`05-内核接口.md` 第七节那张表）。
//!
//! - user：全是文字的拼成一个字符串，相邻两块之间补一个换行，前一块已经以换行结尾的不补；有图片、
//!   文件的分成几段，连着的文字照样拼成一段。
//! - assistant：正文、思考各自直接接上，它们本来就是一整段；工具调用的编号用供应商自己的。
//! - tool：文字照 user 的拼法；图片、PDF 挪到这一串 tool 消息后面的一条 user 消息里。
//! - 文件发不了 PDF 的：内容是文本的照字放进消息，带着文件名（施工 3-9 三补，[`crate::text_file`]）；别的写一句占位，
//!   带文件名、媒体类型、大小。
//! - 带名字的图片（人附的，施工 3-9 四补）：能看图的前后各一段标签，标签是文字，照文字拼；不能看图的占位写上名字。
//! - 替它看的图（施工 8-17）：不能看图、请求的 `described` 里有这张图的转述的，换成带标签的转述，照文字拼；没有的照旧占位。
//! - 接着写的：最后那条 user（只有被打断的那一句）不发，半截那条 assistant 加上接着写的字段。

use std::collections::BTreeMap;
use std::mem;

use miyu_kernel::block::{Block, File, Image, ToolCall};
use miyu_kernel::id::{CallId, ContentHash, MediaType};
use miyu_kernel::request::{Message, Request};
use serde::Deserialize;
use serde::de::IgnoredAny;

use super::wire::{Content, FileData, FunctionCall, Part, ToolCall as WireCall, Url, Wire};
use super::{Compat, ContinuationField, EncodeError, FAMILY, ReasoningField, ReasoningReplay};
use crate::{BlobBytes, Call, DriverTexts, base64, text_file};

/// 写全部消息：system 在最前，每条 tool 消息串后面跟着挪出来的图片、文件。`continuing` 有的是
/// 接着写：最后那条 user 不发，半截那条加上这个字段。
pub(super) fn write(
    request: &Request,
    call: &Call,
    compat: &Compat,
    texts: &DriverTexts,
    blobs: &dyn BlobBytes,
    continuing: Option<ContinuationField>,
) -> Result<Vec<Wire>, EncodeError> {
    let writer = Writer {
        call,
        compat,
        texts,
        blobs,
        ids: wire_ids(request),
        described: &request.described,
    };
    let mut out = Vec::new();
    if !request.system.is_empty() {
        out.push(Wire::System {
            content: request.system.clone(),
        });
    }
    let mut moved = Vec::new();
    let messages = match (continuing, request.messages.split_last()) {
        (Some(_), Some((Message::User { .. }, earlier))) => earlier,
        _ => &request.messages[..],
    };
    for message in messages {
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
    if let (
        Some(field),
        Some(Wire::Assistant {
            prefix, partial, ..
        }),
    ) = (continuing, out.last_mut())
    {
        match field {
            ContinuationField::Prefix => *prefix = Some(true),
            ContinuationField::Partial => *partial = Some(true),
        }
    }
    Ok(out)
}

/// 这是不是一个 PDF：只有 PDF 能作为文件发。
fn is_pdf(file: &File) -> bool {
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
    /// 请求里的图的转述（施工 8-17）：不能看图时照它写。
    described: &'a BTreeMap<ContentHash, String>,
}

impl Writer<'_> {
    /// user 消息。
    fn user(&self, blocks: &[Block]) -> Result<Content, EncodeError> {
        let mut pieces = Pieces::default();
        for block in blocks {
            match block {
                Block::Text(text) => pieces.text(&text.text),
                Block::Image(image) if self.call.inputs.images => {
                    for part in self.image(image)? {
                        pieces.add(part);
                    }
                }
                Block::Image(image) => pieces.text(&self.unseen(image)),
                Block::File(file) if self.call.inputs.pdf && is_pdf(file) => {
                    pieces.part(self.file_part(file)?);
                }
                Block::File(file) => pieces.text(&self.file_text(file)?),
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
            prefix: None,
            partial: None,
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
                    attachments.extend(self.image(image)?);
                }
                Block::Image(image) => join(&mut content, &self.unseen(image)),
                Block::File(file) if self.call.inputs.pdf && is_pdf(file) => {
                    attachments.push(self.file_part(file)?);
                }
                Block::File(file) => join(&mut content, &self.file_text(file)?),
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

    /// 挪出来的图片、文件放成一条 user 消息：先一句说明，再按先后放；带名字的图片前后的标签照 user 的拼法和挨着的
    /// 文字拼成一段。没有就什么都不放。
    fn flush(&self, moved: &mut Vec<Part>, out: &mut Vec<Wire>) {
        if moved.is_empty() {
            return;
        }
        let mut pieces = Pieces::default();
        pieces.text(&self.texts.tool_attachments());
        for part in moved.drain(..) {
            pieces.add(part);
        }
        out.push(Wire::User {
            content: pieces.content(),
        });
    }

    /// 不能看图时的一张图：有转述、快照里有标签的写成带标签的转述（施工 8-17），别的写占位那一句。
    fn unseen(&self, image: &Image) -> String {
        self.described
            .get(&image.blob)
            .and_then(|description| self.texts.image_described(name(image), description))
            .unwrap_or_else(|| self.texts.image_omitted(name(image)))
    }

    /// 能看图时的一张图：data URL；带名字的前后各一段标签（施工 3-9 四补）。
    fn image(&self, image: &Image) -> Result<Vec<Part>, EncodeError> {
        let picture = image_part(self.data_url(&image.media_type, &image.blob)?);
        Ok(match self.texts.image_tags(name(image)) {
            Some((open, close)) => vec![
                Part::Text { text: open },
                picture,
                Part::Text { text: close },
            ],
            None => vec![picture],
        })
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

    /// 发不了 PDF 的文件写成字：内容是文本的、快照里有那三句的，照字放进来；别的写一句占位，带文件名、媒体类型、
    /// 大小（施工 3-9 三补）。
    fn file_text(&self, file: &File) -> Result<String, EncodeError> {
        let bytes = self.bytes(&file.blob)?;
        let name = file.name.as_str();
        let text = text_file::as_text(bytes).and_then(|text| self.texts.text_file(name, text));
        Ok(text.unwrap_or_else(|| {
            self.texts
                .file_omitted(name, file.media_type.as_str(), bytes.len())
        }))
    }

    /// `data:<类型>;base64,<内容>`。
    fn data_url(&self, media_type: &MediaType, blob: &ContentHash) -> Result<String, EncodeError> {
        Ok(format!(
            "data:{};base64,{}",
            media_type.as_str(),
            base64::encode(self.bytes(blob)?)
        ))
    }

    /// 执行器照 [`super::blobs_needed`] 先取好的字节；没交进来的报缺了哪一个。
    fn bytes(&self, blob: &ContentHash) -> Result<&[u8], EncodeError> {
        self.blobs
            .bytes(blob)
            .ok_or_else(|| EncodeError::MissingBlob(blob.clone()))
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

    /// 接上一段：文字照文字拼，图片、文件收成一段。
    fn add(&mut self, part: Part) {
        match part {
            Part::Text { text } => self.text(&text),
            part => self.part(part),
        }
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

/// 图片块的名字：人附的有，`read` 读出来的没有。
fn name(image: &Image) -> Option<&str> {
    image.name.as_ref().map(|name| name.as_str())
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
