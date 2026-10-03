//! 统一的请求里的消息写成 `input` 里的一项项（`docs/blueprint/drivers/openai-responses.md`「怎么走：编码」第 3 到 5 条）。
//!
//! - user：`{"role":"user","content":…}`，全是文字的拼成一个字符串（照 openai-chat 的拼法，[`crate::media::join`]），有能发
//!   的图片、文件的分成几段；发不了的照 [`crate::media`] 换成字。
//! - assistant：照块的先后写成几项，连着的正文接成一项；自己家带加密内容的思考一块一项；工具调用一次一项，不写 `id`。
//! - tool：`function_call_output`，内容照 user 的写法，图、PDF 放在里面；一个字都没有的写「没有输出」那一句。出错的标记
//!   不发：这一家的结果没有这一格。

use std::collections::BTreeMap;
use std::mem;

use miyu_kernel::block::{Block, File, Image, Reasoning, ToolCall};
use miyu_kernel::id::{CallId, ContentHash, MediaType};
use miyu_kernel::request::{Message, Request};
use serde::Deserialize;
use serde::de::IgnoredAny;

use super::FAMILY;
use super::wire::{Content, Item, Part, RoleMessage, Summary, Typed};
use crate::media::{Media, is_pdf, join};
use crate::{BlobBytes, Call, DriverTexts, EncodeError, base64};

/// 写全部的项。
pub(super) fn write(
    request: &Request,
    call: &Call,
    texts: &DriverTexts,
    blobs: &dyn BlobBytes,
) -> Result<Vec<Item>, EncodeError> {
    let writer = Writer {
        call,
        media: Media {
            texts,
            blobs,
            described: &request.described,
        },
        ids: wire_ids(request),
    };
    let mut items = Vec::new();
    for message in &request.messages {
        match message {
            Message::User { blocks } => items.push(Item::Role(RoleMessage {
                role: "user",
                content: writer.content(blocks)?,
            })),
            Message::Assistant { blocks } => writer.assistant(blocks, &mut items),
            Message::Tool {
                call_id, blocks, ..
            } => {
                let output = match writer.content(blocks)? {
                    Content::Text(text) if text.is_empty() => Content::Text(texts.no_output()),
                    content => content,
                };
                items.push(Item::Typed(Typed::FunctionCallOutput {
                    call_id: writer.id(call_id),
                    output,
                }));
            }
        }
    }
    Ok(items)
}

/// 写项要用的：一次调用定的、换成字要用的，和调用编号的对照。
struct Writer<'a> {
    call: &'a Call,
    media: Media<'a>,
    /// 内核的调用编号到线上的编号。
    ids: BTreeMap<CallId, String>,
}

