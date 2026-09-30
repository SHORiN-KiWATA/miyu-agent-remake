//! 起命令、读输出、等它结束（施工 4-8）：标准输入接空的，标准输出、标准错误接到同一根管道上，照写出来的先后；
//! 到时、叫停时整组杀掉。
//!
//! - Unix 上命令自成一个进程组，用 `rustix` 的安全接口 `kill(-pgid, SIGKILL)` 杀整组。命令退出以后，组里还在跑的
//!   （`&` 放到后台的）也杀掉：命令做完了，它起的东西不留。后台命令（施工 7-3）照同一套起、同一套杀，见
//!   `background.rs`。
//! - 核心崩了，命令跟着没（施工 7-8，`miyu_sandbox::lifeline`）：Unix 上组里先起一个看门的，读到核心的生命线断了就杀整组，
//!   组号是看门的那一个；起不来的照旧自成一组，记一行 `WARN`。Windows 上核心自己在作业对象里，这里什么都不用做。
//! - Windows 上用系统自带的 `taskkill /T /F` 杀整棵进程树，只在命令还在跑的时候杀：进程编号回收得快，退出以后再
//!   按编号杀可能杀错。
//! - 命令退出以后，管道最多再读 [`DRAIN`]：还有东西拿着管道的（Windows 上它放出去的孙进程），不等它。
//! - 到时杀了以后最多再等 [`KILL_WAIT`]：还不结束的也算超时，不再等它（施工 4-9 再补二）。

use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use super::output::{Capture, Decoder};

/// 命令退出以后，管道最多再读多久。
const DRAIN: Duration = Duration::from_millis(500);

/// 到时杀了以后，最多再等多久：还不结束的（Windows 上 `taskkill` 没杀掉之类）不再等它（施工 4-9 再补二）。
const KILL_WAIT: Duration = Duration::from_secs(5);

/// 运行日志的来处：工具的名字（`28-运行日志.md` 第二节；施工 4-9 再补四上：原来写 `miyu::basesystem`，10 个字，
/// 来源那一列对不齐）。
const TARGET: &str = "miyu::shell";

/// Windows 上起的程序不弹控制台窗口。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 一条命令怎么结束的。
#[derive(Debug)]
pub(super) enum Ending {
    /// 自己结束了：退出码，或者（Unix）杀掉它的信号。
    Exited(ExitStatus),
    /// 到时了，整组杀掉了。
    TimedOut,
}

/// 跑完的一条命令：怎么结束的，读到的输出。
#[derive(Debug)]
pub(super) struct Finished {
    /// 怎么结束的。
    pub(super) ending: Ending,
    /// 读到的输出。
    pub(super) output: Capture,
}

/// 起好的一条命令：它的进程、整组杀的时候认它的那个编号、读输出的线程、看门的。
pub(super) struct Started {
    child: Child,
    group: Group,
    output: Arc<Mutex<Capture>>,
    read: mpsc::Receiver<()>,
    /// 等完了、整组杀过了才丢掉：丢掉时收掉它（施工 7-8）。
    _watch: Watch,
}

/// 组里看门的（施工 7-8，`miyu_sandbox::lifeline`）：Unix 上一个组一个，丢掉时收掉；别的平台没有（Windows 上核心自己在
/// 作业对象里），是空的。
pub(super) struct Watch {
    /// 看门的：起不来的没有。
    #[cfg(unix)]
    watcher: Option<miyu_sandbox::lifeline::Watcher>,
}

/// 起好的一条命令的几样：进程、整组杀时认的编号、管道读的一头、看门的。
pub(super) struct Spawned {
    pub(super) child: Child,
    pub(super) group: Group,
    pub(super) pipe: io::PipeReader,
    pub(super) watch: Watch,
}

/// 起命令：标准输入接空的，标准输出、标准错误接到同一根管道上，自成一组（Unix 上组里有看门的，施工 7-8）。交回它的
/// 进程、整组杀的时候认它的那个编号、管道读的一头、看门的。前台、后台（施工 7-3）都这样起。
///
/// # Errors
///
/// 管道建不起来，或者程序起不来（找不到、工作目录不在）。
pub(super) fn spawn(mut command: Command) -> io::Result<Spawned> {
    let (pipe, writer) = io::pipe()?;
    command
        .stdin(Stdio::null())
        .stdout(writer.try_clone()?)
        .stderr(writer);
    let watch = alone(&mut command);
    // 父进程手里的写端在 `command` 里，这个函数一返回就跟着它关掉：留着的话，命令都退出了读的一头也读不到结尾。
    let child = command.spawn()?;
    let group = Group(group_of(&watch, &child));
    Ok(Spawned {
        child,
        group,
        pipe,
        watch,
    })
}

