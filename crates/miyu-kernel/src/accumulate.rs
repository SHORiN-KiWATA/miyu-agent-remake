//! 流式累积器：模型的输出一段一段地来，拼成完整的内容块（`docs/designs/03-事件模型.md`
//! 第五节「增量和累积器怎么写」）。
//!
//! 驱动把各家的响应分片解码成四种统一的增量（[`Delta`]），累积器照块收下来。正常说完，
//! 拼成一条回复的全部内容（[`Accumulator::finish`]）；被打断，只留收全了的部分
//! （[`Accumulator::cut_off`]）。一次响应一个累积器。

use std::fmt;

use crate::block::{Block, Private, Reasoning, Text, ToolCall};
use crate::id::{CallId, Seq};

/// 模型响应的一段增量：驱动解码出来的统一写法（`05-内核接口.md` 第七节）。
///
/// 块照开始的先后编号，从 0 数起，一块接一块；几块的字可以交错着来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delta {
    /// 一块开始了。
    Start {
        /// 第几块。
        index: usize,
        /// 这一块是什么。
        kind: Kind,
    },
    /// 这一块的一段字：正文的、思考的，或者工具调用参数原文的一段。
    Text {
        /// 第几块。
        index: usize,
        /// 这一段字。
        text: String,
    },
    /// 这一块的私有数据：思考块的签名、供应商自己的调用编号（03 第九节）。
    /// 只有思考和工具调用有，一块最多一份。
    Private {
        /// 第几块。
        index: usize,
        /// 数据本身，原样留着。
        private: Private,
    },
    /// 这一块收全了。
    End {
        /// 第几块。
        index: usize,
    },
}

/// 一块的种类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// 正文。
    Text,
    /// 思考。
    Reasoning,
    /// 工具调用。
    ToolCall {
        /// 模型说要调用的工具名。
        name: String,
    },
}

/// 一次响应的累积器。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Accumulator {
    /// 已经开始的块，照开始的先后。
    blocks: Vec<Building>,
}

/// 拼到一半的一块。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Building {
    kind: Kind,
    /// 收到的字，照先后接起来。
    text: String,
    private: Option<Private>,
    /// 收全了没有。
    ended: bool,
}

impl Accumulator {
    /// 收下一段增量。
    ///
    /// # Errors
    ///
    /// 增量对不上，是驱动的错，返回 [`DeltaError`]，写明是哪一块、哪里对不上：跳过的编号、
    /// 又开始一次的块、还没开始的块、收全以后又来的、给正文块的私有数据、来了两次的私有数据。
    pub fn apply(&mut self, delta: Delta) -> Result<(), DeltaError> {
        match delta {
            Delta::Start { index, kind } => {
                if index != self.blocks.len() {
                    let why = if index < self.blocks.len() {
                        "这一块已经开始过了"
                    } else {
                        "跳过了编号，块要一块接一块地开始"
                    };
                    return Err(DeltaError::new(index, why));
                }
                self.blocks.push(Building {
                    kind,
                    text: String::new(),
                    private: None,
                    ended: false,
                });
            }
            Delta::Text { index, text } => self.open(index)?.text.push_str(&text),
            Delta::Private { index, private } => {
                let block = self.open(index)?;
                if block.kind == Kind::Text {
                    return Err(DeltaError::new(index, "正文块没有私有数据"));
                }
                if block.private.is_some() {
                    return Err(DeltaError::new(index, "私有数据来了两次"));
                }
                block.private = Some(private);
            }
            Delta::End { index } => self.open(index)?.ended = true,
        }
        Ok(())
    }

    /// 正常说完：每一块照收到的拼起来。工具调用的编号照这条回复的序号 `reply` 分配，写成
    /// `call_<序号>_<第几个>`；空块不要。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：回复的序号是合法的序号，第几个从 1 数起。
    pub fn finish(self, reply: Seq) -> Vec<Block> {
        self.build(reply, true)
    }

    /// 被打断：正文和思考，收到多少留多少；工具调用只留收全了的，没收全的参数不完整，
    /// 执行不了。留下的调用，编号照样一个接一个。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：同 [`Accumulator::finish`]。
    pub fn cut_off(self, reply: Seq) -> Vec<Block> {
        self.build(reply, false)
    }

    /// 还没收全、可以接着收的第 `index` 块。
    fn open(&mut self, index: usize) -> Result<&mut Building, DeltaError> {
        match self.blocks.get_mut(index) {
            None => Err(DeltaError::new(index, "这一块还没开始")),
            Some(block) if block.ended => Err(DeltaError::new(index, "这一块已经收全了")),
            Some(block) => Ok(block),
        }
    }

    /// 拼成内容块。`unended_calls` 为假时，没收全的工具调用丢掉。
    fn build(self, reply: Seq, unended_calls: bool) -> Vec<Block> {
        let mut out = Vec::new();
        let mut calls = 0;
        for block in self.blocks {
            match block.kind {
                Kind::Text if !block.text.is_empty() => {
                    out.push(Block::Text(Text { text: block.text }))
                }
                Kind::Text => {}
                Kind::Reasoning if block.text.is_empty() && block.private.is_none() => {}
                Kind::Reasoning => out.push(Block::Reasoning(Reasoning {
                    text: block.text,
                    private: block.private,
                })),
                Kind::ToolCall { name } if block.ended || unended_calls => {
                    calls += 1;
                    let call_id =
                        CallId::new(reply, calls).expect("回复的序号合法，第几个从 1 数起");
                    out.push(Block::ToolCall(ToolCall {
                        call_id,
                        name,
                        args: block.text,
                        private: block.private,
                    }));
                }
                Kind::ToolCall { .. } => {}
            }
        }
        out
    }
}

/// 增量对不上：驱动的错。这次响应按出错算（施工 2-3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeltaError {
    /// 第几块。
    pub index: usize,
    /// 哪里对不上。
    pub why: String,
}

impl DeltaError {
    fn new(index: usize, why: &str) -> DeltaError {
        DeltaError {
            index,
            why: why.to_string(),
        }
    }
}

impl fmt::Display for DeltaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "模型的增量对不上，第 {} 块：{}", self.index, self.why)
    }
}

impl std::error::Error for DeltaError {}

#[cfg(test)]
mod tests;
