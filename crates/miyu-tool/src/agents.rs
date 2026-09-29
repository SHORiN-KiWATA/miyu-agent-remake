//! 派子代理的端口（`docs/blueprint/agents.md` 第一条，`tools/interface.md`，施工 7-5）：`agent` 这件工具只拿它，不认识
//! 会话表。执行器照父会话抄好属主、场所、工作目录、权限、能不能确认，领一个任务编号，造子会话、把交代送进去，交回编号和
//! 子会话的编号。
//!
//! 端口由下层定义、上层装（`00-设计理念.md` 第四节「依赖与接口的规矩」）：这里只定义形状，执行器（`miyu-session`）照
//! 每一次调用造一个，交给 [`crate::Call::agents`]。测试里的假调用没有，`agent` 照「派不了」出错。

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use miyu_kernel::id::{JobId, SessionId};

/// 派子代理的那件工具的名字（`agents.md` 第一条第 5、6 条）：场所会话、到了深度上限的会话，造会话时从工具面上拿掉它。
pub const AGENT: &str = "agent";

/// 派子代理：交标题和整段交代，拿回任务编号和子会话的编号。
pub trait AgentPort: Send + Sync {
    /// 派一个子代理：`description` 是短标题，`prompt` 是整段交代，原样送进子会话。子会话造好、交代送进去（它的第一轮
    /// 开了）才交回，不等它做完。
    fn spawn<'a>(&'a self, description: &'a str, prompt: &'a str) -> Spawning<'a>;
}

/// 派出去一个子代理的 future。
pub type Spawning<'a> = Pin<Box<dyn Future<Output = Result<Spawned, NotSpawned>> + Send + 'a>>;

/// 派出去了：任务编号，子会话的编号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spawned {
    /// 这个会话里的任务编号，后台命令和子代理共用一串（`kernel/ids.md`）。
    pub job: JobId,
    /// 子会话的编号。
    pub session: SessionId,
}

/// 派不了：子会话造不成、交代送不进去，或者核心正在停。原因执行器已经记进运行日志，给她看的只说派不了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotSpawned;

impl fmt::Display for NotSpawned {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the subagent was not started")
    }
}

impl std::error::Error for NotSpawned {}

/// 端口不打出里面的东西。
impl fmt::Debug for dyn AgentPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AgentPort")
    }
}

/// 两个端口比的是不是同一个：[`crate::Call`] 照格子比较时用。
impl PartialEq for dyn AgentPort {
    fn eq(&self, other: &dyn AgentPort) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl Eq for dyn AgentPort {}
