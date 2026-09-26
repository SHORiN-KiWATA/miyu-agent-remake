//! 把有效历史渲染成消息（`docs/designs/08-上下文投影.md` 第四节「默认的组装怎么写」
//! 第 2 到 4 条）。
//!
//! 检查点排在最前；之后照有效历史排好的先后（[`History::ordered`]）一条条渲染。
//! 人这一边的块（检查点、事实、人的消息）先攒着，碰到模型的回复或工具的结果，再合成一条
//! user 消息放在它前面：照攒进来的先后，只有一处例外，每个回合开始的地方，放这一回合开始时
//! 注入的事实和触发它的那条，先事实、后触发。

use std::collections::{BTreeMap, BTreeSet};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, ToolStatus};
use miyu_kernel::history::History;
use miyu_kernel::id::{Seq, TurnId};
use miyu_kernel::request::Message;

use crate::texts::Texts;

/// 渲染有效历史：检查点和历史，照先后排好的消息。稳定区不在这里。
pub(crate) fn render(history: &History, texts: &Texts) -> Vec<Message> {
    let mut transcript = Transcript::default();
    if let Some(checkpoint) = history.checkpoint()
        && let Body::ContextCompacted(compacted) = &checkpoint.body
    {
        // 摘要原样放进包装里，不转义：它是模型自己写的多行正文，转成一行读不顺。
        let wrapped = format!(
            "{}{}{}",
            texts.checkpoint_open, compacted.summary, texts.checkpoint_close
        );
        transcript.add(checkpoint.seq, None, vec![text_block(wrapped)]);
    }
    for event in history.ordered() {
        match &event.body {
            Body::MessageUser(message) => transcript.add(event.seq, None, known(&message.blocks)),
            Body::ContextInjected(fact) => {
                let before = transcript.trigger_of(event.turn);
                transcript.add(event.seq, before, vec![text_block(fact.text.clone())]);
            }
            Body::TurnStarted(started) => transcript.start(event.turn, started.trigger),
            Body::TurnEnded(ended) => {
                transcript.settle();
                if let Some(said) = texts.turn_ended.for_reason(&ended.reason) {
                    transcript.add(event.seq, None, vec![text_block(said.to_string())]);
                }
            }
            Body::MessageAssistant(reply) => {
                transcript.settle();
                transcript.push(Message::Assistant {
                    blocks: known(&reply.blocks),
                });
            }
            Body::ToolResult(result) => transcript.push(Message::Tool {
                call_id: result.call_id,
                // 被拒绝、已取消、已跳过、失败，对模型都是「没成」，为什么写在内容里。
                error: result.status != ToolStatus::Ok,
                blocks: known(&result.blocks),
            }),
            // 不进上下文的：会话的事件、模型调用的记录、不认识的种类。压缩和撤销已经由
            // 有效历史用掉了，这里碰不到。一个个列出来，加一种事件时编译器会逼着决定它渲不渲染。
            Body::SessionCreated(_)
            | Body::PolicyChanged(_)
            | Body::MetaChanged(_)
            | Body::TurnReverted(_)
            | Body::ContextCompacted(_)
            | Body::ModelCalled(_)
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
    /// 人这一边攒着的块，照攒进来的先后。
    pending: Vec<Piece>,
    /// 这一段里每个回合开始的地方：攒到第几块时开始的，由哪一条触发。
    starts: Vec<(usize, Seq)>,
    /// 刚开始、还没回复过的回合，和触发它的那一条。这时注入的事实，是回合开始时注入的。
    starting: Option<(TurnId, Seq)>,
}

/// 人这一边攒着的一块。
struct Piece {
    /// 它是哪一条事件的。
    seq: Seq,
    /// 回合开始时注入的事实，和触发这一回合的那一条放在一起；别的块是 `None`。
    before: Option<Seq>,
    /// 块本身。
    block: Block,
}

impl Transcript {
    /// 攒下第 `seq` 条事件的几块。
    fn add(&mut self, seq: Seq, before: Option<Seq>, blocks: Vec<Block>) {
        self.pending
            .extend(blocks.into_iter().map(|block| Piece { seq, before, block }));
    }

    /// 回合开始了，由第 `trigger` 条触发。记下开始的地方。
    fn start(&mut self, turn: Option<TurnId>, trigger: Seq) {
        self.starting = turn.map(|turn| (turn, trigger));
        self.starts.push((self.pending.len(), trigger));
    }

    /// 这一回合注入的事实该和哪一条放在一起：回合刚开始、还没回复过，就是触发它的那一条。
    fn trigger_of(&self, turn: Option<TurnId>) -> Option<Seq> {
        match self.starting {
            Some((starting, trigger)) if turn == Some(starting) => Some(trigger),
            _ => None,
        }
    }

    /// 回合里有了回复，或者回合结束了：之后注入的事实，照先后放。
    fn settle(&mut self) {
        self.starting = None;
    }

    /// 放进模型的回复或工具的结果。人这一边攒着的块先合成一条 user 消息，排在它前面。
    fn push(&mut self, message: Message) {
        self.flush();
        self.messages.push(message);
    }

    /// 人这一边攒着的块合成一条 user 消息：照攒进来的先后；每个回合开始的地方，放这一回合
    /// 开始时注入的事实和触发它的那条，先事实、后触发（08 C2）。触发的那一条不在这一段里，
    /// 事实就照原来的先后。什么都没攒就不出消息。
    ///
    /// 挪到的是回合开始的那个位置，日志里它不会动。所以发过的请求里已经排好的先后，以后也
    /// 不会变，前缀接得上。
    fn flush(&mut self) {
        let starts = std::mem::take(&mut self.starts);
        if self.pending.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending);
        let here: BTreeSet<Seq> = pending.iter().map(|piece| piece.seq).collect();
        // 触发在这一段里的，才挪。
        let starts: Vec<(usize, Seq)> = starts
            .into_iter()
            .filter(|(_, trigger)| here.contains(trigger))
            .collect();
        let placed: BTreeSet<Seq> = starts.iter().map(|&(_, trigger)| trigger).collect();
        let mut groups: BTreeMap<Seq, Group> = BTreeMap::new();
        let mut rest = Vec::new();
        for (index, piece) in pending.into_iter().enumerate() {
            match piece.before {
                Some(trigger) if placed.contains(&trigger) => {
                    groups.entry(trigger).or_default().facts.push(piece.block);
                }
                _ if placed.contains(&piece.seq) => {
                    groups
                        .entry(piece.seq)
                        .or_default()
                        .trigger
                        .push(piece.block);
                }
                _ => rest.push((index, piece.block)),
            }
        }
        let mut blocks = Vec::new();
        let mut starts = starts.into_iter().peekable();
        for (index, block) in rest {
            while let Some((_, trigger)) = starts.next_if(|&(at, _)| at <= index) {
                if let Some(group) = groups.remove(&trigger) {
                    group.put(&mut blocks);
                }
            }
            blocks.push(block);
        }
        for (_, trigger) in starts {
            if let Some(group) = groups.remove(&trigger) {
                group.put(&mut blocks);
            }
        }
        self.messages.push(Message::User { blocks });
    }

    /// 渲染完了：最后攒着的也合成一条。
    fn finish(mut self) -> Vec<Message> {
        self.flush();
        self.messages
    }
}

/// 一个回合开始时的那一组：开始时注入的事实，和触发它的那一条。
#[derive(Default)]
struct Group {
    /// 回合开始时注入的事实。
    facts: Vec<Block>,
    /// 触发这一回合的那一条。
    trigger: Vec<Block>,
}

impl Group {
    /// 放进消息里：先事实，后触发。
    fn put(self, blocks: &mut Vec<Block>) {
        blocks.extend(self.facts);
        blocks.extend(self.trigger);
    }
}

/// 一个文本块。
fn text_block(text: String) -> Block {
    Block::Text(Text { text })
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
