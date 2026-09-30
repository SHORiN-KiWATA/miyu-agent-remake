//! Unix 上的生命线（`lifeline.rs`）：一根管道，一个组一个看门的。

use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

/// 看门的那一段：读到结尾（生命线的写端全关了）就杀掉自己所在的整个组，自己也在里面。`read` 和 `kill` 都是内建的，
/// 不经 `PATH`；没人往管道里写，`read` 一直等到结尾。
const SCRIPT: &str = "while read -r _; do :; done; kill -s KILL 0";

/// 看门的 shell：各个 Unix 都有，不经 `PATH` 找。
const SHELL: &str = "/bin/sh";

/// 一根生命线：写端拿在手里，谁都不给；读端交给每个看门的。
#[derive(Debug)]
pub struct Lifeline {
    reader: io::PipeReader,
    /// 只拿着，不写：它关了（这个进程没了，或者丢掉了这根线），看门的读到结尾。
    _writer: io::PipeWriter,
}

impl Lifeline {
    /// 一根新的生命线。两头都带 `O_CLOEXEC`（标准库建的管道都是）：起别的子进程时不会漏给它们。
    ///
    /// # Errors
    ///
    /// 管道建不起来（文件描述符用完了之类）。
    pub fn new() -> io::Result<Lifeline> {
        let (reader, writer) = io::pipe()?;
        Ok(Lifeline {
            reader,
            _writer: writer,
        })
    }

    /// 起一个看门的，自成一组（组号就是它的进程号）：命令照 [`Watcher::group`] 起在这个组里。
    ///
    /// # Errors
    ///
    /// 读端复制不了，或者 `/bin/sh` 起不来。
    pub fn watcher(&self) -> io::Result<Watcher> {
        let child = Command::new(SHELL)
            .args(["-c", SCRIPT])
            .env_clear()
            .current_dir("/")
            .stdin(Stdio::from(self.reader.try_clone()?))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()?;
        Ok(Watcher { child })
    }
}

/// 这个进程的那一根：第一次要用时建，一直拿到进程结束。建不起来的，之后每次都说建不起来。
fn lifeline() -> io::Result<&'static Lifeline> {
    static LIFELINE: OnceLock<Result<Lifeline, String>> = OnceLock::new();
    LIFELINE
        .get_or_init(|| Lifeline::new().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|error| io::Error::other(error.clone()))
}

/// 在这个进程的生命线上起一个看门的（[`Lifeline::watcher`]）。
///
/// # Errors
///
/// 生命线建不起来，或者看门的起不来。
pub fn watcher() -> io::Result<Watcher> {
    lifeline()?.watcher()
}

/// 一个组里看门的。整组杀的时候它跟着没；丢掉它时杀掉它、收掉它，不留僵尸进程。
#[derive(Debug)]
pub struct Watcher {
    child: Child,
}

impl Watcher {
    /// 它的组：命令起在这个组里（`process_group`），整组杀也照它（`kill(-组号)`）。
    pub fn group(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for Watcher {
    /// 杀掉、收掉。它还没被收过，编号还是它的，按编号杀不会杀错；已经跟着整组没了的，杀它是空的，收它马上回来。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "已经没了的杀不了、收不了都不要紧：只为不留僵尸进程"
    )]
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
