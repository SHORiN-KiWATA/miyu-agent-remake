//! 核心没了，它起的命令跟着没（施工 7-8，`docs/blueprint/core.md`「子进程随核心退出」，`12-进程形态与分发.md` R5）。
//! 有计划地停下时核心自己整组杀掉后台命令（施工 7-3）；这里管的是核心崩了、被 `SIGKILL`、被任务管理器结束的时候：
//! 没人来杀，命令得自己知道核心没了。三个平台各用自己最稳的现成办法：
//!
//! - Unix（Linux、macOS 一样）：一根「生命线」管道，写端只在核心手里（`O_CLOEXEC`，别的子进程拿不到），核心怎么没的，
//!   内核都会关掉它。每条命令的进程组里先起一个看门的 `/bin/sh`，标准输入是读端：读到结尾（核心没了），用
//!   `kill -s KILL 0` 杀掉自己所在的整个组。命令起在这个组里，前台、后台、经不经沙盒的助手都一样（`Watcher`）。
//!   看门的只认自己的组，组里还有它就不会被别的组占了编号，所以不会杀错。
//! - Windows：核心起来时把自己放进一个带 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 的作业对象（[`bind_children`]），之后起的
//!   子进程、子进程再起的都在里面；核心没了，作业的最后一个句柄跟着关，里面的全部结束。
//!
//! 没用 Linux 的 `PR_SET_PDEATHSIG`：它盯的是起子进程的那个线程，线程池的线程一退，命令就被误杀；也只管直接的子进程，组里
//! 别的照样活。没用 macOS 的 kqueue：要另起一个进程盯着，和管道一样，却只有一个平台能用。

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{Lifeline, Watcher, watcher};

/// 把核心自己放进一个作业对象，核心没了，它起的子进程都跟着结束（Windows）。核心起来时调一次。别的平台什么都不做：
/// 那里靠每条命令组里看门的（`watcher`）。
///
/// # Errors
///
/// 作业对象建不成、设不上、放不进去：交回系统的原话，调的一方记一行运行日志，照旧起来。
pub fn bind_children() -> std::io::Result<()> {
    #[cfg(windows)]
    return windows::bind_children();
    #[cfg(not(windows))]
    Ok(())
}

#[cfg(all(test, unix))]
mod tests;
