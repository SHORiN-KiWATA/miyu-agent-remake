//! 会话要做的事，和命令的结局（`docs/designs/02-内核.md` 第四节「输入、动作、命令怎么写」）。
//!
//! 会话自己不做 I/O：要追加的事件、要回应的命令、要推送的事件，都写成动作交给执行器。

use crate::event::{Event, Permission, Response, Transient};
use crate::id::{CallId, CommandId, Seq, TurnId};
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
    /// 停下一次在跑的工具调用：回合被打断了。之后还来的结果和输出，都当过时的不理。
    CancelTool {
        /// 哪一次调用。
        call_id: CallId,
    },
    /// 过执行前的链：权限策略和各扩展的守卫，最严者胜，交回一个结论（`02-内核.md` 第六节
    /// 「确认怎么走」）。守卫超时的按拒绝交回。
    GuardTool {
        /// 哪一次调用。
        call_id: CallId,
        /// 工具名。
        name: String,
        /// 修正过的参数：一个 JSON 对象的原文。
        args: String,
        /// 这一轮的工作目录：回合开始时的那一个。
        cwd: String,
        /// 实际生效的那一级：权限策略照它判。
        permission: Permission,
    },
    /// 把人的回答交给在等的那个调用：它问的那组题答完了，回答已经落了盘（`02-内核.md` 第六节
    /// 「提问怎么走」第 3 条）。之后照常等它执行完。
    AnswerTool {
        /// 哪一次调用。
        call_id: CallId,
        /// 照题目的先后，每道题的回答。
        answers: Vec<Response>,
    },
    /// 执行一次工具调用。执行中的输出、执行完了，都带着调用编号回报
    /// （`02-内核.md` 第六节「工具怎么调、下一步怎么走」）。
    RunTool {
        /// 哪一次调用。
        call_id: CallId,
        /// 工具名。
        name: String,
        /// 修正过的参数：一个 JSON 对象的原文。
        args: String,
        /// 这一轮的工作目录：回合开始时的那一个。
        cwd: String,
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
    /// 没有回合在进行，打断不了。
    NotRunning,
    /// 要切到的级别不认识。
    UnknownLevel,
    /// 这个调用不在等人回答：没问过、已经回答过、已经有了结果，或者等的是另一种回答（对着确认
    /// 答提问，对着提问答确认）。
    NotAsking,
    /// 确认的选项不认识。
    UnknownDecision,
    /// 请求没提放行规则，选不了本会话都允许、这个工作区以后都允许。
    NoRule,
    /// 只有拒绝能带理由。
    UnexpectedReason,
    /// 回答对不上题目：题数不对、选了没有的选项、单选的选了几项、同一项选了两次。
    BadAnswer,
    /// 有回合在进行，撤销不了：头先打断再撤（`02-内核.md` 第六节「撤销与恢复」）。
    TurnRunning,
    /// 要撤的那一轮不在有效历史里：没有这一轮，或者已经撤掉了。
    UnknownTurn,
    /// 要撤的那一轮已经压缩进摘要了：要回到那之前，从那里分叉。
    Compacted,
    /// 没有能恢复的撤销：没撤过，或者撤了以后开过回合、压缩过。
    NothingToUnrevert,
}

impl Reason {
    /// 原因码。
    pub fn code(self) -> &'static str {
        match self {
            Reason::EmptyMessage => "empty_message",
            Reason::NotRunning => "not_running",
            Reason::UnknownLevel => "unknown_level",
            Reason::NotAsking => "not_asking",
            Reason::UnknownDecision => "unknown_decision",
            Reason::NoRule => "no_rule",
            Reason::UnexpectedReason => "unexpected_reason",
            Reason::BadAnswer => "bad_answer",
            Reason::TurnRunning => "turn_running",
            Reason::UnknownTurn => "unknown_turn",
            Reason::Compacted => "compacted",
            Reason::NothingToUnrevert => "nothing_to_unrevert",
        }
    }
}
