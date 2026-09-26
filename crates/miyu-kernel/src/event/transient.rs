//! 瞬时事件：不落库，只推给正在连接的头，用于实时显示（`docs/designs/03-事件模型.md`
//! 第五节「瞬时事件的外壳」）。
//!
//! 外壳和持久事件同一种写法，只少了 `seq`：它不进日志。`cause` 留着，一个命令引起的事，
//! 从持久的到瞬时的都能一路追下去。内核只推不读，所以这里只写出去；读回来的那一半，
//! 头读它们的时候再写（M8）。

use serde::{Serialize, Serializer};

use crate::accumulate::Kind;
use crate::id::{CommandId, Seq, TurnId};
use crate::origin::By;
use crate::time::Timestamp;

/// 一条瞬时事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transient {
    /// 发生的时刻，取自执行器送进来的时钟输入。
    pub at: Timestamp,
    /// 所属回合；不属于任何回合时没有。
    pub turn: Option<TurnId>,
    /// 由谁引起。
    pub by: By,
    /// 引起它的命令。
    pub cause: Option<CommandId>,
    /// 种类，连同它自己的内容。
    pub body: TransientBody,
}

/// 瞬时事件的种类，连同它自己的内容。另外两种（`tool.progress`、`status`）随施工 2-4、3-5。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransientBody {
    /// `model.delta`：模型输出的一段增量。
    ModelDelta(ModelDelta),
}

/// `model.delta` 的 `body`：哪次请求的、第几块、这一段增量。私有数据不推，头用不着。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDelta {
    /// 这次请求看到了第几条为止，和这次响应最后写成的回复的 `seen` 一样。
    pub seen: Seq,
    /// 第几块，从 0 数起。
    pub index: usize,
    /// 这一段增量。
    pub piece: Piece,
}

/// 推给头的一段增量：累积器收的四种里，除了私有数据的三种（03 第五节）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// 一块开始了，是什么块。
    Start(Kind),
    /// 这一块的一段字。
    Text(String),
    /// 这一块收全了。
    End,
}

impl TransientBody {
    /// 外壳里 `kind` 那一格写的名字。
    pub fn kind(&self) -> &'static str {
        match self {
            TransientBody::ModelDelta(_) => "model.delta",
        }
    }
}

impl Transient {
    /// 写成推给头的一行：紧凑的 JSON，字段照图纸的顺序，不带换行。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：里面只有字符串、数字和原样的 JSON，写成 JSON 不会失败。
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("瞬时事件里只有字符串、数字和原样的 JSON")
    }
}

/// 写出去的样子。字段的顺序是图纸上的：`at`、`kind`、`turn`、`by`、`cause`、`body`。
#[derive(Serialize)]
struct LineOut<'a> {
    at: Timestamp,
    kind: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn: Option<TurnId>,
    by: &'a By,
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<&'a CommandId>,
    body: &'a TransientBody,
}

impl Serialize for Transient {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        LineOut {
            at: self.at,
            kind: self.body.kind(),
            turn: self.turn,
            by: &self.by,
            cause: self.cause.as_ref(),
            body: &self.body,
        }
        .serialize(s)
    }
}

impl Serialize for TransientBody {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            TransientBody::ModelDelta(delta) => delta.serialize(s),
        }
    }
}

/// `model.delta` 的 `body` 写出去的样子：一块开始写 `start`（工具调用再带 `name`），
/// 一段字写 `text`，收全了写 `"end":true`。
#[derive(Serialize)]
struct DeltaOut<'a> {
    seen: Seq,
    index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    end: bool,
}

impl Serialize for ModelDelta {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = DeltaOut {
            seen: self.seen,
            index: self.index,
            start: None,
            name: None,
            text: None,
            end: false,
        };
        match &self.piece {
            Piece::Start(Kind::Text) => out.start = Some("text"),
            Piece::Start(Kind::Reasoning) => out.start = Some("reasoning"),
            Piece::Start(Kind::ToolCall { name }) => {
                out.start = Some("tool_call");
                out.name = Some(name);
            }
            Piece::Text(text) => out.text = Some(text),
            Piece::End => out.end = true,
        }
        out.serialize(s)
    }
}

#[cfg(test)]
mod tests;