/// 整组杀时认的编号：Unix 上有看门的是它的组，没有的是命令自己的组；Windows 上是命令的进程号（杀整棵树）。
#[cfg(unix)]
fn group_of(watch: &Watch, child: &Child) -> u32 {
    watch
        .watcher
        .as_ref()
        .map_or_else(|| child.id(), miyu_sandbox::lifeline::Watcher::group)
}

/// 同上：别的平台是命令的进程号。
#[cfg(not(unix))]
fn group_of(_watch: &Watch, child: &Child) -> u32 {
    child.id()
}

/// 起前台的命令：读到的一段段解成字交给 `progress`。
///
/// # Errors
///
/// 同 [`spawn`]。
pub(super) fn start(
    command: Command,
    progress: impl Fn(String) + Send + 'static,
) -> io::Result<Started> {
    let Spawned {
        child,
        group,
        pipe,
        watch,
    } = spawn(command)?;
    let output = Arc::new(Mutex::new(Capture::default()));
    let (done, read) = mpsc::channel();
    let theirs = Arc::clone(&output);
    // 读的线程带着派活时的 span：它发的行也有会话编号（施工 4-9 再补四上）。
    let span = tracing::Span::current();
    thread::spawn(move || {
        let _entered = span.enter();
        pump(pipe, &theirs, &progress);
        #[expect(
            clippy::let_underscore_must_use,
            reason = "等的那一头可能已经不等了，没人收也不要紧"
        )]
        let _ = done.send(());
    });
    Ok(Started {
        child,
        group,
        output,
        read,
        _watch: watch,
    })
}

impl Started {
    /// 整组杀的时候认它。
    pub(super) fn group(&self) -> Group {
        self.group
    }

    /// 等它结束，最多等 `timeout`：到时整组杀掉。
    ///
    /// # Errors
    ///
    /// 等不了：系统报了错。
    pub(super) fn wait(self, timeout: Duration) -> io::Result<Finished> {
        self.wait_or_kill(timeout, KILL_WAIT, Group::kill)
    }

    /// 同 [`Self::wait`]：到时用 `kill` 整组杀，杀了以后最多再等 `patience`，还不结束的也算超时，不再等它。测试里
    /// 换成杀不掉的、等得短的。
    fn wait_or_kill(
        self,
        timeout: Duration,
        patience: Duration,
        kill: impl FnOnce(Group),
    ) -> io::Result<Finished> {
        let Started {
            mut child,
            group,
            output,
            read,
            _watch,
        } = self;
        let (exited, waited) = mpsc::channel();
        thread::spawn(move || {
            let status = child.wait();
            // 进程的句柄跟着交回去：Windows 上它还开着，编号就不会被别的进程拿去。
            #[expect(
                clippy::let_underscore_must_use,
                reason = "等的那一头走了，结局就没人要了"
            )]
            let _ = exited.send((status, child));
        });
        let gone = || io::Error::other("the thread waiting for the command stopped");
        let (ending, child) = match waited.recv_timeout(timeout) {
            Ok((status, child)) => (Ending::Exited(status?), Some(child)),
            Err(RecvTimeoutError::Timeout) => {
                kill(group);
                match waited.recv_timeout(patience) {
                    Ok((status, child)) => {
                        status?;
                        (Ending::TimedOut, Some(child))
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        tracing::warn!(target: TARGET, "command still running after kill");
                        (Ending::TimedOut, None)
                    }
                    Err(RecvTimeoutError::Disconnected) => return Err(gone()),
                }
            }
            Err(RecvTimeoutError::Disconnected) => return Err(gone()),
        };
        group.leftovers();
        drop(child);
        if read.recv_timeout(DRAIN).is_err() {
            tracing::debug!(target: TARGET, "command output still open after the command ended");
        }
        let output = std::mem::take(&mut *lock(&output));
        Ok(Finished { ending, output })
    }
}

