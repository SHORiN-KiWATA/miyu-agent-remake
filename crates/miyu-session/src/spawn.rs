//! 造子会话、给别的会话发命令的端口（`docs/blueprint/agents.md`「在哪」，施工 7-5）：会话表在协议端点（`miyu-endpoint`），
//! 比会话 actor 高一层，所以端口在这里定义、由会话表造会话和载入时交进来（`00-设计理念.md` 第四节「依赖与接口的规矩」：
//! 下层定义窄接口，上层实现）。执行器派子代理时经它造子会话、把交代送进去（`crate::agents`）；子会话经它向上回报
//! （`crate::report`），父会话载入以后经它叫起还没回报的子会话（施工 7-6）。
//!
//! 测试里自己造的会话没有它：`agent` 照派不了出错。`jobs` 停子代理、读它在做什么也经它（施工 7-4）。`sessions` 列主会话也经它
//! （施工 C-3，`crate::sessions`）。`send_message` 发给别的会话、认它是不是没人看着的一次性会话也经它（施工 C-5，
//! `crate::messages`）。订「空了告诉我」也经它（施工 C-6，`crate::peers`）：会话表把「谁在等」交给被等的那个会话的 actor，
//! 被等的那一边空下来，照样经它把通知交给等的那个会话（命令 `PeerIdle`）。

use std::future::Future;
use std::pin::Pin;

use miyu_kernel::event::Permission;
use miyu_kernel::id::{AccountId, CommandId, SessionId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome};
use miyu_kernel::time::Timestamp;
use miyu_tool::{Log, MainSession, Stop};

use crate::jobs::Peek;

/// 造子会话、给别的会话发命令。原因是英文的一句，执行器记进运行日志，不给她看。
pub trait SessionPort: Send + Sync {
    /// 会话 `session` 在会话列表里的那一项变了（施工 9-5，`protocol.md`「会话列表的推送」）：造了、改名置顶、一轮开始、
    /// 空下来。只报「变了」，那一项由会话表照索引算。默认什么都不做。
    fn listing(&self, _session: SessionId) {}

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

    /// 只读地开会话 `session` 的日志（施工 C-4，`cross-session.md` 第二条第 2 款）：不载入它，在跑的也读得到。核心正在停的
    /// 交回原因；放会话目录本身不必读了才知道读不读得到，日志坏了、读不了的要等交回的 [`Log`] 读的时候才知道。`history`
    /// 只在认出 `session` 参数写的是这个属主看得到的另一个会话（不是它自己）时才调它。
    fn read_log(&self, session: SessionId) -> Pending<'_, Result<Log, String>>;

    /// 会话 `session` 这时是不是没人看着的一次性会话（施工 C-5，`cross-session.md` 第三条第 4 款）：`send_message` 送到
    /// 以后照它说 `sent.txt` 还是 `held.txt`。调它之前这个会话已经经 [`SessionPort::command`] 送过一次，这时在会话表里
    /// 一定载入着；没在表里（会话表照它核对不出）的，当不是：读不出「没人看着」，照 `sent.txt` 说。
    fn held(&self, session: SessionId) -> Pending<'_, bool>;

    /// 会话 `watcher` 在等会话 `session` 空下来（施工 C-6，`cross-session.md` 第六条第 3 款）：交给 `session` 的 actor 记进
    /// 名单，不经内核、不进它的日志；它没载入的先载入。`since` 是等的那一边这次订从哪一刻算起，通知的命令编号带着它：同一次
    /// 订再交一遍，通知还是同一个编号，那边认得出是重的。交到了就回，不等它空下来。
    ///
    /// # Errors
    ///
    /// 那个会话不在了（删了、找不到）：[`NotWatched::Gone`]，等的那一边记 `gone`。别的（它停了、核心正在停）：
    /// [`NotWatched::Failed`]，交回原因，等的那一边记一行运行日志。
    fn watch(
        &self,
        session: SessionId,
        watcher: SessionId,
        since: Timestamp,
    ) -> Pending<'_, Result<(), NotWatched>>;
}

/// 订「空了告诉我」没订上（施工 C-6）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotWatched {
    /// 那个会话不在了：删了、找不到。
    Gone,
    /// 别的原因：它停了、核心正在停。英文的一句，记进运行日志。
    Failed(String),
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
    /// 人格：派的时候挑的（施工 P-2 补）；没挑的是无人格（施工 P-4 上）。
    pub persona: Option<String>,
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
    /// 子会话用哪个模型（施工 8-8）：模型或 `@池`，记进它的 `session.created`。她写了池的是 `@<池>`（施工 8-8 补），没写的是
    /// 父会话这时生效的；都没有的是空的，子会话照它造出来那时的 `models.chat`。
    pub model: Option<String>,
    /// 子会话的预设（施工 P-2 上）：父会话的（C5 的默认）；父会话以前造的、没有预设的是空的。
    pub preset: Option<String>,
}
