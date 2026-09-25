//! 事件：已经发生的事实，追加进会话的日志（`docs/designs/03-事件模型.md`）。
//!
//! 一条事件在日志里是一行紧凑的 JSON，外壳的写法见第二节「外壳的写法」。
//! 认识的种类读成对应的类型；不认识的，`body` 原样留着。

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::id::{CommandId, EventKind, Seq, TurnId};
use crate::origin::By;
use crate::raw::{self, RawJson};
use crate::time::Timestamp;

mod message;

pub use message::MessageUser;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub seq: Seq,
    pub at: Timestamp,
    /// 所属回合；不属于任何回合时没有。
    pub turn: Option<TurnId>,
    pub by: By,
    /// 引起它的命令，用于去重和追踪。
    pub cause: Option<CommandId>,
    pub body: Body,
}

/// 事件的种类，连同它自己的内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    MessageUser(MessageUser),
    /// 不认识的种类，包括不认识的 `ext.*`：`body` 原样留着，投影跳过它。
    Unknown {
        kind: EventKind,
        body: RawJson,
    },
}

impl Body {
    /// 外壳里 `kind` 那一格写的名字。
    pub fn kind(&self) -> &str {
        match self {
            Body::MessageUser(_) => "message.user",
            Body::Unknown { kind, .. } => kind.as_str(),
        }
    }

    /// 按种类读 `body`。认识的种类读不出来是坏数据，报错写明是哪一种。
    fn read(kind: EventKind, body: RawJson) -> Result<Body, String> {
        let known = |read: serde_json::Result<Body>| {
            read.map_err(|e| format!("{kind} 的 body 读不出来：{e}"))
        };
        match kind.as_str() {
            "message.user" => known(raw::parse(body.get()).map(Body::MessageUser)),
            _ => Ok(Body::Unknown { kind, body }),
        }
    }
}

impl Event {
    /// 写成日志里的一行：紧凑的 JSON，不带换行。
    pub fn to_line(&self) -> String {
        serde_json::to_string(self)
            .expect("事件里只有字符串、数字和原样的 JSON，写成 JSON 不会失败")
    }

    /// 从日志里的一行读回来。
    pub fn from_line(line: &str) -> Result<Event, serde_json::Error> {
        serde_json::from_str(line)
    }
}

/// 写出去的样子。字段的顺序就是图纸上的顺序。
#[derive(Serialize)]
struct LineOut<'a> {
    seq: Seq,
    at: Timestamp,
    kind: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn: Option<TurnId>,
    by: &'a By,
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<&'a CommandId>,
    body: &'a Body,
}

/// 读进来的样子：`body` 先原样读下来，看过 `kind` 再照那一种读。
/// 可选字段没有、写成 `null`，都当没有；不认识的字段不管。
#[derive(Deserialize)]
struct LineIn {
    seq: Seq,
    at: Timestamp,
    kind: EventKind,
    #[serde(default)]
    turn: Option<TurnId>,
    by: By,
    #[serde(default)]
    cause: Option<CommandId>,
    body: RawJson,
}

impl Serialize for Event {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        LineOut {
            seq: self.seq,
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

/// `body` 只写它自己的内容；种类写在外壳的 `kind` 里。
impl Serialize for Body {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Body::MessageUser(body) => body.serialize(s),
            Body::Unknown { body, .. } => body.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for Event {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let line = LineIn::deserialize(d)?;
        Ok(Event {
            seq: line.seq,
            at: line.at,
            turn: line.turn,
            by: line.by,
            cause: line.cause,
            body: Body::read(line.kind, line.body).map_err(D::Error::custom)?,
        })
    }
}

#[cfg(test)]
mod tests;