impl Writer<'_> {
    /// user 消息、工具结果的内容：字照拼法接成一段，能发的图片、PDF 各是一段。
    fn content(&self, blocks: &[Block]) -> Result<Content, EncodeError> {
        let mut pieces = Pieces::default();
        for block in blocks {
            match block {
                Block::Text(text) => pieces.text(&text.text),
                Block::Image(image) if self.call.inputs.images => self.image(image, &mut pieces)?,
                Block::Image(image) => pieces.text(&self.media.unseen(image)),
                Block::File(file) if self.call.inputs.pdf && is_pdf(file) => {
                    pieces.part(self.file(file)?);
                }
                Block::File(file) => pieces.text(&self.media.file_text(file)?),
                Block::Reasoning(_) | Block::ToolCall(_) | Block::Unknown(_) => {}
            }
        }
        Ok(pieces.content())
    }

    /// assistant 消息：照块的先后写成几项，连着的正文接成一项。
    fn assistant(&self, blocks: &[Block], items: &mut Vec<Item>) {
        let mut text = String::new();
        for block in blocks {
            let item = match block {
                Block::Text(said) => {
                    text.push_str(&said.text);
                    continue;
                }
                Block::Reasoning(thought) => match reasoning(thought) {
                    Some(item) => item,
                    None => continue,
                },
                Block::ToolCall(call) => Typed::FunctionCall {
                    call_id: self.id(&call.call_id),
                    name: call.name.clone(),
                    arguments: arguments(&call.args),
                },
                Block::Image(_) | Block::File(_) | Block::Unknown(_) => continue,
            };
            flush(&mut text, items);
            items.push(Item::Typed(item));
        }
        flush(&mut text, items);
    }

    /// 能看图时的一张图：data URL；带名字的前后各一段标签，标签照字拼。
    fn image(&self, image: &Image, pieces: &mut Pieces) -> Result<(), EncodeError> {
        let picture = Part::Image {
            image_url: self.data_url(&image.media_type, &image.blob)?,
        };
        match self.media.image_tags(image) {
            Some((open, close)) => {
                pieces.text(&open);
                pieces.part(picture);
                pieces.text(&close);
            }
            None => pieces.part(picture),
        }
        Ok(())
    }

    /// 一份 PDF：`input_file`，带文件名。
    fn file(&self, file: &File) -> Result<Part, EncodeError> {
        Ok(Part::File {
            filename: file.name.as_str().to_string(),
            file_data: self.data_url(&file.media_type, &file.blob)?,
        })
    }

    /// `data:<类型>;base64,<内容>`。
    fn data_url(&self, media_type: &MediaType, blob: &ContentHash) -> Result<String, EncodeError> {
        Ok(format!(
            "data:{};base64,{}",
            media_type.as_str(),
            base64::encode(self.media.bytes(blob)?)
        ))
    }

    /// 线上的调用编号：自己家私有数据里的，没有就用内核分的（别家的编号写法不一定收）。
    fn id(&self, call_id: &CallId) -> String {
        self.ids
            .get(call_id)
            .cloned()
            .unwrap_or_else(|| call_id.to_string())
    }
}

/// 攒着的正文写成一项 `{"role":"assistant","content":<字>}`；空的不写。
fn flush(text: &mut String, items: &mut Vec<Item>) {
    if !text.is_empty() {
        items.push(Item::Role(RoleMessage {
            role: "assistant",
            content: Content::Text(mem::take(text)),
        }));
    }
}

/// 拼内容：文字先攒着，碰到图片、文件才收成一段。
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

    /// 没有图片、文件的，是一个字符串（一个字都没有的是空串）；有的，分成几段。
    fn content(mut self) -> Content {
        if self.parts.is_empty() {
            return Content::Text(self.text);
        }
        self.end_text();
        Content::Parts(self.parts)
    }
}

/// 一块思考怎么回传（「编码」第 4 条）：私有数据是这个驱动的、里面有 `id` 和 `encrypted_content` 的，写成一项 `reasoning`，
/// 字是空的摘要写 `[]`；别的不写（没有加密内容，`store` 是假的，这一家认不出这一项）。
fn reasoning(thought: &Reasoning) -> Option<Typed> {
    #[derive(Deserialize)]
    struct Data {
        id: String,
        encrypted_content: String,
    }
    let private = thought
        .private
        .as_ref()
        .filter(|private| private.driver.as_str() == FAMILY)?;
    let data: Data = serde_json::from_str(private.data.get()).ok()?;
    let summary = match thought.text.is_empty() {
        true => Vec::new(),
        false => vec![Summary {
            kind: "summary_text",
            text: thought.text.clone(),
        }],
    };
    Some(Typed::Reasoning {
        id: data.id,
        summary,
        encrypted_content: data.encrypted_content,
    })
}

/// 参数原文：是一个 JSON 对象的照抄；空的、坏的写成 `{}`。
fn arguments(args: &str) -> String {
    if serde_json::from_str::<BTreeMap<String, IgnoredAny>>(args).is_ok() {
        args.to_string()
    } else {
        "{}".to_string()
    }
}

/// 内核的调用编号到线上的编号：私有数据是这个驱动的、里面有字符串 `call_id` 的，用它。
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

/// 私有数据里自己家的调用编号：`{"call_id":"…"}`。
fn provider_id(call: &ToolCall) -> Option<String> {
    #[derive(Deserialize)]
    struct Id {
        call_id: String,
    }
    let private = call
        .private
        .as_ref()
        .filter(|private| private.driver.as_str() == FAMILY)?;
    serde_json::from_str::<Id>(private.data.get())
        .ok()
        .map(|found| found.call_id)
}
