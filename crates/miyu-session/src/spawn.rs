//! 造子会话、给别的会话发命令的端口（`docs/blueprint/agents.md`「在哪」，施工 7-5）：会话表在协议端点（`miyu-endpoint`），
//! 比会话 actor 高一层，所以端口在这里定义、由会话表造会话和载入时交进来（`00-设计理念.md` 第四节「依赖与接口的规矩」：
//! 下层定义窄接口，上层实现）。执行器派子代理时经它造子会话、把交代送进去（`crate::agents`）；子会话经它向上回报
//! （`crate::report`），父会话载入以后经它叫起还没回报的子会话（施工 7-6）。
//!
//! 测试里自己造的会话没有它：`agent` 照派不了出错。

use std::future::Future;
use std::pin::Pin;

use miyu_kernel::event::Permission;
use miyu_kernel::id::{AccountId, CommandId, SessionId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome};

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