/// 读管道，直到读完或者读不了。
fn pump(mut pipe: io::PipeReader, output: &Mutex<Capture>, progress: &impl Fn(String)) {
    let mut decoder = Decoder::default();
    let mut buffer = [0u8; 8192];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                let chunk = &buffer[..count];
                lock(output).push(chunk);
                let text = decoder.push(chunk);
                if !text.is_empty() {
                    progress(text);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                tracing::debug!(target: TARGET, error = %error, "command output not readable");
                break;
            }
        }
    }
    let rest = decoder.finish();
    if !rest.is_empty() {
        progress(rest);
    }
}

/// 读到的输出：读的线程 panic 了也照样拿。
fn lock(output: &Mutex<Capture>) -> MutexGuard<'_, Capture> {
    output.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 一条命令和它起的：Unix 上是它的进程组，Windows 上是它的进程树。认的是命令的进程编号。
#[derive(Debug, Clone, Copy)]
pub(super) struct Group(u32);

impl Group {
    /// 整组杀掉。
    pub(super) fn kill(self) {
        kill(self.0);
    }

    /// 命令退出以后，组里还在跑的也杀掉。Windows 上不杀：退出以后按编号杀可能杀错。
    pub(super) fn leftovers(self) {
        if cfg!(unix) {
            kill(self.0);
        }
    }
}

/// Unix：`kill(-pgid, SIGKILL)`。组里已经没有进程的，不算出错。
#[cfg(unix)]
fn kill(id: u32) {
    use rustix::process::{Pid, Signal, kill_process_group};
    let Some(pid) = i32::try_from(id).ok().and_then(Pid::from_raw) else {
        return;
    };
    match kill_process_group(pid, Signal::KILL) {
        Ok(()) => {}
        Err(error) if error == rustix::io::Errno::SRCH => {}
        Err(error) => tracing::warn!(target: TARGET, error = %error, "command group not killed"),
    }
}

/// Windows：`taskkill /T /F /PID`，整棵进程树。
#[cfg(windows)]
fn kill(id: u32) {
    use std::os::windows::process::CommandExt;
    let killed = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &id.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    if let Err(error) = killed {
        tracing::warn!(target: TARGET, error = %error, "command tree not killed");
    }
}

/// 别的系统：只能杀它自己，这里拿不到它，不杀。
#[cfg(not(any(unix, windows)))]
fn kill(_id: u32) {}

/// 让命令自成一组，整组杀得掉：Unix 上先起看门的，命令进它的组（施工 7-8），起不来的照旧自成一组。Windows 上不弹控制台
/// 窗口。
#[cfg(unix)]
fn alone(command: &mut Command) -> Watch {
    use std::os::unix::process::CommandExt;
    let watcher = match miyu_sandbox::lifeline::watcher() {
        Ok(watcher) => match i32::try_from(watcher.group()) {
            Ok(group) => {
                command.process_group(group);
                Some(watcher)
            }
            Err(_) => {
                command.process_group(0);
                None
            }
        },
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "command lifeline not started");
            command.process_group(0);
            None
        }
    };
    Watch { watcher }
}

/// 同上：Windows 上不弹控制台窗口。
#[cfg(windows)]
fn alone(command: &mut Command) -> Watch {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
    Watch {}
}

/// 同上：别的系统什么都不做。
#[cfg(not(any(unix, windows)))]
fn alone(_command: &mut Command) -> Watch {
    Watch {}
}

/// 掐掉时整组杀掉：丢掉它就杀；跑完了先 [`Guard::disarm`]。掐掉就是丢掉这次调用的 future（施工 4-2）；执行命令
/// 不会被「叫它停」，那只给改文件的（施工 4-9 再补一）。
pub(super) struct Guard(Option<Group>);

impl Guard {
    /// 看着 `group`。
    pub(super) fn new(group: Group) -> Guard {
        Guard(Some(group))
    }

    /// 跑完了，丢掉它也不杀。
    pub(super) fn disarm(mut self) {
        self.0 = None;
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(group) = self.0.take() {
            group.kill();
        }
    }
}

#[cfg(all(test, unix))]
mod tests;
