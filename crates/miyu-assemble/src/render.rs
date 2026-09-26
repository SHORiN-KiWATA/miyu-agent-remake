//! 把有效历史渲染成消息（`docs/designs/08-上下文投影.md` 第四节「默认的组装怎么写」
//! 第 2 到 4 条）。
//!
//! 检查点排在最前；之后照有效历史排好的先后（[`History::ordered`]）一条条渲染。
//! 人这一边的块（检查点、事实、人的消息）先攒着，碰到模型的回复或工具的结果，
//! 再合成一条 user 消息放在它前面。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, ToolStatus};
use miyu_kernel::history::History;
use miyu_kernel::request::Message;

use crate::texts::Texts;

/// 渲染有效历史：检查点和历史，照先后排好的消息。稳定区不在这里。
pub(crate) fn render(history: &History, texts: &Texts) -> Vec<Message> {
    let mut transcript = Transcript::default();
    if let Some(Body::ContextCompacted(compacted)) = history.checkpoint().map(|event| &event.body) {
        // 摘要原样放进包装里，不转义：它是模型自己写的多行正文，转成一行读不顺。
        transcript.fact(format!(
            "{}{}{}",
            texts.checkpoint_open, compacted.summary, texts.checkpoint_close
        ));
    }
    for event in history.ordered() {
        match &event.body {
            Body::MessageUser(message) => transcript.say(&message.blocks),
            Body::ContextInjected(fact) => transcript.fact(fact.text.clone()),
            Body::TurnEnded(ended) => {
                if let Some(text) = texts.turn_ended.for_reason(&ended.reason) {
                    transcript.fact(text.to_string());
                }
            }
            Body::MessageAssistant(reply) => transcript.push(Message::Assistant {
                blocks: known(&reply.blocks),
            }),
            Body::ToolResult(result) => transcript.push(Message::Tool {
                call_id: result.call_id,
                // 被拒绝、已取消、已跳过、失败，对模型都是「没成」，为什么写在内容里。
                error: result.status != ToolStatus::Ok,
                blocks: known(&result.blocks),
            }),
            // 不进上下文的：回合开始、会话的事件、不认识的种类。压缩和撤销已经由有效历史
            // 用掉了，这里碰不到。一个个列出来，加一种事件时编译器会逼着决定它渲不渲染。
            Body::SessionCreated(_)
            | Body::PolicyChanged(_)
            | Body::MetaChanged(_)
            | Body::TurnStarted(_)
            | Body::TurnReverted(_)
            | Body::ContextCompacted(_)
            | Body::Unknown { .. } => {}
        }
    }
    transcript.finish()
}

/// 渲染到一半的消息，加上人这一边还没合成消息的块。
#[derive(Default)]
struct Transcript {
    /// 已经排好的消息。
    messages: Vec<Message>,
    /// 攒着的检查点和事实，合成时排在前面，照攒进来的先后。
    facts: Vec<Block>,
    /// 攒着的人的消息（`message.user` 的内容块），合成时排在最后：
    /// 当前要回应的那句话离生成位置最近（08 C2）。
    said: Vec<Block>,
}

impl Transcript {
    /// 攒一块事实：检查点、注入的事实、回合没走完的那一句。
    fn fact(&mut self, text: String) {
        self.facts.push(Block::Text(Text { text }));
    }

    /// 攒一条人的消息。
    fn say(&mut self, blocks: &[Block]) {
        self.said.extend(known(blocks));
    }

    /// 放进模型的回复或工具的结果。人这一边攒着的块先合成一条 user 消息，排在它前面。
    fn push(&mut self, message: Message) {
        self.flush();
        self.messages.push(message);
    }

    /// 人这一边攒着的块合成一条 user 消息：事实在前，人的消息在后。什么都没攒就不出消息。
    fn flush(&mut self) {
        if self.facts.is_empty() && self.said.is_empty() {
            return;
        }
        let mut blocks = std::mem::take(&mut self.facts);
        blocks.append(&mut self.said);
        self.messages.push(Message::User { blocks });
    }

    /// 渲染完了：最后攒着的也合成一条。
    fn finish(mut self) -> Vec<Message> {
        self.flush();
        self.messages
    }
}

/// 认识的内容块，照先后。不认识的块原样存在日志里，投影跳过它（`03-事件模型.md` 第八节）。
fn known(blocks: &[Block]) -> Vec<Block> {
    blocks
        .iter()
        .filter(|block| !matches!(block, Block::Unknown(_)))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
