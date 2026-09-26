//! 会话要做的事，和命令的结局（`docs/designs/02-内核.md` 第四节「输入、动作、命令怎么写」）。
//!
//! 会话自己不做 I/O：要追加的事件、要回应的命令、要推送的事件，都写成动作交给执行器。

use crate::event::Event;
use crate::id::{CommandId, Seq};

/// 会话要执行器做的一件事。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// 追加这几条事件：一次写入、一次同步（`07-存储.md` 第三节）。落了盘，执行器送一条
    /// 「落盘了」回来。
    Append(Vec<Event>),
    /// 回应一个命令：接受的等它的事件落了盘才回，拒绝的当场回。
    Reply {
        /// 回应的是哪个命令。
        id: CommandId,
        /// 结局。
        outcome: Outcome,
    },
    /// 把这几条落了盘的事件推给头（S4：先落盘，后推送）。
    Push(Vec<Event>),
}

/// 一个命令的结局：被接受并产生事件，或被拒绝并附原因（不变量 7）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// 接受了。
    Accepted {
        /// 它产生的事件的序号，照先后。
        events: Vec<Seq>,
    },
    /// 拒绝了，什么都没产生。
    Rejected {
        /// 为什么拒绝。
        reason: Reason,
    },
}

/// 拒绝的原因。给程序看的原因码是稳定的英文（[`Reason::code`]）；给人看的话，由核心照头的
/// 语言配上（`04-核心协议.md` 第六节第 4 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// 发来的消息一块内容都没有。
    EmptyMessage,
}

impl Reason {
    /// 原因码。
    pub fn code(self) -> &'static str {
        match self {
            Reason::EmptyMessage => "empty_message",
        }
    }
}
