//! 发话的端口（`docs/blueprint/cross-session.md` 第三条，`docs/blueprint/agents.md` 第六条，`tools/interface.md`，施工
//! 7-7、C-5）：`send_message` 这件工具只拿它，不认识会话表。执行器照这一次调用抄好这个会话的父会话、它派出去的子代理，
//! 把话作为这个会话发来的话送过去，对方不是子代理、父会话的，照会话编号认（`cross-session.md` 第三条第 1 款）。
//!
//! 施工 C-5 从 `message_agent` 改名：发得到父子之外的别的会话，回应多一种（[`Delivered::Held`]：对方是没人看着的
//! 一次性会话，只记下，不开轮），拒绝也多几种（防刷屏的三款，只在发给别的会话时才会碰到，`cross-session.md` 第五条
//! 第 7 款：父子之间的留言不受它们管）。
//!
//! 端口由下层定义、上层装（`00-设计理念.md` 第四节「依赖与接口的规矩」）：这里只定义形状，执行器（`miyu-session`）照
//! 每一次调用造一个，交给 [`crate::Call::messages`]。测试里的假调用没有，`send_message` 照「送不到」出错。

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use miyu_kernel::id::{JobId, SessionId};

/// 这件工具的名字（`agents.md` 第一条第 6 条）：场所会话里不给它。施工 C-5 从 `message_agent` 改名（照 Claude Code 的
/// `SendMessage`，2026-10-01 项目主人定）。
pub const SEND_MESSAGE: &str = "send_message";

/// 它以前的名字（施工 C-5）：以前造的会话快照里冻着它，前缀不能变，她照旧这样调；工具目录照它也找得到这一件
/// （[`crate::Tool::formerly`]），新造的会话工具面上没有它。
pub const SEND_MESSAGE_FORMERLY: &str = "message_agent";

/// 留言发给谁：父会话、这个会话派的子代理，或者（施工 C-5）别的会话——整个编号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recipient {
    /// 这个会话的父会话。
    Parent,
    /// 这个会话派的子代理：它的任务编号。
    Child(JobId),
    /// 不在这棵树上的别的会话：整个编号（施工 C-5，`cross-session.md` 第三条第 1 款，`find_session` 认出来的）。
    Session(SessionId),
}

/// 给父会话、自己派的子代理、别的会话留言。
pub trait MessagePort: Send + Sync {
    /// 把 `message` 作为这个会话发来的话送给 `to`：对方落了盘才交回，不等它回答。
    fn send<'a>(&'a self, to: Recipient, message: &'a str) -> Sending<'a>;
}

/// 送一句留言的 future。
pub type Sending<'a> = Pin<Box<dyn Future<Output = Result<Delivered, NotSent>> + Send + 'a>>;

/// 送到了：是不是落进了一个没人看着的一次性会话（施工 C-5，`cross-session.md` 第三条第 4 款）。两种都不算出错。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivered {
    /// 对方开得了轮的，正常送到。
    Sent,
    /// 对方这时是没人看着的一次性会话（`oneshot`，没有头订阅着）：话记下了，不开轮，等有人接着说时一起看到。
    Held,
}

/// 没送出去：为什么，她看得懂的那几种分开说（`agents.md` 第六条第 1 条），送不到的原因执行器记进运行日志。
/// 防刷屏的三种（施工 C-5）只在发给别的会话时才会碰到：父子之间的留言不受它们管（`cross-session.md` 第五条第 7 款）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotSent {
    /// 发给父会话，这个会话却没有父：它是主会话。
    NoParent,
    /// 不是这个会话派的子代理：没派过、派的是后台命令、派它的那一轮撤掉了。兄弟、孙代理、别人的子代理都是这一种。
    NotYours,
    /// 是它派的子代理，已经被停掉了：不再收留言。
    Stopped,
    /// 送不到：对方拒收、对方的会话停了、核心正在停。
    Undelivered,
    /// 同一个发话方在窗口里到了限速的上限（`cross-session.md` 第五条第 1 款）。
    TooMany,
    /// 同一个发话方在窗口里发过一字不差的一句（第五条第 2 款）：不算出错，那句话已经在那边了。
    Duplicate,
    /// 对方还没听到的别的会话的话到了上限（第五条第 3 款）。
    InboxFull,
}

impl fmt::Display for NotSent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            NotSent::NoParent => "this session has no parent",
            NotSent::NotYours => "not a subagent of this session",
            NotSent::Stopped => "the subagent was stopped",
            NotSent::Undelivered => "the message was not delivered",
            NotSent::TooMany => "too many messages sent recently",
            NotSent::Duplicate => "an identical message was already sent recently",
            NotSent::InboxFull => "the recipient has too many unread messages",
        })
    }
}

impl std::error::Error for NotSent {}

/// 端口不打出里面的东西。
impl fmt::Debug for dyn MessagePort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MessagePort")
    }
}

/// 两个端口比的是不是同一个：[`crate::Call`] 照格子比较时用。
impl PartialEq for dyn MessagePort {
    fn eq(&self, other: &dyn MessagePort) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl Eq for dyn MessagePort {}
