//! 哪些算一条、一条的原文怎么写（`docs/blueprint/tools/history.md`「哪些算一条」「一条的原文」，施工 6-4）。
//!
//! 撤掉的、撤回的交给内核的 `History::whole()` 算过，这里只看剩下的：人说的话、她的回复、工具结果、以前的摘要。
//! 她自己翻记录的那几步（`history` 的调用和结果）不算：找的时候会找到自己这一次调用，翻出来的旧结果又和原文重复。

use std::collections::BTreeSet;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::CallId;
use miyu_kernel::template::Template;
use miyu_kernel::time::Timestamp;

use crate::load::say;

/// 一条是谁说的：参数 `by` 的三种写法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Who {
    /// 人说的话。
    User,
    /// 她的回复、以前的摘要。
    Assistant,
    /// 工具结果。
    Tool,
}

impl Who {
    /// 参数里、输出里的写法。
    pub(super) fn name(self) -> &'static str {
        match self {
            Who::User => "user",
            Who::Assistant => "assistant",
            Who::Tool => "tool",
        }
    }

    /// 读参数 `by`；不是这三种的没有。
    pub(super) fn parse(text: &str) -> Option<Who> {
        [Who::User, Who::Assistant, Who::Tool]
            .into_iter()
            .find(|who| who.name() == text)
    }
}

/// 一条：序号、时刻、谁说的、原文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Entry {
    /// 日志的序号。
    pub(super) seq: u64,
    /// 写下的时刻。
    pub(super) at: Timestamp,
    /// 谁说的。
    pub(super) who: Who,
    /// 「读」时这一条下面的原文；「找」也比它。
    pub(super) text: String,
}

/// 图片、文件的占位：`history/image.txt`、`history/file.txt`。
#[derive(Clone)]
pub(super) struct Placeholders {
    pub(super) image: Template,
    pub(super) file: Template,
}

/// 还算数的事件里挑出算一条的，照日志的先后。一个字都没有的（例如只想了没说就被打断的回复）不算；末尾的空白去掉。
/// 工具名是 `own` 的调用和它们的结果不算：她自己翻记录的那几步。
pub(super) fn entries(events: &[Event], placeholders: &Placeholders, own: &str) -> Vec<Entry> {
    let lookups: BTreeSet<CallId> = events
        .iter()
        .filter_map(|event| match &event.body {
            Body::MessageAssistant(reply) => Some(&reply.blocks),
            _ => None,
        })
        .flatten()
        .filter_map(|block| match block {
            Block::ToolCall(call) if call.name == own => Some(call.call_id),
            _ => None,
        })
        .collect();
    let written = |blocks: &[Block]| self::blocks(blocks, placeholders, own);
    events
        .iter()
        .filter_map(|event| {
            let (who, text) = match &event.body {
                Body::MessageUser(message) => (Who::User, written(&message.blocks)),
                Body::MessageAssistant(reply) => (Who::Assistant, written(&reply.blocks)),
                Body::ToolResult(result) if lookups.contains(&result.call_id) => return None,
                Body::ToolResult(result) => (Who::Tool, written(&result.blocks)),
                Body::ContextCompacted(compacted) => (Who::Assistant, compacted.summary.clone()),
                _ => return None,
            };
            let text = text.trim_end();
            (!text.trim_start().is_empty()).then(|| Entry {
                seq: event.seq.get(),
                at: event.at,
                who,
                text: text.to_string(),
            })
        })
        .collect()
}

/// 几块写成原文，一块一段：正文照原样；工具调用写成 `→ <工具名> <参数原文>`，工具名是 `own` 的不写；图片、文件写
/// 占位；思考不给。
fn blocks(blocks: &[Block], placeholders: &Placeholders, own: &str) -> String {
    let parts: Vec<String> = blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.clone()),
            Block::ToolCall(call) if call.name == own => None,
            Block::ToolCall(call) => Some(format!("→ {} {}", call.name, call.args)),
            Block::Image(_) => Some(inline(say(&placeholders.image, &[]))),
            Block::File(file) => Some(inline(say(
                &placeholders.file,
                &[("name", file.name.as_str())],
            ))),
            Block::Reasoning(_) | Block::Unknown(_) => None,
        })
        .collect();
    parts.join("\n")
}

/// 占位写在一行里：去掉字的文件末尾那个换行。
fn inline(text: String) -> String {
    text.trim_end().to_string()
}
