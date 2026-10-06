//! 统一的请求里的消息写成线上的消息（`docs/blueprint/drivers/anthropic.md`「怎么走：编码」第 3 到 6 条）。
//!
//! - user、tool 都写成线上的 `user`，assistant 写成 `assistant`；相邻两条线上的角色一样的合成一条，后一条的块接在前一条
//!   后面；一块都没有的不发。所以一串 tool 合成一条 user，tool 后面跟着的 user 也合进去，接在结果后面。
//! - user 的块一块一块写，不拼；空的字不写。图片照模型能不能看、媒体类型收不收写成 `image` 或者换成字；文件能读 PDF 的
//!   写成 `document`，别的换成字（[`crate::media`]）。
//! - assistant：正文、自己家带签名的思考、工具调用照原来的先后；别家的、没签名的思考不写（发回去这一家报 400）。
//! - tool：一块 `tool_result`，里面照 user 的写法；一块都没有的写「没有输出」那一句。

use std::collections::BTreeMap;

use miyu_kernel::block::{Block, File, Image, Reasoning, ToolCall};
use miyu_kernel::id::CallId;
use miyu_kernel::request::{Message, Request};
use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::value::RawValue;

use super::FAMILY;
use super::wire::{Part, Source, Wire};
use crate::media::{Media, is_pdf};
use crate::{BlobBytes, Call, DriverTexts, EncodeError, base64};

/// 这一家收的图片媒体类型：别的发过去也报 400，换成字（图纸「起草时定的」第 14 条）。
const IMAGE_TYPES: [&str; 4] = ["image/jpeg", "image/png", "image/gif", "image/webp"];

/// 写好的消息，和打点要知道的两样。
pub(super) struct Written {
    /// 合并以后的线上的消息。
    pub(super) messages: Vec<Wire>,
    /// 每条线上的消息是从统一的请求里第几条消息开始的。
    pub(super) firsts: Vec<usize>,
    /// 前 `stable` 条消息写出的最后一块：第几条线上的消息、第几块。一块都没写出的、`stable` 是 0 的没有。
    pub(super) stable_end: Option<(usize, usize)>,
}

/// 写全部消息。
pub(super) fn write(
    request: &Request,
    call: &Call,
    texts: &DriverTexts,
    blobs: &dyn BlobBytes,
) -> Result<Written, EncodeError> {
    let writer = Writer {
        call,
        texts,
        media: Media {
            texts,
            blobs,
            described: &request.described,
        },
        ids: wire_ids(request),
    };
    let mut written = Written {
        messages: Vec::new(),
        firsts: Vec::new(),
        stable_end: None,
    };
    for (index, message) in request.messages.iter().enumerate() {
        let (role, parts) = match message {
            Message::User { blocks } => ("user", writer.blocks(blocks)?),
            Message::Assistant { blocks } => ("assistant", writer.assistant(blocks)),
            Message::Tool {
                call_id,
                error,
                blocks,
            } => ("user", vec![writer.tool(call_id, *error, blocks)?]),
        };
        if !parts.is_empty() {
            match written.messages.last_mut() {
                Some(last) if last.role == role => last.content.extend(parts),
                _ => {
                    written.messages.push(Wire {
                        role,
                        content: parts,
                    });
                    written.firsts.push(index);
                }
            }
        }
        if index + 1 == request.stable
            && let Some(last) = written.messages.last()
        {
            written.stable_end = Some((written.messages.len() - 1, last.content.len() - 1));
        }
    }
    Ok(written)
}

/// 写消息要用的：一次调用定的、占位的几句、换成字要用的，和调用编号的对照。
struct Writer<'a> {
    call: &'a Call,
    texts: &'a DriverTexts,
    media: Media<'a>,
    /// 内核的调用编号到线上的编号。
    ids: BTreeMap<CallId, String>,
}

