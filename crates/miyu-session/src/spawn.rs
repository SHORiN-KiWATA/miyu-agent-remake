//! 造子会话、给别的会话发命令的端口（`docs/blueprint/agents.md`「在哪」，施工 7-5）：会话表在协议端点（`miyu-endpoint`），
//! 比会话 actor 高一层，所以端口在这里定义、由会话表造会话和载入时交进来（`00-设计理念.md` 第四节「依赖与接口的规矩」：
//! 下层定义窄接口，上层实现）。执行器派子代理时经它造子会话、把交代送进去（`crate::agents`）；子会话经它向上回报
//! （`crate::report`），父会话载入以后经它叫起还没回报的子会话（施工 7-6）。
//!
//! 测试里自己造的会话没有它：`agent` 照派不了出错。`jobs` 停子代理、读它在做什么也经它（施工 7-4）。`sessions` 列主会话也经它
//! （施工 C-3，`crate::sessions`）。

use std::future::Future;
use std::pin::Pin;

use miyu_kernel::event::Permission;
use miyu_kernel::id::{AccountId, CommandId, SessionId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome};
use miyu_tool::{MainSession, Stop};

use crate::jobs::Peek;

/// 造子会话、给别的会话发命令。原因是英文的一句，执行器记进运行日志，不给她看。
pub trait SessionPort: Send + Sync {
    /// 照 `child` 造一个子会话：`session.created` 落了盘、会话表里有了它才交回编号。
    fn create(&self, child: Child) -> Pending<'_, Result<SessionId, String>>;

    /// 叫起会话 `session`（施工 7-6）：没在跑的照会话表的规矩载入，在跑的什么都不做。父会话载入以后叫起还没回报的子会话：
    /// 崩了的由它们自己补报、重启打断的接着干（`agents.md` 第八条）。
    fn open(&self, session: SessionId) -> Pending<'_, Result<(), String>>;

    /// 给会话 `session` 发一个命令，等回应：编号 `id`，谁发的 `by`。会话没在跑的，照会话表的规矩先载入。子会话向上回报也
    /// 经它交给父会话（施工 7-6，`crate::report`）。
    fn command(
        &self,
        session: SessionId,
        id: CommandId,
        by: By,
        command: Command,
    ) -> Pending<'_, Result<Outcome, String>>;

    /// 停下会话 `session`（施工 7-4，`agents.md` 第五条）：打断它在跑的一轮（排着的退回），再停掉它派出去、还没结束的，
    /// 连它们派的一起。打断记成 `by` 发的、编号 `id`。停好了才交回。会话没在跑的，照会话表的规矩先载入。
    fn stop(&self, session: SessionId, id: CommandId, by: By) -> Pending<'_, Result<(), String>>;

    /// 会话 `session` 这会儿的样子（施工 7-4）：照它的日志算，不载入它。
    fn peek(&self, session: SessionId) -> Pending<'_, Result<Peek, String>>;

    /// 属主是 `owner` 的主会话（施工 C-3，`cross-session.md` 第一条第 2 款）：`session.created` 不带 `parent`、没删的，含调的这个
    /// 会话自己，不排先后。和协议的 `session.list` 同一个函数算：每个会话只读地读一遍日志，不载入它；读下一个之前看一眼
    /// `stop`，举起来了就不往下读，交回已经读到的。放会话的目录读不了、核心正在停：交回原因。
    fn sessions(
        &self,
        owner: AccountId,
        stop: Stop,
    ) -> Pending<'_, Result<Vec<MainSession>, String>>;
}

/// 端口交回的 future。
pub type Pending<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// 子会话是谁派的、第几层（`kernel/events-bodies.md`：`session.created` 的 `parent`、`depth`）。主会话没有。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lineage {
    /// 父会话。
    pub parent: SessionId,
    /// 第几层：父会话的加一，至少是 1。
    pub depth: u32,
}

/// 造一个子会话要的（`agents.md` 第一条第 1 条）：执行器照父会话这一刻的样子填好，会话表照它造。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Child {
    /// 父会话和第几层。
    pub lineage: Lineage,
    /// 造它的命令编号：`session.created` 的 `cause`，`by` 是父会话。
    pub command: CommandId,
    /// 人格：现在总是软件工程师，挑人格随预设那一步。
    pub persona: String,
    /// 属主：父会话的属主。
    pub owner: AccountId,
    /// 场所：父会话的。
    pub venue: VenueId,
    /// 开始时的权限：父会话派它那一刻的，常用的那一级和只读开关都抄（只读开关以后跟着父会话变随 7-8）。
    pub permission: Permission,
    /// 有没有人能确认：父会话的。
    pub attended: bool,
    /// 工作目录：父会话这一轮的。
    pub cwd: String,
    /// 加进来的目录：父会话这一轮的。
    pub dirs: Vec<String>,
}
