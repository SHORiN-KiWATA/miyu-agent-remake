//! OpenAI 兼容的对话接口（`docs/designs/05-内核接口.md` 第七节「OpenAI 兼容的对话接口怎么编码」）：
//! DeepSeek、智谱、OpenRouter、opencode Zen，本机的 Ollama、LM Studio 都说这一种。
//!
//! 统一的请求照那张表写成一个 JSON：顶层字段的先后固定，每条消息照 `wire.rs` 里写的来，同样的
//! 输入字节一定一样。供应商之间不一样的地方是 [`Compat`] 里的三个开关，跟着供应商定、会话里不变。
//!
//! 响应是 SSE 流，[`Decoder`] 解成内核的四种增量，说完时交出用量和出错。

mod decode;
mod messages;
mod usage;
mod wire;

pub use decode::Decoder;

use std::collections::BTreeSet;

use miyu_kernel::block::Block;
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::{Message, Request};
use serde::Serialize;

pub use crate::{EncodeError, Encoded};

use crate::{BlobBytes, Call, DriverTexts};

/// 驱动家族：私有数据里写的是它的，才是这个驱动的（`03-事件模型.md` 第九节）。
pub const FAMILY: &str = "openai-chat";

/// 请求发到供应商地址后面的这一截。
pub const PATH: &str = "/chat/completions";

/// 供应商之间不一样的三处。默认是最常见的写法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Compat {
    /// 输出上限写在哪个字段。
    pub output_limit: OutputLimit,
    /// 思考回不回传、怎么回传。
    pub reasoning: ReasoningReplay,
    /// 要不要在流里报用量（`stream_options.include_usage`）。
    pub stream_usage: bool,
}

impl Default for Compat {
    /// `max_tokens`，不回传思考，报用量。
    fn default() -> Compat {
        Compat {
            output_limit: OutputLimit::MaxTokens,
            reasoning: ReasoningReplay::Drop,
            stream_usage: true,
        }
    }
}

/// 输出上限写在哪个字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputLimit {
    /// `max_tokens`：这类接口的供应商多数只认它。
    MaxTokens,
    /// `max_completion_tokens`。
    MaxCompletionTokens,
}

impl OutputLimit {
    fn field(self) -> &'static str {
        match self {
            OutputLimit::MaxTokens => "max_tokens",
            OutputLimit::MaxCompletionTokens => "max_completion_tokens",
        }
    }
}

/// 以前的思考怎么回传。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningReplay {
    /// 不回传。
    Drop,
    /// 写进 `field`；`always` 的，没有思考时也写，写空串（DeepSeek 要每条 assistant 都带）。
    Replay {
        /// 写进哪个字段。
        field: ReasoningField,
        /// 没有思考时也写。
        always: bool,
    },
}

/// 思考写进哪个字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningField {
    /// `reasoning_content`：DeepSeek、Moonshot、智谱、阿里。
    ReasoningContent,
    /// `reasoning`。
    Reasoning,
}

/// 编码：顶层照 `model`、`messages`、`tools`、`stream`、`stream_options`、输出上限的先后写，
/// 别的字段一概不发。
///
/// # Errors
///
/// 要用的 blob 执行器没交进来：先照 [`blobs_needed`] 取。
///
/// # Panics
///
/// 实际不会 panic：线上的消息里没有写不成 JSON 的东西。
pub fn encode(
    request: &Request,
    call: &Call,
    compat: &Compat,
    texts: &DriverTexts,
    blobs: &dyn BlobBytes,
) -> Result<Encoded, EncodeError> {
    let messages = messages::write(request, call, compat, texts, blobs)?;
    let mut body = Vec::new();
    body.extend_from_slice(b"{\"model\":");
    json(&mut body, call.model.as_str());
    body.extend_from_slice(b",\"messages\":[");
    let mut ranges = Vec::with_capacity(messages.len());
    for (index, message) in messages.iter().enumerate() {
        if index > 0 {
            body.push(b',');
        }
        let start = body.len();
        json(&mut body, message);
        ranges.push(start..body.len());
    }
    body.push(b']');
    if let Some(tools) = wire::tools(request) {
        body.extend_from_slice(b",\"tools\":");
        json(&mut body, &tools);
    }
    body.extend_from_slice(b",\"stream\":true");
    if compat.stream_usage {
        body.extend_from_slice(b",\"stream_options\":{\"include_usage\":true}");
    }
    if let Some(limit) = call.max_output {
        let field = compat.output_limit.field();
        body.extend_from_slice(format!(",\"{field}\":{limit}").as_bytes());
    }
    body.push(b'}');
    Ok(Encoded {
        body,
        messages: ranges,
    })
}

/// 这份请求编码时要用哪些 blob：模型能看图的，要图片；能读 PDF 的，要 PDF。执行器照着先取出来。
pub fn blobs_needed(request: &Request, call: &Call) -> BTreeSet<ContentHash> {
    let mut needed = BTreeSet::new();
    for message in &request.messages {
        let blocks = match message {
            Message::User { blocks } | Message::Tool { blocks, .. } => blocks,
            Message::Assistant { .. } => continue,
        };
        for block in blocks {
            match block {
                Block::Image(image) if call.inputs.images => {
                    needed.insert(image.blob.clone());
                }
                Block::File(file) if call.inputs.pdf && messages::is_pdf(file) => {
                    needed.insert(file.blob.clone());
                }
                _ => {}
            }
        }
    }
    needed
}

/// 写成紧凑的 JSON，接在后面。
fn json(body: &mut Vec<u8>, value: &(impl Serialize + ?Sized)) {
    serde_json::to_writer(body, value).expect("线上的消息里没有写不成 JSON 的东西");
}
