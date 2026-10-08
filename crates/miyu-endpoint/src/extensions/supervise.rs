//! 看管一个扩展（施工 9-4 上，`docs/blueprint/extensions.md`「怎么走」第 1、2、4、5 条）：拉起，把标准输入输出交给协议的连接，
//! 等它退出，照退出的样子退避再拉起，或者停下。请它退出就是关它的标准输入；等一会儿没退的杀掉。

#[cfg(test)]
mod tests;

use std::io;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Weak;
use std::time::Duration;

use tokio::process::{Child, Command};
use tokio::sync::{oneshot, watch};
use tokio::time::Instant;

use super::{Reason, Shared, State, TARGET, stderr};
use crate::Core;
use crate::connection::serve_spawned;

/// 连续失败几次停下。
pub(super) const TRIES: u32 = 5;

/// Windows 上不弹控制台窗口：核心多半是没有控制台的后台进程，起控制台程序会新开一个窗口。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 等多久、退避多久。出厂的见 [`Timing::default`]；测试里设短的，不用真等。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// 请它退出以后等多久再杀。
    pub grace: Duration,
    /// 握了手、连着跑满多久，下一次失败从 1 数。
    pub stable: Duration,
    /// 第一次退避多久，之后每次翻倍。
    pub backoff: Duration,
    /// 退避最多多久。
    pub longest: Duration,
}

impl Default for Timing {
    /// 等 5 秒再杀，跑满 60 秒算稳了，退避从 1 秒起、最多 60 秒。
    fn default() -> Timing {
        Timing {
            grace: Duration::from_secs(5),
            stable: Duration::from_secs(60),
            backoff: Duration::from_secs(1),
            longest: Duration::from_secs(60),
        }
    }
}

/// 怎么拉起一个包。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Plan {
    /// 包的编号。
    pub(super) id: String,
    /// 程序：`miyu` 旁边的。
    pub(super) program: PathBuf,
    /// 参数：清单的 `[process] args`。
    pub(super) args: Vec<String>,
    /// 工作目录：包放状态的目录，没有的建。
    pub(super) dir: PathBuf,
    /// 核心手上的数据根：拉起时设成 `MIYU_HOME`（施工 O-18，`extensions.md`「怎么走」第 1 条）。
    pub(super) home: PathBuf,
    /// 核心手上的资源目录：拉起时设成 `MIYU_RESOURCES`（施工 O-18）。
    pub(super) resources: PathBuf,
    /// 标准错误接到哪个文件。
    pub(super) log: PathBuf,
}

/// 退出了一次以后怎么办。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Next {
    /// 过 `after` 再拉起；连续失败了 `failures` 次。
    Retry { failures: u32, after: Duration },
    /// 停下；连续失败了 `failures` 次。
    Stop { reason: Reason, failures: u32 },
}

/// 已经连续失败了 `failures` 次，这次退出码是 `code`（被信号杀掉、等不到的没有），`stable` 是握了手、连着跑满了：退出码 1
/// 停下；跑满了的这一次从 1 数，不然加一；到了 [`TRIES`] 停下，没到的退避。
pub(super) fn next(timing: &Timing, failures: u32, code: Option<i32>, stable: bool) -> Next {
    let failures = if stable { 1 } else { failures + 1 };
    if code == Some(1) {
        return Next::Stop {
            reason: Reason::ConfigError,
            failures,
        };
    }
    if failures >= TRIES {
        return Next::Stop {
            reason: Reason::FailedRepeatedly,
            failures,
        };
    }
    Next::Retry {
        failures,
        after: backoff(timing, failures),
    }
}

/// 第 `failures` 次失败以后退避多久：第一次 `backoff`，之后每次翻倍，最多 `longest`。
pub(super) fn backoff(timing: &Timing, failures: u32) -> Duration {
    let doubled = timing
        .backoff
        .saturating_mul(1 << failures.saturating_sub(1).min(16));
    doubled.min(timing.longest)
}

