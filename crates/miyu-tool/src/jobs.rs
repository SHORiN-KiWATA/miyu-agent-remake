//! 交给工具的任务端口（施工 7-3，`docs/blueprint/tools/interface.md`「后台命令」，`agents.md` 第四条）：`shell` 起好一条
//! 后台命令，交给它，拿回编号，当场返回；进程活过这一次调用，由执行器的任务表管：输出落盘、等它结束、整组杀。
//!
//! 工具只拿端口，不认识任务表（`agents.md`「在哪」）：怎么读输出、怎么等、怎么整组杀，是起它的工具知道的事，照
//! [`Process`] 交出来；编号怎么数、输出写到哪、结束了告诉谁，是执行器的事。

use std::fmt;
use std::io;

use miyu_kernel::id::JobId;

/// 任务端口：执行器照这一次调用造一个，交给工具（[`crate::Call::jobs`]）。只有 `shell` 用。
pub trait JobPort: Send + Sync {
    /// 把起好的后台命令交给任务表，交回它的编号。当场返回，不等它：输出一直读、写进会话目录，结束了由任务表告诉会话。
    ///
    /// # Errors
    ///
    /// 任务表收不下（输出的文件建不起来、会话已经停了）：这时任务表已经整组杀掉了它，交回原因。
    fn start(&self, command: Background) -> io::Result<JobId>;
}

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
