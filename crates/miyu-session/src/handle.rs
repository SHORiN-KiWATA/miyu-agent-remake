//! 拿着一个会话：发命令、订阅、有计划地停下（施工 3-7 中）。拿着它的都放下了，actor 就退出。

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::{broadcast, mpsc, oneshot};

use miyu_kernel::event::{Event, Transient};
use miyu_kernel::id::{AccountId, CommandId, JobId, SessionId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, ContextLimits, Outcome, Reason};
use miyu_kernel::time::Timestamp;
use miyu_store::recall::Room;
use miyu_tool::{JobError, Log, Output};

use crate::backlog::Backlog;

mod subscription;

use crate::jobs::Unreadable;
use crate::shown::{Next, Shown};
use subscription::Watching;
pub use subscription::{Ended, Subscription};

/// 一个会话：它的 actor 的收件箱。可以复制，几个头一起拿着。
#[derive(Debug, Clone)]
pub struct Handle {
    id: SessionId,
    /// 场所（施工 O-3）：造好以后不变。
    venue: VenueId,
    /// 会话的属主（施工 O-4 上）：日志、blob、回收处在它的家目录下。造好以后不变。
    owner: AccountId,
    inbox: mpsc::UnboundedSender<Message>,
    /// 有没有在跑的回合：actor 每送完一批输入就写一次（施工 3-9 上）。
    busy: Arc<AtomicBool>,
    /// 一次性的会话：`miyu ask` 开的（`session.created` 的 `oneshot`）。造好以后不变（施工 C-5，`send_message` 照它和
    /// [`Handle::watched`] 决定说 `sent` 还是 `held`）。
    oneshot: bool,
    /// 拿着订阅的头有没有至少一个：actor 每多了、少了一个订阅就写一次（施工 7-9，施工 C-5 从 `busy` 的做法照抄）。
    watched: Arc<AtomicBool>,
    /// 给头看的限额和会话接下来请求的模型：造会话、载入时交完限额向内核要的（施工 6-3 补）。和 actor 共用：钉住的池出错换了
    /// 成员（施工 8-9）、回合开始重新解析（施工 8-10），actor 写一次。
    shown: Arc<Mutex<Shown>>,
    /// 这个会话的记忆放在哪一间（施工 R-3 补）：协议、`/remember` 照它找；范围 `off` 的、核心没交记忆的没有。造好以后不变。
    memory: Option<Room>,
}

/// 发给 actor 的。
#[derive(Debug)]
pub(crate) enum Message {
    /// 一个命令，和等它回应的那一头。
    Command {
        id: CommandId,
        by: By,
        command: Command,
        reply: oneshot::Sender<Outcome>,
    },
    /// 要订阅：从这一刻起的推送都交给它。一个订阅算一个在看着的头（施工 7-9）。
    Subscribe(oneshot::Sender<Taken>),
    /// 要这一刻「当前的」几样（施工 9-6 上）：已经订阅着的头再订阅一次时，不另起订阅。
    Current(oneshot::Sender<crate::current::Current>),
    /// 放下了一个订阅（施工 7-9）：订阅被丢掉时由它自己送来，连同要订阅、没等到回答就不等了的。
    Unsubscribed,
    /// 有计划地停下：它的事件都落了盘，actor 退出以前交回一声。
    Stop(oneshot::Sender<()>),
    /// 删会话之前停下（施工 3-8 三补）：`force` 是假的，内核说删不了就交回原因、照常跑；删得了、或者 `force`，后台命令
    /// 整组杀掉、不记，放开日志，交回一声再退出。
    Delete {
        force: bool,
        reply: oneshot::Sender<Result<(), Reason>>,
    },
    /// 停掉派出去的任务（施工 7-4）。
    Halt(Halt),
    /// 头读一条后台命令的输出（施工 7-4 补）：不进内核、不写盘。
    Output {
        job: JobId,
        reply: oneshot::Sender<Result<Output, Unreadable>>,
    },
    /// 现在就整理记忆（施工 R-7 补）：先抽这个会话还没抽的，再合，交回几样数。
    Dream(oneshot::Sender<Result<crate::memory::Dreamed, crate::memory::NotDreamed>>),
    /// 会话 `watcher` 等这个会话空下来（施工 C-6）：记进名单，不进内核、不写盘；`since` 是那一边这次订的起算时刻。
    Watch {
        watcher: SessionId,
        since: Timestamp,
    },
}

