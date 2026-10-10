//! 交给工具的任务端口（施工 7-3，`docs/blueprint/tools/interface.md`「后台命令」，`agents.md` 第四条）：`shell` 起好一条
//! 后台命令，交给它，拿回编号，当场返回；进程活过这一次调用，由执行器的任务表管：输出落盘、等它结束、整组杀。
//!
//! 工具只拿端口，不认识任务表（`agents.md`「在哪」）：怎么读输出、怎么等、怎么整组杀，是起它的工具知道的事，照
//! [`Process`] 交出来；编号怎么数、输出写到哪、结束了告诉谁，是执行器的事。
//!
//! 查和停（施工 7-4，`docs/blueprint/tools/jobs.md`）：`jobs` 经同一个端口列出这个会话派出去的任务、读输出、停掉。后台命令和
//! 子代理都管：子代理那一头执行器经会话表的端口去读、去停，工具不认识会话表。

/// 「后台运行」这个功能的编号（施工 T-1 上，设计 `30-插件框架.md` 第三节第 6、7 条）：基础系统里管 `jobs`、`shell` 放到后台、
/// 子代理在后台跑。关着的会话没有后台。
pub const BACKGROUND: &str = "background";

/// `shell` 这件工具的名字。
pub const SHELL: &str = "shell";

/// `shell` 放到后台那一项参数的名字：后台运行关着的会话里工具面上没有它（施工 T-1 上）。
pub const RUN_IN_BACKGROUND: &str = "run_in_background";

use std::fmt;
use std::future::Future;
use std::io::{self, Read};
use std::pin::Pin;

use miyu_kernel::event::JobKind;
use miyu_kernel::id::JobId;

/// 任务端口：执行器照这一次调用造一个，交给工具（[`crate::Call::jobs`]）。`shell` 交后台命令，`jobs` 查、停（施工 7-4）。
pub trait JobPort: Send + Sync {
    /// 把起好的后台命令交给任务表，交回它的编号。当场返回，不等它：输出一直读、写进会话目录，结束了由任务表告诉会话。
    ///
    /// # Errors
    ///
    /// 任务表收不下（输出的文件建不起来、会话已经停了）：这时任务表已经整组杀掉了它，交回原因。
    fn start(&self, command: Background) -> io::Result<JobId>;

    /// 这个会话派出去的任务（施工 7-4）：还没结束的全部，和最近结束的几个，照编号排。照日志算，用时照这一刻。
    fn list(&self) -> Vec<Listed>;

    /// 读任务 `job` 的输出（施工 7-4）：后台命令读到这时的输出，结束了的读存下的那一份；子代理读它最近的回答和这一步在跑
    /// 什么。
    fn output(&self, job: JobId) -> Asking<'_, Result<Output, JobError>>;

    /// 停掉任务 `job`（施工 7-4）：后台命令整组杀掉，子代理停掉它这一轮连它派的。回报由执行器记下，带 `by_model`，不叫醒
    /// 她。停好了才交回。
    fn stop(&self, job: JobId) -> Asking<'_, Result<(), JobError>>;
}

/// 查、停交回的 future（施工 7-4）。
pub type Asking<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// 列出来的一个任务（施工 7-4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// 编号。
    pub job: JobId,
    /// 后台命令、子代理，不认识的原样。
    pub what: JobKind,
    /// 派它时给的标题。
    pub title: String,
    /// 结束了的是最后那条回报的 `reason`（`exited`、`stopped`、`done`……）；还在跑的是空的。
    pub ended: Option<String>,
    /// 用时，毫秒：结束了的到结束，还在跑的到这一刻。
    pub took_ms: u64,
}

/// 读到的一个任务的输出（施工 7-4）。
pub struct Output {
    /// 后台命令、子代理。
    pub what: JobKind,
    /// 读到的字：后台命令的输出，子代理最近的回答，已经是合法的 UTF-8。没有的（输出没存下来、子代理还没说过话）是空的。
    pub text: Option<Box<dyn Read + Send>>,
    /// 还在跑：后台命令还没结束，子代理这一轮还没完。
    pub running: bool,
    /// 子代理这一步在跑的工具，照先后；后台命令、闲着的子代理是空的。
    pub doing: Vec<String>,
}

impl fmt::Debug for Output {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Output")
            .field("what", &self.what)
            .field("text", &self.text.is_some())
            .field("running", &self.running)
            .field("doing", &self.doing)
            .finish()
    }
}

/// 读不了、停不了（施工 7-4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobError {
    /// 这个会话没有这个任务。
    Unknown,
    /// 已经结束了：停不了。
    Ended,
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            JobError::Unknown => "no such job",
            JobError::Ended => "the job has already ended",
        })
    }
}

impl std::error::Error for JobError {}

/// 一条起好的后台命令：它的输出、它的进程。
pub struct Background {
    /// 输出：标准输出、标准错误合成一根管道，照写出来的先后，已经照前台的规矩合法化了（解成字、`\r\n` 换成 `\n`），
    /// 一段一段交出来。读的时候会等；管道关了（命令和它起的都退出了、关了输出）就没有了。
    pub output: Box<dyn Iterator<Item = String> + Send>,
    /// 进程：等它结束、整组杀。
    pub process: Box<dyn Process>,
}

impl fmt::Debug for Background {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Background")
    }
}

/// 起好的一条后台命令的进程，由起它的工具实现（`shell` 的 `run_in_background`）。任务表在一个线程里等它、在别处杀它，
/// 两件可以同时来。
pub trait Process: Send + Sync {
    /// 等它结束，交回怎么结束的；会等。只调一次。
    ///
    /// # Errors
    ///
    /// 等不了：系统报了错。
    fn wait(&self) -> io::Result<Exit>;

    /// 整组杀掉：Unix 上它的进程组，Windows 上它的进程树。已经结束了的什么都不做：退出以后再按编号杀，可能杀错。
    fn kill(&self);
}

/// 一条后台命令怎么结束的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// 自己退出了：退出码，照系统交回的原样（Windows 上的 NTSTATUS 是负的）。
    Code(i32),
    /// 被信号杀掉了（Unix）：信号的编号。
    Signal(u32),
}

impl fmt::Debug for dyn JobPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JobPort")
    }
}

/// 两个端口比的是不是同一个：[`crate::Call`] 照格子比较时用。
impl PartialEq for dyn JobPort {
    fn eq(&self, other: &dyn JobPort) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl Eq for dyn JobPort {}
