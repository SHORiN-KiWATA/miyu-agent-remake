//! 线上的写法：Anthropic 消息接口的 JSON（`docs/blueprint/drivers/anthropic.md`「怎么走：编码」）。
//!
//! 全用结构体，字段照声明的先后写出去，不经过 `serde_json::Value` 的映射：依赖里哪个 crate 打开了 serde_json 的
//! `preserve_order`，键的先后也不会变。参数格式、调用的参数原文原样照抄。打点不在这里：编码时接在一块的后面（`anthropic.rs`）。

use miyu_kernel::block::Block;
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::{Message, Request};
use serde::Serialize;
use serde_json::value::RawValue;

/// 一条线上的消息：`user` 或者 `assistant`，内容一块一块的。
#[derive(Debug)]
pub(super) struct Wire {
    /// `user` 或者 `assistant`。
    pub(super) role: &'static str,
    /// 内容块，照先后。
    pub(super) content: Vec<Part>,
}

/// 内容里的一块。
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum Part {
    /// 字。
    Text {
        /// 字，原样。
        text: String,
    },
    /// 图片，base64。
    Image {
        /// 媒体类型和内容。
        source: Source,
    },
    /// PDF，base64，带文件名。
    Document {
        /// 媒体类型和内容。
        source: Source,
        /// 文件名。
        title: String,
    },
    /// 带签名的思考。
    Thinking {
        /// 思考的字；这一家默认不给字的，是空的。
        thinking: String,
        /// 签名，原样。
        signature: String,
    },
    /// 加密的思考。
    RedactedThinking {
        /// 加密的内容，原样。
        data: String,
    },
    /// 一次工具调用。
    ToolUse {
        /// 调用的编号。
        id: String,
        /// 工具名。
        name: String,
        /// 参数：原文是一个 JSON 对象的原样，别的是 `{}`。
        input: Box<RawValue>,
    },
    /// 一次调用的结果。
    ToolResult {
        /// 对应的那次调用的编号。
        tool_use_id: String,
        /// 结果里的字、图片、PDF，照 user 的写法。
        content: Vec<Part>,
        /// 算出错的才写。
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

impl Part {
    /// 一块字。
    pub(super) fn text(text: &str) -> Part {
        Part::Text {
            text: text.to_string(),
        }
    }

    /// 能不能打点（「缓存打点」第 2 条）：思考块不能，别的能。
    pub(super) fn markable(&self) -> bool {
        !matches!(self, Part::Thinking { .. } | Part::RedactedThinking { .. })
    }
}

/// 图片、PDF 的内容：总是 base64。
#[derive(Debug, Serialize)]
pub(super) struct Source {
    /// 总是 `base64`。
    #[serde(rename = "type")]
    pub(super) kind: &'static str,
    /// 媒体类型。
    pub(super) media_type: String,
    /// base64 的内容。
    pub(super) data: String,
}

/// 工具面上的一件工具。
#[derive(Debug, Serialize)]
pub(super) struct Tool<'a> {
    name: &'a str,
    description: &'a str,
    /// 参数格式原样照抄。
    input_schema: &'a RawJson,
}

/// 工具面：没有工具、历史里也没有工具调用的不发；历史里有调用的，没有工具也发一个空的（这一家要带着调用的请求有
/// `tools`，「怎么走：编码」第 8 条）。
pub(super) fn tools(request: &Request) -> Option<Vec<Tool<'_>>> {
    if request.tools.is_empty() && !calls_tools(request) {
        return None;
    }
    Some(
        request
            .tools
            .iter()
            .map(|tool| Tool {
                name: &tool.name,
                description: &tool.description,
                input_schema: &tool.parameters,
            })
            .collect(),
    )
}

/// 历史里有没有工具调用。
fn calls_tools(request: &Request) -> bool {
    request.messages.iter().any(|message| match message {
        Message::Assistant { blocks } => blocks
            .iter()
            .any(|block| matches!(block, Block::ToolCall(_))),
        Message::User { .. } | Message::Tool { .. } => false,
    })
}