/// 要订阅时 actor 在同一步里交回的（施工 3-8 六补）：从这一刻起的推送，这一刻落了盘的最后一条（一条都没有是 0），日志的
/// 只读入口。补发的那一截照后两样读（[`Backlog`]）。
#[derive(Debug)]
pub(crate) struct Taken {
    pub(crate) pushes: broadcast::Receiver<Arc<Pushed>>,
    pub(crate) upto: u64,
    pub(crate) log: Log,
    /// 这一刻「当前的」几样（施工 9-6 上）。
    pub(crate) current: crate::current::Current,
}

/// 停掉派出去的任务（施工 7-4，`docs/blueprint/session/actor.md`「停掉任务」）。
#[derive(Debug)]
pub(crate) enum Halt {
    /// 人用 `job.stop` 停一个：`by` 是人，`cause` 是那条命令。停好了、回报落了盘才回；没有、已经结束了的回
    /// [`JobError`]。
    One {
        job: JobId,
        by: By,
        cause: CommandId,
        reply: oneshot::Sender<Result<(), JobError>>,
    },
    /// 这个会话被父会话停下：还在跑的全停，连它们派的，都只记下、不叫醒（带 `by_model`）。后台命令那几条 `by`、`cause`
    /// 照给的。都停好了才回。
    All {
        by: By,
        cause: CommandId,
        reply: oneshot::Sender<()>,
    },
    /// 人删了这个会话派的子代理 `job`，删的那一头已经把它停下了（施工 7-8）：照人停它记一条回报，当场交进内核，落了盘才回；
    /// 不是还在跑的子代理的回 [`JobError`]。
    Deleted {
        job: JobId,
        reply: oneshot::Sender<Result<(), JobError>>,
    },
}

/// 会话是哪一个、在哪个场所、谁的（施工 O-4 上）：造把手时一起交，造好以后都不变。
#[derive(Debug, Clone)]
pub(crate) struct Ids {
    pub(crate) id: SessionId,
    pub(crate) venue: VenueId,
    pub(crate) owner: AccountId,
}

impl Handle {
    pub(crate) fn new(
        ids: Ids,
        inbox: mpsc::UnboundedSender<Message>,
        busy: Arc<AtomicBool>,
        oneshot: bool,
        watched: Arc<AtomicBool>,
        shown: Arc<Mutex<Shown>>,
    ) -> Handle {
        let Ids { id, venue, owner } = ids;
        Handle {
            id,
            venue,
            owner,
            inbox,
            busy,
            oneshot,
            watched,
            shown,
            memory: None,
        }
    }

    /// 带上这个会话的记忆的那一间（施工 R-3 补）。
    #[must_use]
    pub(crate) fn with_memory(mut self, memory: Option<Room>) -> Handle {
        self.memory = memory;
        self
    }

    /// 这个会话的记忆放在哪一间（施工 R-3 补，`memory.md`「协议」）：范围 `off` 的没有。
    pub fn memory_room(&self) -> Option<&Room> {
        self.memory.as_ref()
    }