/// 看管包 `plan.id`，直到停下，或者 `asked` 来了（请它退出、等它退出以后交回，状态由叫停的一方记）。核心没了的也交回。
pub(super) async fn run(
    core: Weak<Core>,
    plan: Plan,
    shared: Shared,
    mut asked: watch::Receiver<bool>,
    timing: Timing,
) {
    let id = plan.id.as_str();
    let mut failures = 0;
    loop {
        let Some(owner) = core.upgrade() else {
            return;
        };
        let mut child = match spawn(&plan) {
            Ok(child) => child,
            Err(error) => {
                tracing::warn!(target: TARGET, package = id, error = %error, "extension not started");
                set(&shared, State::Stopped(Reason::CannotStart), failures);
                return;
            }
        };
        let pid = child.id().unwrap_or_default();
        tracing::info!(target: TARGET, package = id, pid, "extension started");
        set(&shared, State::Starting { pid: Some(pid) }, failures);
        let (Some(input), Some(output)) = (child.stdin.take(), child.stdout.take()) else {
            // 两头都接了管道，不会没有。
            set(&shared, State::Stopped(Reason::CannotStart), failures);
            return;
        };
        let (ready, shook) = oneshot::channel();
        let serving = serve_spawned(tokio::io::join(output, input), owner, ready, id.to_string());
        let watched = Watched {
            shared: &shared,
            id,
            pid,
            failures,
        };
        let (stopping, ready_at) = watched.serve(serving, shook, &mut asked).await;
        let status = finish(&mut child, timing.grace).await;
        let code = status.as_ref().and_then(std::process::ExitStatus::code);
        let said = status.map_or_else(|| "unknown".to_string(), |status| status.to_string());
        if stopping {
            tracing::info!(target: TARGET, package = id, status = said.as_str(), "extension exited");
            return;
        }
        tracing::warn!(target: TARGET, package = id, status = said.as_str(), "extension exited");
        let stable = ready_at.is_some_and(|at| at.elapsed() >= timing.stable);
        match next(&timing, failures, code, stable) {
            Next::Stop { reason, failures } => {
                tracing::warn!(target: TARGET, package = id, reason = reason.as_str(), failures, "extension stopped");
                set(&shared, State::Stopped(reason), failures);
                return;
            }
            Next::Retry {
                failures: now,
                after,
            } => {
                failures = now;
                tracing::info!(target: TARGET, package = id, failures, after_ms = after.as_millis(), "extension waiting");
                set(
                    &shared,
                    State::Waiting {
                        until: Instant::now() + after,
                    },
                    failures,
                );
                tokio::select! {
                    () = tokio::time::sleep(after) => {}
                    _ = asked.changed() => return,
                }
            }
        }
    }
}

/// 一次拉起，连着的这一段。
struct Watched<'a> {
    shared: &'a Shared,
    id: &'a str,
    pid: u32,
    failures: u32,
}

impl Watched<'_> {
    /// 等连接断了，或者请它退出；握成了记成在跑。交回是不是请它退出的、什么时候握成的。交回时连接已经丢下了：它的标准输入
    /// 关了。
    async fn serve(
        &self,
        serving: impl Future<Output = ()>,
        mut shook: oneshot::Receiver<()>,
        asked: &mut watch::Receiver<bool>,
    ) -> (bool, Option<Instant>) {
        tokio::pin!(serving);
        let mut ready_at = None;
        let mut listening = true;
        loop {
            tokio::select! {
                () = &mut serving => return (false, ready_at),
                done = &mut shook, if listening => {
                    listening = false;
                    if done.is_ok() {
                        ready_at = Some(Instant::now());
                        tracing::info!(target: TARGET, package = self.id, pid = self.pid, "extension ready");
                        set(self.shared, State::Running { pid: self.pid }, self.failures);
                    }
                }
                _ = asked.changed() => return (true, ready_at),
            }
        }
    }
}

/// 拉起：工作目录没有的建，标准错误接到它的文件上，核心丢下它时杀掉。
fn spawn(plan: &Plan) -> io::Result<Child> {
    std::fs::create_dir_all(&plan.dir)?;
    let errors = stderr::open(&plan.log)?;
    let mut command = std::process::Command::new(&plan.program);
    // 环境照核心的，只把这两个设成核心手上的（施工 O-18）：扩展和核心认同一份数据根、资源目录，不管核心是怎么找到它们的。
    command
        .args(&plan.args)
        .env("MIYU_HOME", &plan.home)
        .env("MIYU_RESOURCES", &plan.resources)
        .current_dir(&plan.dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(errors);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut command = Command::from(command);
    command.kill_on_drop(true);
    command.spawn()
}

/// 连接丢下了（标准输入关了）：等它退出，最多等 `grace`，没退的杀掉。交回它怎么退出的；等不到的没有。
async fn finish(child: &mut Child, grace: Duration) -> Option<std::process::ExitStatus> {
    if let Ok(waited) = tokio::time::timeout(grace, child.wait()).await {
        return waited.ok();
    }
    if let Err(error) = child.start_kill() {
        tracing::warn!(target: TARGET, error = %error, "extension not killed");
    }
    child.wait().await.ok()
}

/// 记下状态，广播一声。
fn set(shared: &Shared, state: State, failures: u32) {
    shared.set(state, failures);
}
