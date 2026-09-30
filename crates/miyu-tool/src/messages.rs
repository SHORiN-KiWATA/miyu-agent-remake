//! 父子之间留言的端口（`docs/blueprint/agents.md` 第六条，`tools/interface.md`，施工 7-7）：`message_agent` 这件工具只拿它，
//! 不认识会话表。执行器照这一次调用抄好这个会话的父会话、它派出去的子代理，把留言作为这个会话发来的话送过去。
//!
//! 端口由下层定义、上层装（`00-设计理念.md` 第四节「依赖与接口的规矩」）：这里只定义形状，执行器（`miyu-session`）照
//! 每一次调用造一个，交给 [`crate::Call::messages`]。测试里的假调用没有，`message_agent` 照「送不到」出错。

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use miyu_kernel::id::JobId;

/// 父子之间留言的那件工具的名字（`agents.md` 第一条第 6 条）：场所会话里不给它。
pub const MESSAGE_AGENT: &str = "message_agent";

/// 留言发给谁：只在树上相邻的两层之间（2026-09-29 项目主人定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recipient {
    /// 这个会话的父会话。
    Parent,
    /// 这个会话派的子代理：它的任务编号。
    Child(JobId),
}

/// 给父会话、自己派的子代理留言。
pub trait MessagePort: Send + Sync {
    /// 把 `message` 作为这个会话发来的话送给 `to`：对方落了盘才交回，不等它回答。
    fn send<'a>(&'a self, to: Recipient, message: &'a str) -> Sending<'a>;
}

/// 送一句留言的 future。
pub type Sending<'a> = Pin<Box<dyn Future<Output = Result<(), NotSent>> + Send + 'a>>;

/// 没送出去：为什么，她看得懂的那几种分开说（`agents.md` 第六条第 1 条），送不到的原因执行器记进运行日志。
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
}

impl fmt::Display for NotSent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            NotSent::NoParent => "this session has no parent",
            NotSent::NotYours => "not a subagent of this session",
            NotSent::Stopped => "the subagent was stopped",
            NotSent::Undelivered => "the message was not delivered",
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