impl Writer<'_> {
    /// user 消息、工具结果里的块：字、图片、文件，一块一块写。
    fn blocks(&self, blocks: &[Block]) -> Result<Vec<Part>, EncodeError> {
        let mut parts = Vec::new();
        for block in blocks {
            match block {
                Block::Text(text) => push_text(&mut parts, &text.text),
                Block::Image(image) => self.image(image, &mut parts)?,
                Block::File(file) if self.call.inputs.pdf && is_pdf(file) => {
                    parts.push(self.document(file)?);
                }
                Block::File(file) => push_text(&mut parts, &self.media.file_text(file)?),
                Block::Reasoning(_) | Block::ToolCall(_) | Block::Unknown(_) => {}
            }
        }
        Ok(parts)
    }

    /// assistant 消息：正文、自己家带签名的思考、工具调用，照原来的先后。
    fn assistant(&self, blocks: &[Block]) -> Vec<Part> {
        let mut parts = Vec::new();
        for block in blocks {
            match block {
                Block::Text(text) => push_text(&mut parts, &text.text),
                Block::Reasoning(thought) => parts.extend(thinking(thought)),
                Block::ToolCall(call) => parts.push(Part::ToolUse {
                    id: self.id(&call.call_id),
                    name: call.name.clone(),
                    input: input(&call.args),
                }),
                Block::Image(_) | Block::File(_) | Block::Unknown(_) => {}
            }
        }
        parts
    }

    /// 一次调用的结果：一块 `tool_result`，里面照 user 的写法；一块都没有的写「没有输出」那一句。
    fn tool(&self, call_id: &CallId, error: bool, blocks: &[Block]) -> Result<Part, EncodeError> {
        let mut content = self.blocks(blocks)?;
        if content.is_empty() {
            content.push(Part::text(&self.texts.no_output()));
        }
        Ok(Part::ToolResult {
            tool_use_id: self.id(call_id),
            content,
            is_error: error.then_some(true),
        })
    }

    /// 一张图：能看图、媒体类型这一家收的写成 `image`，带名字的前后各一块标签；别的换成字。
    fn image(&self, image: &Image, parts: &mut Vec<Part>) -> Result<(), EncodeError> {
        let media_type = image.media_type.as_str();
        if !self.call.inputs.images || !IMAGE_TYPES.contains(&media_type) {
            push_text(parts, &self.media.unseen(image));
            return Ok(());
        }
        let picture = Part::Image {
            source: self.source(media_type, &image.blob)?,
        };
        match self.media.image_tags(image) {
            Some((open, close)) => {
                push_text(parts, &open);
                parts.push(picture);
                push_text(parts, &close);
            }
            None => parts.push(picture),
        }
        Ok(())
    }

    /// 一份 PDF：`document`，带文件名。
    fn document(&self, file: &File) -> Result<Part, EncodeError> {
        Ok(Part::Document {
            source: self.source(file.media_type.as_str(), &file.blob)?,
            title: file.name.as_str().to_string(),
        })
    }

    /// base64 的内容。
    fn source(
        &self,
        media_type: &str,
        blob: &miyu_kernel::id::ContentHash,
    ) -> Result<Source, EncodeError> {
        Ok(Source {
            kind: "base64",
            media_type: media_type.to_string(),
            data: base64::encode(self.media.bytes(blob)?),
        })
    }

    /// 线上的调用编号：自己家私有数据里的，没有就用内核分的（别家的编号可能带这一家不收的字）。
    fn id(&self, call_id: &CallId) -> String {
        self.ids
            .get(call_id)
            .cloned()
            .unwrap_or_else(|| call_id.to_string())
    }
}

/// 接上一块字：空的不写，这一家不收空字。
fn push_text(parts: &mut Vec<Part>, text: &str) {
    if !text.is_empty() {
        parts.push(Part::text(text));
    }
}

/// 一块思考怎么回传（「怎么走：编码」第 5 条）：私有数据是这个驱动的，里面有 `signature` 的写成 `thinking`（字是空的也写），
/// 有 `redacted` 的写成 `redacted_thinking`；别的不写。
fn thinking(thought: &Reasoning) -> Option<Part> {
    #[derive(Deserialize)]
    struct Data {
        signature: Option<String>,
        redacted: Option<String>,
    }
    let private = thought
        .private
        .as_ref()
        .filter(|private| private.driver.as_str() == FAMILY)?;
    let data: Data = serde_json::from_str(private.data.get()).ok()?;
    match (data.signature, data.redacted) {
        (Some(signature), _) => Some(Part::Thinking {
            thinking: thought.text.clone(),
            signature,
        }),
        (None, Some(data)) => Some(Part::RedactedThinking { data }),
        (None, None) => None,
    }
}

/// 参数：原文是一个 JSON 对象的原样，空的、坏的、别的写 `{}`。
fn input(args: &str) -> Box<RawValue> {
    let object = serde_json::from_str::<BTreeMap<String, IgnoredAny>>(args).is_ok();
    let text = if object { args } else { "{}" };
    serde_json::from_str(text).expect("读得成对象的就读得成原样的 JSON")
}

/// 内核的调用编号到线上的编号：私有数据是这个驱动的、里面有字符串 `id` 的，用它。
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

/// 私有数据里自己家的调用编号：`{"id":"…"}`。
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
