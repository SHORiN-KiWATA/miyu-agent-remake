//! 线上的写法：OpenAI 兼容对话接口的 JSON（`05-内核接口.md` 第七节那张表）。
//!
//! 全用结构体，字段照声明的先后写出去，不经过 `serde_json::Value` 的映射：依赖里哪个 crate 打开了
//! serde_json 的 `preserve_order`，键的先后也不会变。参数格式原样照抄。

use miyu_kernel::block::Block;
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::{Message, Request};
use serde::Serialize;

/// 一条线上的消息，照 `role` 分开四种。
#[derive(Debug, Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub(super) enum Wire {
    /// 最前面的 system。
    System {
        /// 系统提示词。
        content: String,
    },
    /// user：人这一边的，和挪出来的图片、文件。
    User {
        /// 全是文字的是一个字符串，有图片、文件的分成几段。
        content: Content,
    },
    /// assistant：模型说过的。
    Assistant {
        /// 正文；没有正文、只有工具调用的是 `null`。
        content: Option<String>,
        /// 回传的思考，写在 `reasoning_content` 的。
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning_content: Option<String>,
        /// 回传的思考，写在 `reasoning` 的。
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning: Option<String>,
        /// 工具调用。
        #[serde(skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
    },
    /// tool：一次调用的结果，只有文字。
    Tool {
        /// 对应的那次调用的编号。
        tool_call_id: String,
        /// 结果的文字。
        content: String,
    },
}

/// user 消息的内容。
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(super) enum Content {
    /// 全是文字。
    Text(String),
    /// 分成几段。
    Parts(Vec<Part>),
}

/// 分段的内容里的一段。
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum Part {
    /// 文字。
    Text {
        /// 字。
        text: String,
    },
    /// 图片，写成 data URL。
    ImageUrl {
        /// `{"url":"data:…;base64,…"}`。
        image_url: Url,
    },
    /// 文件（PDF），写成 data URL。
    File {
        /// 文件名和内容。
        file: FileData,
    },
}

/// 图片的地址。
#[derive(Debug, Serialize)]
pub(super) struct Url {
    /// data URL。
    pub(super) url: String,
}

/// 文件的名字和内容。
#[derive(Debug, Serialize)]
pub(super) struct FileData {
    /// 文件名。
    pub(super) filename: String,
    /// data URL。
    pub(super) file_data: String,
}

/// assistant 消息里的一次工具调用。
#[derive(Debug, Serialize)]
pub(super) struct ToolCall {
    /// 调用的编号。
    pub(super) id: String,
    /// 总是 `function`。
    #[serde(rename = "type")]
    pub(super) kind: &'static str,
    /// 工具名和参数。
    pub(super) function: FunctionCall,
}

/// 工具名和参数原文。
#[derive(Debug, Serialize)]
pub(super) struct FunctionCall {
    /// 工具名。
    pub(super) name: String,
    /// 参数原文：一个字符串。
    pub(super) arguments: String,
}

/// 工具面上的一件工具。
#[derive(Debug, Serialize)]
pub(super) struct Tool<'a> {
    /// 总是 `function`。
    #[serde(rename = "type")]
    kind: &'static str,
    /// 名字、说明、参数格式。
    function: Function<'a>,
}

/// 名字、说明、参数格式。
#[derive(Debug, Serialize)]
struct Function<'a> {
    name: &'a str,
    description: &'a str,
    /// 参数格式原样照抄。
    parameters: &'a RawJson,
}

/// 工具面：没有工具时不发；可历史里有工具调用的，发一个空的（有的网关要）。
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
                function: Function {
                    name: &tool.name,
                    description: &tool.description,
                    parameters: &tool.parameters,
                },
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
