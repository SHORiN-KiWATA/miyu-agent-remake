//! 起标题的请求（施工 3-8 五补，`docs/blueprint/kernel/request.md`「起标题的请求」）：照回顾的请求的写法（`recap.rs`），单独
//! 一次辅助请求，不接主对话的前缀、不带 system 和工具面，一条 user：指令接对话记录。
//!
//! 只喂第一轮：有效历史里她第一个回答以前人这边的话（挨着的几段并成一段，别的 harness、子代理的照主请求里的外壳写），和那个
//! 回答（那一轮最后一条有正文的回复）的正文。标签、截断的记号借回顾的；整份连指令至多 `tokens` 个 token（字节/4），放不下的
//! 两段各留头尾、截掉中间（`recap::transcript`），最后整份截到上限、只在字的边界上截。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::estimate::BYTES_PER_TOKEN;
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;
use miyu_kernel::request::{Message, Request};

use crate::recap::{self, Exchange, GAP};
use crate::texts::Texts;

/// 起标题的请求，和它照到的那一条：第一个回答的序号。快照里没有起标题的字、没有回顾的标签的，她一个带正文的回复都
/// 没有的，没有。
pub(crate) fn request(history: &History, texts: &Texts) -> Option<(Request, Seq)> {
    let title = texts.title.as_ref()?;
    let labels = texts.recap.as_ref()?;
    let first = first_exchange(history, texts)?;
    let upto = first.last?;
    let limit = usize::try_from(title.tokens.saturating_mul(BYTES_PER_TOKEN)).unwrap_or(usize::MAX);
    let room = limit.saturating_sub(title.instruction.len());
    let mut transcript = recap::transcript(&[first], labels, room);
    transcript.truncate(transcript.floor_char_boundary(room));
    let text = format!("{}{transcript}", title.instruction);
    let request = Request {
        tools: Vec::new(),
        system: String::new(),
        messages: vec![Message::User {
            blocks: vec![Block::Text(Text { text })],
        }],
        stable: 0,
        continuation: false,
        described: Default::default(),
        output_cap: None,
    };
    Some((request, upto))
}

/// 第一轮：第一个回答以前人这边的话（挨着的并成一段），和那个回答。照到的是那个回答。一个回答都没有的，没有。
fn first_exchange(history: &History, texts: &Texts) -> Option<Exchange> {
    let mut first = Exchange::default();
    for entry in recap::entries(history, texts) {
        if entry.assistant {
            first.assistant = entry.text;
            first.last = Some(entry.seq);
            return Some(first);
        }
        if !first.user.is_empty() {
            first.user.push_str(GAP);
        }
        first.user.push_str(&entry.text);
    }
    None
}

#[cfg(test)]
mod tests;
