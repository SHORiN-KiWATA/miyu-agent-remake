//! 后台命令（施工 7-3，`docs/blueprint/tools/shell.md`「后台」，`agents.md` 第四条）：照前台一样起（同一个 shell、同一份
//! 环境变量、同一个沙盒，自成一组），起来了交给任务端口，调用当场返回。进程活过这一次调用：读输出、等它结束、整组杀，
//! 由任务表经这里的 [`Process`] 做。
//!
//! 输出照前台的规矩合法化（`output.rs`）：一根管道，照 UTF-8 边读边解、一个字切在两段中间的留到下一段，`\r\n` 换成
//! `\n`；一段一段交出去，不截。
//!
//! 整组杀照前台（`process.rs`）：Unix 杀进程组，Windows `taskkill /T` 杀整棵树；已经结束了的不再按编号杀。命令自己
//! 退出以后，Unix 上组里还在跑的也杀掉，和前台一样。核心崩了，命令跟着没，也和前台一样（施工 7-8）：Unix 上组里有看门的，
//! Windows 上核心在作业对象里。

use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus};
use std::sync::{Mutex, MutexGuard, PoisonError};

use miyu_tool::{Background, Exit, Process};

use super::output::{Crlf, Decoder};
use super::process::{self, Group};

/// 运行日志的来处，和前台一样（`process.rs`）。
const TARGET: &str = "miyu::shell";

/// 起一条后台命令：交回它的输出和进程，交给任务端口。
///
/// # Errors
///
/// 同前台：管道建不起来，或者程序起不来。
pub(super) fn start(command: Command) -> io::Result<Background> {
    let process::Spawned {
        child,
        group,
        pipe,
        watch,
    } = process::spawn(command)?;
    Ok(Background {
        output: Box::new(Output {
            pipe,
            decoder: Decoder::default(),
            crlf: Crlf::default(),
            ended: false,
        }),
        process: Box::new(Running {
            group,
            child: Mutex::new(Some(child)),
            exited: Mutex::new(false),
            _watch: watch,
        }),
    })
}

/// 读输出：一次读 8192 字节，解成字、换好行尾交出去。读完了（管道关了）、读不了的，把剩下的交出去就没有了。
struct Output {
    pipe: io::PipeReader,
    decoder: Decoder,
    crlf: Crlf,
    ended: bool,
}

impl Iterator for Output {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        let mut buffer = [0u8; 8192];
        while !self.ended {
            match self.pipe.read(&mut buffer) {
                Ok(0) => self.ended = true,
                Ok(count) => {
                    let text = self.crlf.push(&self.decoder.push(&buffer[..count]));
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    tracing::debug!(target: TARGET, error = %error, "command output not readable");
                    self.ended = true;
                }
            }
        }
        let rest = self.crlf.push(&self.decoder.finish()) + &self.crlf.finish();
        (!rest.is_empty()).then_some(rest)
    }
}

/// 起好的进程：等的那一头拿走它的句柄，杀的那一头只认组。
struct Running {
    group: Group,
    /// 等它结束的那一头拿走。
    child: Mutex<Option<Child>>,
    /// 等到了、句柄就要放下：之后不再按编号杀。Windows 上句柄开着编号就不会被别的进程拿去，所以先记下它、再放下
    /// 句柄；杀的那一头拿着这把锁杀，杀完之前句柄放不下。
    exited: Mutex<bool>,
    /// 组里看门的（施工 7-8）：核心崩了它杀整组；丢掉这条命令时收掉它。
    _watch: process::Watch,
}

impl Process for Running {
    fn wait(&self) -> io::Result<Exit> {
        let mut child = lock(&self.child)
            .take()
            .ok_or_else(|| io::Error::other("the command was already waited for"))?;
        let status = child.wait();
        *lock(&self.exited) = true;
        drop(child);
        let status = status?;
        self.group.leftovers();
        Ok(exit(status))
    }

    fn kill(&self) {
        let exited = lock(&self.exited);
        if !*exited {
            self.group.kill();
        }
    }
}

/// 读锁：拿着它的线程 panic 了也照样拿。
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 怎么结束的：有退出码的照退出码；没有的是被信号杀掉的（Unix）。
fn exit(status: ExitStatus) -> Exit {
    match status.code() {
        Some(code) => Exit::Code(code),
        None => Exit::Signal(signal(status)),
    }
}

/// 杀掉它的信号（Unix）。
#[cfg(unix)]
fn signal(status: ExitStatus) -> u32 {
    use std::os::unix::process::ExitStatusExt;
    status
        .signal()
        .and_then(|signal| u32::try_from(signal).ok())
        .unwrap_or(0)
}

/// 别的系统没有信号：退出码总是有的，走不到这里。
#[cfg(not(unix))]
fn signal(_status: ExitStatus) -> u32 {
    0
}
