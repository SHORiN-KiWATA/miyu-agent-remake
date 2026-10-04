//! OpenAI 兼容的对话接口（`docs/designs/05-内核接口.md` 第七节「OpenAI 兼容的对话接口怎么编码」）：
//! DeepSeek、智谱、OpenRouter、opencode Zen，本机的 Ollama、LM Studio 都说这一种。
//!
//! 统一的请求照那张表写成一个 JSON：顶层字段的先后固定，每条消息照 `wire.rs` 里写的来，同样的
//! 输入字节一定一样。供应商之间不一样的地方是 [`Compat`] 里的几个开关，跟着供应商定、会话里不变；
//! 出厂的几套住在资源目录的供应商档案里（`resources/models/profiles.toml`，施工 8-6），只给实测过的供应商开。
//!
//! 响应是 SSE 流，[`Decoder`] 解成内核的四种增量，说完时交出用量和出错。列模型（施工 8-7）：`GET /models`，
//! [`parse_models`] 读出模型名和报了的窗口。

mod decode;
mod effort;
mod messages;
mod models;
mod usage;
mod wire;

pub use decode::Decoder;
pub use models::{MODELS_PATH, parse_models};

use std::collections::BTreeSet;

use miyu_kernel::id::ContentHash;
use miyu_kernel::request::Request;
use serde::Serialize;

pub use crate::{EncodeError, Encoded};

use crate::{BlobBytes, Call, DriverTexts, media};

/// 驱动家族：私有数据里写的是它的，才是这个驱动的（`03-事件模型.md` 第九节）。
pub const FAMILY: &str = "openai-chat";

/// 请求发到供应商地址后面的这一截。
pub const PATH: &str = "/chat/completions";

/// 供应商之间不一样的几处。默认是最常见的写法。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compat {
    /// 输出上限写在哪个字段。
    pub output_limit: OutputLimit,
    /// 思考回不回传、怎么回传。
    pub reasoning: ReasoningReplay,
    /// 要不要在流里报用量（`stream_options.include_usage`）。
    pub stream_usage: bool,
    /// 会不会接着写被打断的回复（`05-内核接口.md` 第七节「接着写被打断的回复」）。
    pub continuation: Continuation,
    /// 开关思考写在哪个字段（施工 8-18）：没有的不能照开关关思考，`off` 只能发 `reasoning_effort: "none"`。装在盒子里：
    /// 大多数供应商没有它，`Compat` 跟着每个请求的目标走，不为它变大。
    pub toggle: Option<Box<Toggle>>,
}

impl Default for Compat {
    /// `max_tokens`，不回传思考，报用量，不会接着写，没有思考的开关。
    fn default() -> Compat {
        Compat {
            output_limit: OutputLimit::MaxTokens,
            reasoning: ReasoningReplay::Drop,
            stream_usage: true,
            continuation: Continuation::None,
            toggle: None,
        }
    }
}

/// 开关思考的字段（施工 8-18，档案的 `compat.toggle`）：思考强度是 [`crate::EFFORT_OFF`]、[`crate::EFFORT_ON`] 时，在请求的
/// 顶层写 `"<field>":<off 或 on>`，值照原样的 JSON。各家写法不一样，是档案里的数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toggle {
    /// 顶层的哪个字段，例如 DeepSeek 的 `thinking`。
    pub field: String,
    /// 开的时候写什么，例如 `{"type":"enabled"}`。
    pub on: serde_json::Value,
    /// 关的时候写什么，例如 `{"type":"disabled"}`。
    pub off: serde_json::Value,
}

impl Compat {
    /// DeepSeek 官方的那一套，测试用（施工 8-6 起出厂的这一套住在档案的 `[providers.deepseek]`，核心的测试守着两边一样）：
    /// 每条 assistant 都带 `reasoning_content`，没有就发空串；接着写用 `prefix: true`，发到 `/beta/chat/completions`，
    /// 半截的思考照常写在 `reasoning_content`。2026-09-27 用真的请求实测过：思考、回复都从截断处接着往下（施工 3-5 补、
    /// 再补）。思考的开关是 `thinking` 的 `type`（施工 8-18）。
    #[cfg(any(test, feature = "testkit"))]
    pub fn deepseek() -> Compat {
        Compat {
            reasoning: ReasoningReplay::Replay {
                field: ReasoningField::ReasoningContent,
                always: true,
            },
            continuation: Continuation::Prefix {
                field: ContinuationField::Prefix,
                path: "/beta/chat/completions".to_string(),
            },
            toggle: Some(Box::new(Toggle {
                field: "thinking".to_string(),
                on: serde_json::json!({"type": "enabled"}),
                off: serde_json::json!({"type": "disabled"}),
            })),
            ..Compat::default()
        }
    }
}

/// 会不会接着写被打断的回复。只给实测过的供应商打开：不猜，也不发请求去探测。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Continuation {
    /// 不会：带着接着写记号的请求也照原样发，被打断的那一句里写着从断的地方接着说。
    None,
    /// 前缀续写：不发最后那句被打断的提示，半截那条 assistant 加上 `field: true`，发到 `path`。
    Prefix {
        /// 半截那条 assistant 上加哪个字段。
        field: ContinuationField,
        /// 这一次发到供应商地址后面的哪一截。
        path: String,
    },
}

/// 接着写的时候，半截那条 assistant 上加的字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuationField {
    /// `prefix`：DeepSeek、Mistral。
    Prefix,
    /// `partial`：Kimi、通义。
    Partial,
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

/// 编码：顶层照 `model`、`messages`、`tools`、`stream`、`stream_options`、输出上限、温度（施工 8-22）、思考强度（施工 8-18，`openai_chat/effort.rs`）的
/// 先后写，别的字段一概不发。带着接着写的记号、供应商又会接着写的，照 [`Continuation::Prefix`] 写，发到它的
/// 路径；别的发到 [`PATH`]。
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
    let continuing = match &compat.continuation {
        Continuation::Prefix { field, path } if request.continuation => Some((*field, path)),
        _ => None,
    };
    let messages = messages::write(
        request,
        call,
        compat,
        texts,
        blobs,
        continuing.map(|(field, _)| field),
    )?;
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
    crate::write_temperature(&mut body, call.temperature);
    effort::write(&mut body, call.effort.as_deref(), compat);
    body.push(b'}');
    Ok(Encoded {
        body,
        messages: ranges,
        path: continuing.map_or(PATH, |(_, path)| path).to_string(),
    })
}

/// 这份请求编码时要用哪些 blob：模型能看图的，要图片；文件每一个都要（施工 3-9 三补）：能读 PDF 的发 PDF，别的
/// 要认是不是文本、要写有多大。执行器照着先取出来。
pub fn blobs_needed(request: &Request, call: &Call) -> BTreeSet<ContentHash> {
    media::blobs_needed(request, call)
}

/// 写成紧凑的 JSON，接在后面。
fn json(body: &mut Vec<u8>, value: &(impl Serialize + ?Sized)) {
    serde_json::to_writer(body, value).expect("线上的消息里没有写不成 JSON 的东西");
}
