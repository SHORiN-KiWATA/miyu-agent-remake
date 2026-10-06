//! 线上的写法：Responses 接口 `input` 里的一项项、工具面（`docs/blueprint/drivers/openai-responses.md`「怎么走：编码」）。
//!
//! 全用结构体，字段照声明的先后写出去，不经过 `serde_json::Value` 的映射。参数格式原样照抄。

use miyu_kernel::block::Block;
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::{Message, Request};
use serde::Serialize;

/// `input` 里的一项：带角色的消息，或者带种类的一项。
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(super) enum Item {
    /// `{"role":…,"content":…}`。
    Role(RoleMessage),
    /// `{"type":…,…}`。
    Typed(Typed),
}

/// 带角色的消息。
#[derive(Debug, Serialize)]
pub(super) struct RoleMessage {
    /// `user` 或者 `assistant`。
    pub(super) role: &'static str,
    /// 全是文字的是一个字符串，有图片、文件的分成几段。
    pub(super) content: Content,
}

/// 带种类的一项。
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum Typed {
    /// 回传的思考：`id`、摘要、加密的内容。
    Reasoning {
        /// 这一项的编号。
        id: String,
        /// 摘要；字是空的写 `[]`。
        summary: Vec<Summary>,
        /// 加密的思考，原样。
        encrypted_content: String,
    },
    /// 一次工具调用，不写 `id`（服务端没存，写了会去找）。
    FunctionCall {
        /// 调用的编号。
        call_id: String,
        /// 工具名。
        name: String,
        /// 参数原文：一个字符串。
        arguments: String,
    },
    /// 一次调用的结果。
    FunctionCallOutput {
        /// 对应的那次调用的编号。
        call_id: String,
        /// 全是文字的是一个字符串，有图片、文件的分成几段。
        output: Content,
    },
}

/// 一段摘要。
#[derive(Debug, Serialize)]
pub(super) struct Summary {
    /// 总是 `summary_text`。
    #[serde(rename = "type")]
    pub(super) kind: &'static str,
    /// 字。
    pub(super) text: String,
}

/// 消息、结果的内容。
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(super) enum Content {
    /// 全是文字。
    Text(String),
    /// 分成几段。
    Parts(Vec<Part>),
}

/// 分段的内容里的一段：线上的种类是 `input_text`、`input_image`、`input_file`。
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub(super) enum Part {
    /// 字。
    #[serde(rename = "input_text")]
    Text {
        /// 字。
        text: String,
    },
    /// 图片，写成 data URL。
    #[serde(rename = "input_image")]
    Image {
        /// `data:…;base64,…`。
        image_url: String,
    },
    /// 文件（PDF），写成 data URL。
    #[serde(rename = "input_file")]
    File {
        /// 文件名。
        filename: String,
        /// `data:…;base64,…`。
        file_data: String,
    },
}

/// 工具面上的一件工具。
#[derive(Debug, Serialize)]
pub(super) struct Tool<'a> {
    /// 总是 `function`。
    #[serde(rename = "type")]
    kind: &'static str,
    name: &'a str,
    description: &'a str,
    /// 参数格式原样照抄。
    parameters: &'a RawJson,
    /// 一定写假的：这一家默认严格，严格模式要每个参数都必填（图纸「起草时定的」第 3 条）。
    strict: bool,
}

/// 工具面：没有工具、历史里也没有工具调用的不发；历史里有调用的，没有工具也发一个空的。
pub(super) fn tools(request: &Request) -> Option<Vec<Tool<'_>>> {
    if request.tools.is_empty() && !calls_tools(request) {
        return None;
    }
    Some(
        request
            .tools
            .iter()
            .map(|tool| Tool {
                kind: "function",
                name: &tool.name,
                description: &tool.description,
                parameters: &tool.parameters,
                strict: false,
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
