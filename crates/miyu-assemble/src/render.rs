//! 把有效历史渲染成消息（`docs/designs/08-上下文投影.md` 第四节「默认的组装怎么写」
//! 第 2 到 4 条）。
//!
//! 检查点排在最前；之后照有效历史排好的先后（[`History::ordered`]）一条条渲染。
//! 人这一边的块（检查点、事实、人的消息）先攒着，碰到模型的回复或工具的结果，再合成一条
//! user 消息放在它前面：照攒进来的先后，只有一处例外，每个回合开始的地方，放这一回合开始时
//! 注入的事实和触发它的那条，先事实、后触发。「开始时注入的」到这一轮有了回复、结束，或者第一次
//! 记下模型调用为止（施工 4-9 再补三上）：出错了等着重试时再注入的，照先后放，前缀才接得上。

use std::collections::{BTreeMap, BTreeSet};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, ContextCompacted, ToolStatus};
use miyu_kernel::history::History;
use miyu_kernel::id::{Seq, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::request::Message;

use crate::texts::Texts;

/// 渲染有效历史：检查点和历史，照先后排好的消息。稳定区不在这里。
pub(crate) fn render(history: &History, texts: &Texts) -> Vec<Message> {
    let mut transcript = Transcript::default();
    if let Some(checkpoint) = history.checkpoint()
        && let Body::ContextCompacted(compacted) = &checkpoint.body
    {
        transcript.add(
            checkpoint.seq,
            None,
            vec![text_block(checkpoint_block(history, compacted, texts))],
        );
    }
    for event in history.ordered() {
        match &event.body {
            Body::MessageUser(message) => transcript.add(event.seq, None, known(&message.blocks)),
            Body::ContextInjected(fact) => {
                let before = transcript.trigger_of(event.turn);
                transcript.add(event.seq, before, vec![text_block(fact.text.clone())]);
            }
            Body::TurnStarted(started) => transcript.start(event.turn, started.trigger),
            // 这一轮请求过一次了：之后注入的不再是开始时的（施工 4-9 再补三上）。它自己不进上下文。压缩以前记下的
            // 不算（施工 6-2 上）：那是被替代掉的那段的请求和摘要请求，压完的第一次主请求前缀本来就从头来，回合开头压的，
            // 压完再注入的事实照样和触发的那句放在一起。
            Body::ModelCalled(_) if before_checkpoint(history, event.seq) => {}
            Body::ModelCalled(_) => transcript.settle(),
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
            // 不进上下文的：会话的事件、请人确认和人的决定、问人和人的回答（她看到的只有工具
            // 结果）、改回文件的结局（她不知道被撤过）、暂停了自动压缩（给人看的）、不认识的种类。压缩、撤销、恢复、撤回已经由
            // 有效历史用掉了，这里碰不到。一个个列出来，加一种事件时编译器会逼着决定它渲不渲染。
            Body::SessionCreated(_)
            | Body::PolicyChanged(_)
            | Body::MetaChanged(_)
            | Body::TurnReverted(_)
            | Body::TurnUnreverted(_)
            | Body::FilesRestored(_)
            | Body::MessageWithdrawn(_)
            | Body::ApprovalRequested(_)
            | Body::ApprovalDecided(_)
            | Body::QuestionAsked(_)
            | Body::QuestionAnswered(_)
            | Body::ContextCompacted(_)
            | Body::CompactionPaused(_)
            | Body::Unknown { .. } => {}
        }
    }
    transcript.finish()
}

/// 第 `seq` 条排在检查点前面：被动压缩、回合开头压缩留下的尾巴里的，和摘要请求自己的 `model.called`。
/// 检查点那一块（`kernel/request.md`「组装」第 2 条）：包装的开头、摘要、摘要的收尾、代码写的几段、重读的文件、包装的
/// 结尾。摘要、重读的原文原样放，不转义：一个是模型自己写的多行正文，一个是文件本来的样子。重读的原文照 blob 从有效
/// 历史取，取不到的那一份整块不写（施工 6-5）。
fn checkpoint_block(history: &History, compacted: &ContextCompacted, texts: &Texts) -> String {
    let mut block = format!(
        "{}{}{}{}",
        texts.checkpoint_open, compacted.summary, texts.checkpoint_close, compacted.notes
    );
    if let Some(wrap) = &texts.restored {
        for file in &compacted.restored {
            let Some(text) = history.recalled(&file.blob) else {
                continue;
            };
            let fields = BTreeMap::from([("path", file.path.as_str())]);
            block.push_str(&wrap.open.render(&fields).unwrap_or_default());
            block.push_str(text);
            block.push_str(&wrap.close);
        }
    }
    block.push_str(&texts.checkpoint_end);
    block
}

fn before_checkpoint(history: &History, seq: Seq) -> bool {
    history
        .checkpoint()
        .is_some_and(|checkpoint| seq < checkpoint.seq)
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

    /// 回合里有了回复、请求过一次，或者回合结束了：之后注入的事实，照先后放。
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

/// 这一次是不是接着写（`05-内核接口.md` 第七节「接着写被打断的回复」，施工 3-5 再补）：有效历史的
/// 最后，是一条带 `interrupted` 的回复，后面只有一条内核记的 `reply_cut` 事实，中间只隔着
/// `model.called`。这时渲染出来的最后一条 user 消息里只有被打断的那一句，前面那条 assistant 是
/// 半截。那一句之后又来了别的（人的消息、切了级别以后的事实），不算：最后那条 user 里不只有那一句，
/// 去不掉。
pub(crate) fn continues(history: &History) -> bool {
    let mut tail = history
        .ordered()
        .into_iter()
        .rev()
        .filter(|event| !matches!(event.body, Body::ModelCalled(_)));
    let noticed = tail.next().is_some_and(|event| {
        event.by == By::Kernel
            && matches!(&event.body, Body::ContextInjected(fact) if fact.kind.as_str() == "reply_cut")
    });
    noticed
        && tail.next().is_some_and(
            |event| matches!(&event.body, Body::MessageAssistant(reply) if reply.interrupted),
        )
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
