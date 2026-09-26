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

mod context;
mod message;
mod model;
mod question;
mod session;
mod tool;
mod transient;
mod turn;

pub use context::{ContextCompacted, ContextInjected};
pub use message::{MessageAssistant, MessageUser, MessageWithdrawn};
pub use model::{
    CallError, CallResult, ErrorClass, FirstDifference, MessageRole, ModelCalled, Part, Usage,
};
pub use question::{Choice, Question, QuestionAnswered, QuestionAsked, Response, fits};
pub use session::{Level, MetaChanged, Permission, PolicyChanged, SessionCreated};
pub use tool::{ApprovalDecided, ApprovalRequested, Decision, ToolResult, ToolStatus};
pub use transient::{ModelDelta, Piece, ToolProgress, Transient, TransientBody};
pub use turn::{EndReason, TurnEnded, TurnReverted, TurnStarted, TurnUnreverted};

/// 一条事件：已经发生的一件事。追加进日志以后不改、不删；撤销和压缩也是追加一条新事件
/// （`03-事件模型.md` 第一节）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// 会话内的序号，从 1 开始，连续递增。
    pub seq: Seq,
    /// 发生的时刻，取自执行器送进来的时钟输入。
    pub at: Timestamp,
    /// 所属回合；不属于任何回合时没有。
    pub turn: Option<TurnId>,
    /// 由谁引起，取自连接，不取自正文。
    pub by: By,
    /// 引起它的命令，用于去重和追踪。
    pub cause: Option<CommandId>,
    /// 事件的种类，连同它自己的内容。
    pub body: Body,
}

/// 事件的种类：每一种写一行，类型和它在 JSON 里的名字。
/// `Body` 本身、`kind`、按种类读、写出去，都照这一张表生成，加一种只加一行。
macro_rules! bodies {
    ($($(#[$doc:meta])* $variant:ident = $kind:literal,)+) => {
        /// 事件的种类，连同它自己的内容。
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum Body {
            $($(#[$doc])* $variant($variant),)+
            /// 不认识的种类，包括不认识的 `ext.*`：`body` 原样留着，投影跳过它。
            Unknown {
                /// 外壳里写的种类名。
                kind: EventKind,
                /// 原样的 `body`，写出去一字不差。
                body: RawJson,
            },
        }

        impl Body {
            /// 内核认识的全部种类名，照表里的先后。样本测试拿它查每一种都有样本
            /// （03 第三节「样本文件」）。
            pub const KINDS: &'static [&'static str] = &[$($kind,)+];

            /// 外壳里 `kind` 那一格写的名字。
            pub fn kind(&self) -> &str {
                match self {
                    $(Body::$variant(_) => $kind,)+
                    Body::Unknown { kind, .. } => kind.as_str(),
                }
            }

            /// 按种类读 `body`。认识的种类读不出来是坏数据，报错写明是哪一种。
            fn read(kind: EventKind, body: RawJson) -> Result<Body, String> {
                let read = match kind.as_str() {
                    $($kind => raw::parse(body.get()).map(Body::$variant),)+
                    _ => return Ok(Body::Unknown { kind, body }),
                };
                read.map_err(|e| format!("{kind} 的 body 读不出来：{e}"))
            }
        }

        /// `body` 只写它自己的内容；种类写在外壳的 `kind` 里。
        impl Serialize for Body {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                match self {
                    $(Body::$variant(body) => body.serialize(s),)+
                    Body::Unknown { body, .. } => body.serialize(s),
                }
            }
        }
    };
}

bodies! {
    /// 会话创建。
    SessionCreated = "session.created",
    /// 换了策略快照，或者换了权限。
    PolicyChanged = "session.policy_changed",
    /// 改了标题、置顶。
    MetaChanged = "session.meta_changed",
    /// 回合开始。
    TurnStarted = "turn.started",
    /// 回合结束。
    TurnEnded = "turn.ended",
    /// 撤销了几个回合。
    TurnReverted = "turn.reverted",
    /// 恢复了最近一次撤销的回合。
    TurnUnreverted = "turn.unreverted",
    /// 人发来的消息，或者另一个会话发来的消息。
    MessageUser = "message.user",
    /// 模型一次响应的完整内容，工具调用也在里面。
    MessageAssistant = "message.assistant",
    /// 撤回排着队、她还没听到的消息。
    MessageWithdrawn = "message.withdrawn",
    /// 一个工具调用的结果。
    ToolResult = "tool.result",
    /// 请人确认一次工具调用。
    ApprovalRequested = "tool.approval_requested",
    /// 人对确认请求的决定。
    ApprovalDecided = "tool.approval_decided",
    /// 一个在跑的调用请人回答一组题。
    QuestionAsked = "question.asked",
    /// 人对一组题的回答。
    QuestionAnswered = "question.answered",
    /// 注入进上下文的一块事实。
    ContextInjected = "context.injected",
    /// 压缩的检查点。
    ContextCompacted = "context.compacted",
    /// 一次模型请求的记录。
    ModelCalled = "model.called",
}

impl Event {
    /// 写成日志里的一行：紧凑的 JSON，字段照图纸的顺序，不带换行。换行由存日志的那一层加。
    ///
    /// # Panics
    ///
    /// 实际不会 panic。serde_json 只在两种情况下写不出来：键不是字符串，或者某个 `Serialize`
    /// 自己报错。事件里这两样都没有。
    pub fn to_line(&self) -> String {
        serde_json::to_string(self)
            .expect("事件里只有字符串、数字和原样的 JSON，写成 JSON 不会失败")
    }

    /// 从日志里的一行读回来。认识的种类读成对应的类型，不认识的 `body` 原样留着。
    ///
    /// # Errors
    ///
    /// 这一行不是 JSON、缺了外壳的字段、某个字段不合写法、认识的种类 `body` 读不出来，
    /// 都返回错误，写明哪里错。
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
