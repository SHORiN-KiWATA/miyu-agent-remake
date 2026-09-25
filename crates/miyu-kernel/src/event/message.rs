//! 消息的事件（`docs/designs/03-事件模型.md` 第三节）。

use serde::{Deserialize, Serialize};

use crate::block::Block;

/// `message.user`：人发来的消息，或另一个会话发来的消息。谁发的写在事件的 `by` 里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageUser {
    /// 消息的内容，一串内容块：文字、图片、文件。
    pub blocks: Vec<Block>,
}
