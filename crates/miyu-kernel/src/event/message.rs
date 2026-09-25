//! 消息的事件（`docs/designs/03-事件模型.md` 第三节）。

use serde::{Deserialize, Serialize};

use crate::block::Block;

/// `message.user`：人发来的消息，或另一个会话发来的消息。谁发的写在事件的 `by` 里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageUser {
    /// 消息的内容，一串内容块：文字、图片、文件。
    pub blocks: Vec<Block>,
}

/// `message.assistant`：模型一次响应的完整内容，工具调用也在里面（03 E1）。
/// 哪个模型说的写在事件的 `by` 里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageAssistant {
    /// 响应的内容，一串内容块：文字、思考、工具调用，照模型给出的先后。
    pub blocks: Vec<Block>,
    /// 响应中途被打断了。这时 `blocks` 只有已经收到的部分，参数没收全的工具调用不在里面
    /// （03 第五节）。只在被打断时写这一格；读的时候没有，就是没被打断。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub interrupted: bool,
}

#[cfg(test)]
mod tests;
