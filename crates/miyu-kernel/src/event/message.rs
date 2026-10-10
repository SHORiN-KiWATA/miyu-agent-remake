//! 消息的事件（`docs/designs/03-事件模型.md` 第三节）。

use serde::{Deserialize, Serialize};

use crate::block::Block;
use crate::id::Seq;

use super::VenueMessage;

/// `message.user`：人发来的消息，或另一个会话发来的消息。谁发的写在事件的 `by` 里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageUser {
    /// 消息的内容，一串内容块：文字、图片、文件。
    pub blocks: Vec<Block>,
    /// 通讯平台上的一条消息（施工 O-13 上）：平台编号、引用、@ 了谁、带的东西、旁听。本机的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venue: Option<VenueMessage>,
}

/// `message.assistant`：模型一次响应的完整内容，工具调用也在里面（03 E1）。
/// 哪个模型说的写在事件的 `by` 里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageAssistant {
    /// 响应的内容，一串内容块：文字、思考、工具调用，照模型给出的先后。
    pub blocks: Vec<Block>,
    /// 发这次请求时，日志到第几条为止：这条回复就是看着它们写的。请求在路上时到的事件
    /// 序号比它大，投影时排在这条回复后面（03 第六节「照每次请求看到的范围排」）。
    pub seen: Seq,
    /// 响应中途被打断了。这时 `blocks` 只有已经收到的部分，参数没收全的工具调用不在里面
    /// （03 第五节）。只在被打断时写这一格；读的时候没有，就是没被打断。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub interrupted: bool,
    /// 每一块在流里是第几块（`model.delta` 的 `index`），照 `blocks` 的先后（施工 9-8 上，视图投影照它给块起编号）。
    /// 空块、丢掉的工具调用不进回复，后面的块的位置和块号就错开了：只在错开时写，和位置一样的不写；以前的日志
    /// 没有这一格。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub indexes: Vec<usize>,
}

impl MessageAssistant {
    /// 留下的每一块在流里是第几块（`kept`，照留下的先后）写成 `indexes`：和位置一样的不写（施工 9-8 上）。
    #[must_use]
    pub fn indexes_for(kept: &[usize]) -> Vec<usize> {
        match kept.iter().copied().eq(0..kept.len()) {
            true => Vec::new(),
            false => kept.to_vec(),
        }
    }

    /// 第 `position` 块在流里是第几块：写了 `indexes` 的照它，没写的就是位置。
    #[must_use]
    pub fn index_of(&self, position: usize) -> usize {
        self.indexes.get(position).copied().unwrap_or(position)
    }
}

/// `message.withdrawn`：撤回排着队、她还没听到的消息（`02-内核.md` 第六节「排队的消息」）。
/// 谁撤回的写在事件的 `by` 里。撤回的消息和这一条都不进有效历史（03 第七节）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageWithdrawn {
    /// 撤回了哪几条 `message.user`，照序号的先后。
    pub messages: Vec<Seq>,
}

#[cfg(test)]
mod tests;