    /// 有没有在跑的回合：核心看它决定能不能空闲退出（施工 3-9 上）。会话停了的，不算在跑。
    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }

    /// 一次性的会话：`miyu ask` 开的（施工 C-5，`cross-session.md` 第三条第 4 款）。
    pub fn oneshot(&self) -> bool {
        self.oneshot
    }

    /// 这时有没有至少一个头订阅着（施工 C-5）：造会话、载入以后是假的，和内核一样当没人看着。
    pub fn watched(&self) -> bool {
        self.watched.load(Ordering::Acquire)
    }

    /// 会话编号。
    pub fn id(&self) -> &SessionId {
        &self.id
    }

    /// 会话的属主（施工 O-4 上）：日志、blob、回收处在 `home/<它>/` 下。
    pub fn owner(&self) -> &AccountId {
        &self.owner
    }

    /// 会话的场所：`session.created` 的 `venue`（施工 O-3）。本机的是 `local`，通讯平台的场所会话只收代表外部的人说的话。
    pub fn venue(&self) -> &VenueId {
        &self.venue
    }

    /// 给头看的限额：窗口、压缩线（施工 6-3 补）。协议照它回 `subscribe`（`docs/blueprint/protocol.md`）。会话中途变了的
    /// 是变了以后的（施工 8-9、8-10）。
    pub fn limits(&self) -> ContextLimits {
        self.shown
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .limits
    }

    /// 当前的待办（施工 D-3）：协议照它写 `subscribe` 回应的 `todos`。内核推了 `todos.changed` 的是推了以后的。
    pub fn todos(&self) -> Vec<miyu_kernel::event::Todo> {
        self.shown
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .todos
            .clone()
    }

    /// 会话接下来请求的模型（施工 8-10）：引用、接下来发给谁。协议照它写 `subscribe` 回应的 `model`。回合开始重新解析过、
    /// 出错换了成员的是换了以后的。
    pub fn next(&self) -> Next {
        self.shown
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .next
            .clone()
    }

    /// 发一个命令，等它的回应：接受的，它产生的事件落了盘才回（`07-存储.md` S4）；拒绝的当场回。
    /// 编号 `id` 由发命令的一方生成，同一个编号只生效一次（`02-内核.md` 不变量 9）。
    ///
    /// # Errors
    ///
    /// 会话停了：写不进去、出了 bug、有计划地停下了。
    pub async fn command(
        &self,
        id: CommandId,
        by: By,
        command: Command,
    ) -> Result<Outcome, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Command {
            id,
            by,
            command,
            reply,
        })?;
        answer.await.map_err(|_| Stopped)
    }

    /// 这一刻「当前的」几样（施工 9-6 上）：已经订阅着、再订阅一次（协议上「还是那一个」）的，回应照它答。和那个订阅不在
    /// 同一步：头已经收着推送，这一份可能比它收到的新一点。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub async fn current(&self) -> Result<crate::current::Current, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Current(reply))?;
        answer.await.map_err(|_| Stopped)
    }

    /// 订阅：从这一刻起，落了盘的事件和瞬时事件照先后交过来。在发命令之前订阅的，这个命令产生的
    /// 事件一定先于它的回应到（`04-核心协议.md` 第六节第 2 条）。
    ///
    /// 拿着订阅就算一个头在看着这个会话：没人看着的一次性会话，回报只记下、不叫醒她（`agents.md` 第三条第 3 条，施工
    /// 7-9）。订阅放下了（丢掉它、连接断了），自己告诉 actor。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub async fn subscribe(&self) -> Result<Subscription, Stopped> {
        self.take().await.map(|(subscription, ..)| subscription)
    }

    /// 订阅，连同补发之前的事件（施工 3-8 六补，协议的 `subscribe` 带 `after`）：交回的订阅从这一刻起推，[`Backlog`] 是日志里
    /// 序号大于 `after`、这一刻落了盘的那一截，两样在 actor 的同一步里拿，接得上、不重不漏。读那一截由拿着的一方去读，不占
    /// actor。别的同 [`Handle::subscribe`]。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub async fn subscribe_after(&self, after: u64) -> Result<(Subscription, Backlog), Stopped> {
        let (subscription, upto, log) = self.take().await?;
        Ok((subscription, Backlog::new(log, after, upto)))
    }

    /// 向 actor 要一个订阅，连同它在同一步里交回的：落了盘的最后一条、日志的只读入口。
    async fn take(&self) -> Result<(Subscription, u64, Log), Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Subscribe(reply))?;
        // 送进去了才算数：等回答的时候不等了（这个 future 被丢掉），它照样放下、告诉 actor，一来一去对得上。
        let watching = Watching(self.inbox.downgrade());
        let Taken {
            pushes,
            upto,
            log,
            current,
        } = answer.await.map_err(|_| Stopped)?;
        let subscription = Subscription::new(pushes).watched(watching, current);
        Ok((subscription, upto, log))
    }

    /// 有计划地停下：送进「要重启了」，等它产生的事件落了盘，actor 退出（`02-内核.md` 第六节
    /// 「载入、崩溃、重启」第 3 条）。再载入时，被打断的那一轮接着干。
    ///
    /// # Errors
    ///
    /// 会话已经停了。
    pub async fn stop(&self) -> Result<(), Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Stop(reply))?;
        answer.await.map_err(|_| Stopped)
    }

    /// 删会话之前停下（施工 3-8 三补，`docs/blueprint/session/actor.md` 第 9 条）：有回合在进行、正在读回日志、改回文件的
    /// 删不了，交回内核说的原因，会话照常；删得了的，这个会话在跑的后台命令整组杀掉、不记回报，什么都不再写，放开日志的
    /// 文件，actor 退出。交回的时候日志已经关了：会话表接着就挪它的目录（Windows 上开着的文件挪不走）。
    ///
    /// # Errors
    ///
    /// 会话已经停了。
    pub async fn delete(&self) -> Result<Result<(), Reason>, Stopped> {
        self.stop_for_deletion(false).await
    }

    /// 同 [`Handle::delete`]，只是不问删不删得了：父会话被删，派出去的子会话不管在不在忙，一起停下（`agents.md` 第七条
    /// 第 5 条）。在路上的请求叫停、在跑的工具掐掉，跑到一半的回合不收尾。
    ///
    /// # Errors
    ///
    /// 会话已经停了。
    pub async fn discard(&self) -> Result<(), Stopped> {
        // 不问删不删得了，actor 就不会说删不了：里面那一层总是 `Ok`。
        self.stop_for_deletion(true).await.map(|_| ())
    }

    async fn stop_for_deletion(&self, force: bool) -> Result<Result<(), Reason>, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Delete { force, reply })?;
        answer.await.map_err(|_| Stopped)
    }

    /// 人停掉任务 `job`（施工 7-4，协议的 `job.stop`）：`by` 是停它的人，`cause` 是那条命令。后台命令整组杀掉，记
    /// `job.reported`（`stopped`）；子代理停掉它这一轮连它派的，记 `child.reported`（`stopped`）。两种都叫醒她。回报落了盘才回。
    ///
    /// # Errors
    ///
    /// 会话停了。里面那一层：这个会话没有这个任务、它已经结束了。
    pub async fn stop_job(
        &self,
        job: JobId,
        by: By,
        cause: CommandId,
    ) -> Result<Result<(), JobError>, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Halt(Halt::One {
            job,
            by,
            cause,
            reply,
        }))?;
        answer.await.map_err(|_| Stopped)
    }

    /// 头读后台命令 `job` 的输出（施工 7-4 补，协议的 `job.output`）：交回读得到的字、它还在不在跑。读的和她用 `jobs` 读的
    /// 是同一份（`docs/blueprint/session/tools.md` 第 6 条第 3 款）：结束了、存成 blob 的读 blob，别的读会话目录下的输出
    /// 文件，跑着的读到这时的。不进内核、不写盘：是什么照名册当场看，开文件另起一个任务，不占 actor。
    ///
    /// # Errors
    ///
    /// 会话停了。里面那一层：这个会话没派过这个任务（[`Unreadable::Unknown`]），或者它是子代理（[`Unreadable::Agent`]）。
    pub async fn job_output(&self, job: JobId) -> Result<Result<Output, Unreadable>, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Output { job, reply })?;
        answer.await.map_err(|_| Stopped)
    }

    /// 停掉这个会话派出去、还没结束的全部任务，连它们派的（施工 7-4）：父会话停下它的时候，会话表经端口来调。都只记下、
    /// 不叫醒它；后台命令那几条的 `by`、`cause` 照给的。都停好了才回。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub async fn stop_jobs(&self, by: By, cause: CommandId) -> Result<(), Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Halt(Halt::All { by, cause, reply }))?;
        answer.await.map_err(|_| Stopped)
    }

    /// 人删了这个会话派的子代理 `job`，删的那一头（会话表，拿着表的锁）已经把它停下了（施工 7-8，`agents.md` 第七条第 6 条：
    /// 删一个子会话本身等于人先停掉它再删）：照人用 `job.stop` 停它的样子记一条 `child.reported`（`stopped`，`by` 是子会话，
    /// 不带 `by_model`，叫醒她），正文照它的日志看。回报当场交进这个会话，不经会话表：表的锁在删的那一头手里。落了盘才回。
    ///
    /// # Errors
    ///
    /// 会话停了。里面那一层：这个会话没派过这个子代理、它已经结束了（报过、被停过），或者没有会话表的端口。
    pub async fn stopped_child(&self, job: JobId) -> Result<Result<(), JobError>, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Halt(Halt::Deleted { job, reply }))?;
        answer.await.map_err(|_| Stopped)
    }

    /// 现在就整理这个会话的记忆（施工 R-7 补，`/dream`、带 `session` 的 `memory.dream`，`memory.md` 第七条第 9 款）：先把这个
    /// 会话还没抽的那一段抽了（不等闲、答了一轮就算；正忙的、正在抽的不抽），再不看会话数、间隔现在就合，交回几样数。
    ///
    /// # Errors
    ///
    /// 会话停了。里面那一层：这个会话的记忆关着、人格记忆没装（`Off`），这一间正在合（`Busy`），没合成（`Failed`）。
    pub async fn dream(
        &self,
    ) -> Result<Result<crate::memory::Dreamed, crate::memory::NotDreamed>, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Dream(reply))?;
        answer.await.map_err(|_| Stopped)
    }

    /// 会话 `watcher` 等这个会话空下来（施工 C-6，`cross-session.md` 第六条第 3 款，2026-10-01 改）：记进它 actor 的
    /// 名单，同一个会话只记一个（后订的替掉先订的）；这时正忙着、或者 `since` 不晚于它上一次忙完的时刻才当场发通知，
    /// 不然等它下一次忙完。`since` 是那一边这次订的起算时刻。交进收件箱就回，不等。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub fn watch(&self, watcher: SessionId, since: Timestamp) -> Result<(), Stopped> {
        self.send(Message::Watch { watcher, since })
    }

    fn send(&self, message: Message) -> Result<(), Stopped> {
        self.inbox.send(message).map_err(|_| Stopped)
    }
}

/// 推给订阅者的一份。
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::large_enum_variant,
    reason = "一份份装在 Arc 里共享（施工 O-3 外部身份多了两格，瞬时事件变大）：大小不碍事"
)]
pub enum Pushed {
    /// 落了盘的几条事件，照先后。
    Events(Vec<Event>),
    /// 一条瞬时事件：不落盘（`03-事件模型.md` 第五节）。
    Transient(Transient),
}

/// 会话停了：写不进去、出了 bug、有计划地停下了，或者没人拿着了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stopped;

impl fmt::Display for Stopped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the session stopped")
    }
}

impl std::error::Error for Stopped {}

#[cfg(test)]
mod tests;
