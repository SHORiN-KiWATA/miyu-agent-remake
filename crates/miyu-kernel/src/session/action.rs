//! 会话要做的事，和命令的结局（`docs/designs/02-内核.md` 第四节「输入、动作、命令怎么写」）。
//!
//! 会话自己不做 I/O：要追加的事件、要回应的命令、要推送的事件，都写成动作交给执行器。

use crate::event::{Event, Transient};
use crate::id::{CommandId, Seq, TurnId};
use crate::request::Request;

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
    /// 跑回合开始的挂接点：叫各模块，等它们都回来或者超时，把各自的注入照固定的先后交回来
    /// （`05-内核接口.md` 第五节第 2 条）。一个模块都没挂，也照样回一次，交回空的。
    RunTurnStartHooks {
        /// 哪个回合。
        turn: TurnId,
    },
    /// 请求模型：把这份请求交给驱动编码、发出去（`05-内核接口.md` 第七节）。发出去了、
    /// 每一段增量、说完了，都带着 `seen` 回报（`02-内核.md` 第六节「回复怎么收、回合怎么结束」）。
    CallModel {
        /// 这次请求看到了第几条为止，也是这次请求的名字。
        seen: Seq,
        /// 统一的请求。
        request: Request,
    },
    /// 把一条瞬时事件推给头：不落盘，不等（`03-事件模型.md` 第五节）。
    PushTransient(Transient),
    /// 不要这次请求了：停下来，别再发它的增量。之后还来的回报，都当过时的不理。
    CancelModel {
        /// 哪一次请求。
        seen: Seq,
    },
    /// 跑回合结束的挂接点：广播给各模块，不等结果（`05-内核接口.md` 第五节）。
    RunTurnEndHooks {
        /// 哪个回合。
        turn: TurnId,
    },
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
