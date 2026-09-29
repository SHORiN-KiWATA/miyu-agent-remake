//! 拿着一个会话：发命令、订阅、有计划地停下（施工 3-7 中）。拿着它的都放下了，actor 就退出。

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::{broadcast, mpsc, oneshot};

use miyu_kernel::event::{Event, Transient};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{CommandId, JobId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, ContextLimits, Outcome};
use miyu_tool::JobError;

/// 一个会话：它的 actor 的收件箱。可以复制，几个头一起拿着。
#[derive(Debug, Clone)]
pub struct Handle {
    id: SessionId,
    inbox: mpsc::UnboundedSender<Message>,
    /// 有没有在跑的回合：actor 每送完一批输入就写一次（施工 3-9 上）。
    busy: Arc<AtomicBool>,
    /// 给头看的限额：造会话、载入时交完限额向内核要的（施工 6-3 补）。会话里不变：一个核心一个模型，策略冻结在会话上；
    /// 换模型那一步再改成会变的。
    limits: ContextLimits,
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
    /// 要订阅：从这一刻起的推送都交给它。
    Subscribe(oneshot::Sender<broadcast::Receiver<Arc<Pushed>>>),
    /// 有计划地停下：它的事件都落了盘，actor 退出以前交回一声。
    Stop(oneshot::Sender<()>),
    /// 环境变了：工作目录、时区。
    Environment(Environment),
    /// 停掉派出去的任务（施工 7-4）。
    Halt(Halt),
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
}

impl Handle {
    pub(crate) fn new(
        id: SessionId,
        inbox: mpsc::UnboundedSender<Message>,
        busy: Arc<AtomicBool>,
        limits: ContextLimits,
    ) -> Handle {
        Handle {
            id,
            inbox,
            busy,
            limits,
        }
    }

    /// 有没有在跑的回合：核心看它决定能不能空闲退出（施工 3-9 上）。会话停了的，不算在跑。
    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }

    /// 会话编号。
    pub fn id(&self) -> &SessionId {
        &self.id
    }

    /// 给头看的限额：窗口、压缩线（施工 6-3 补）。协议照它回 `subscribe`（`docs/blueprint/protocol.md`）。
    pub fn limits(&self) -> ContextLimits {
        self.limits
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

    /// 订阅：从这一刻起，落了盘的事件和瞬时事件照先后交过来。在发命令之前订阅的，这个命令产生的
    /// 事件一定先于它的回应到（`04-核心协议.md` 第六节第 2 条）。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub async fn subscribe(&self) -> Result<Subscription, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Subscribe(reply))?;
        answer.await.map(Subscription::new).map_err(|_| Stopped)
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

    /// 环境变了：头报上来的工作目录换了，或者时区换了。不当场注入，到下一个边界再查
    /// （`08-上下文投影.md` C10）。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub fn environment(&self, environment: Environment) -> Result<(), Stopped> {
        self.send(Message::Environment(environment))
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

    fn send(&self, message: Message) -> Result<(), Stopped> {
        self.inbox.send(message).map_err(|_| Stopped)
    }
}

/// 推给订阅者的一份。
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// 一个订阅。
#[derive(Debug)]
pub struct Subscription {
    pushes: broadcast::Receiver<Arc<Pushed>>,
    /// 掉过队了：这个订阅作废，头重新订阅。
    lagged: bool,
}

/// 订阅断了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// 读得太慢，掉了队：中间漏了推送，这个订阅作废，要重新订阅（`04-核心协议.md` 第七节的 resync）。
    Lagged,
    /// 会话停了。
    Stopped,
}

impl Subscription {
    pub(crate) fn new(pushes: broadcast::Receiver<Arc<Pushed>>) -> Subscription {
        Subscription {
            pushes,
            lagged: false,
        }
    }

    /// 下一份推送。
    ///
    /// # Errors
    ///
    /// 掉了队，或者会话停了。掉过一次队，以后一直是 [`Ended::Lagged`]。
    pub async fn next(&mut self) -> Result<Arc<Pushed>, Ended> {
        if self.lagged {
            return Err(Ended::Lagged);
        }
        match self.pushes.recv().await {
            Ok(pushed) => Ok(pushed),
            Err(RecvError::Lagged(_)) => {
                self.lagged = true;
                Err(Ended::Lagged)
            }
            Err(RecvError::Closed) => Err(Ended::Stopped),
        }
    }

    /// 不等：已经到了的下一份；还没到的，交回 `None`。协议端点收到命令的回应时，先把已经到了的
    /// 推送都写出去，再写回应（`04-核心协议.md` 第六节第 2 条）。
    ///
    /// # Errors
    ///
    /// 同 [`Subscription::next`]。
    pub fn try_next(&mut self) -> Option<Result<Arc<Pushed>, Ended>> {
        if self.lagged {
            return Some(Err(Ended::Lagged));
        }
        match self.pushes.try_recv() {
            Ok(pushed) => Some(Ok(pushed)),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Lagged(_)) => {
                self.lagged = true;
                Some(Err(Ended::Lagged))
            }
            Err(TryRecvError::Closed) => Some(Err(Ended::Stopped)),
        }
    }
}

#[cfg(test)]
mod tests;
