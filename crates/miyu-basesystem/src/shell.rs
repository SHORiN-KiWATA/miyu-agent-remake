//! `shell`（`10-自带软件.md` 第三节「`shell` 的细则」，施工 4-8）：在这一轮的工作目录里执行一条命令，交回输出和
//! 退出码。前台的：跑完才交回，超时、叫停时整组杀掉。写了 `run_in_background` 的放到后台（施工 7-3）：照前台一样起，
//! 交给任务端口，当场交回编号。
//!
//! 用哪个 shell（[`program`]）、命令拿得到哪些环境变量（[`env`](mod@env)）、起命令和整组杀（[`process`]）、输出怎么截
//! （[`output`]）、后台命令（[`background`]）各在一处。每次调用起一个新的 shell，`cd`、变量都不带到下一次。调用带了
//! 沙盒的，经沙盒的助手起（施工 5-1），别的都照旧。

mod background;
mod env;
mod output;
mod process;
mod program;
#[cfg(test)]
mod tests;

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use miyu_fs::tilde;
use miyu_kernel::event::{JobKind, JobStarted};
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Effect, Progress, Running, Spec, Tool};

use crate::blocking::blocking;
use crate::common::{Common, said, said_n};
use crate::load::{self, LoadError, say};
use output::{Capture, Shown};
use process::{Ending, Finished, Guard};
use program::Program;

/// 不写 `timeout` 的等多久，毫秒（照 Claude Code）。改它要跟着改参数格式里的那一句。
const DEFAULT: u64 = 120_000;

/// `timeout` 最多写多大，毫秒（照 Claude Code）。改它要跟着改参数格式里的那一句。
const MAX: u64 = 600_000;

/// `shell`。
pub(crate) struct Shell {
    spec: Spec,
    texts: Texts,
    program: Program,
}

/// 输出里给她看的几句：`software/basesystem/shell/*.txt`，和几件工具共用的。
struct Texts {
    common: Common,
    empty: Template,
    exit: Template,
    signal: Template,
    timed_out: Template,
    omitted: Template,
    truncated: Template,
    failed: Template,
    no_background: Template,
    started: Template,
}

/// 她给的参数。
#[derive(Deserialize)]
struct Args {
    command: String,
    /// 这条命令在做什么的短标题（施工 4-13，2026-09-28 项目主人定）：必填，前台的执行不用它，记在调用里给前端显示；
    /// 后台的是任务的标题，记进 `job.started`（施工 7-3）。
    description: String,
    /// 前台的超时；后台的不看它（施工 7-3）。
    timeout: Option<u64>,
    #[serde(default)]
    run_in_background: bool,
    /// 这一次要在沙盒外跑（施工 D-4）：只报给权限策略和执行器，套不套沙盒照 `Call::sandbox`。
    #[serde(default)]
    outside_sandbox: bool,
}

impl Shell {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。用哪个 shell 在这时候找，写进说明里。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<Shell, LoadError> {
        let program = Program::find(std::env::var_os("PATH").as_deref());
        let text = |name: &str, fields: &[&str]| load::text(resources, "shell", name, fields);
        Ok(Shell {
            spec: load::spec_filled(
                resources,
                "shell",
                Access::Execute,
                &[("shell", program.kind.name())],
            )?,
            texts: Texts {
                common,
                empty: text("empty", &[])?,
                exit: text("exit", &["code"])?,
                signal: text("signal", &["signal"])?,
                timed_out: text("timed-out", &["timeout", "max"])?,
                omitted: text("omitted", &["count"])?,
                truncated: text("truncated", &["total"])?,
                failed: text("failed", &["shell", "error"])?,
                no_background: text("no-background", &[])?,
                started: text("started", &["job"])?,
            },
            program,
        })
    }

    /// 起不来、等不了。
    fn failed(&self, error: &io::Error) -> Done {
        let error = error.to_string();
        Done::error(say(
            &self.texts.failed,
            &[("shell", self.program.kind.name()), ("error", &error)],
        ))
        .said(said("shell/failed").with("error", error))
    }

    /// 放到后台（施工 7-3）：照前台一样造命令、起进程，交给任务端口，当场交回编号，报 `job.started`。这里一个 `await`
    /// 都没有：交上了就一定交回结果，不会交上了却被掐掉、没人知道它在跑。没有任务端口的（会话外面的调用）不跑。
    fn background(&self, call: &Call, args: Args) -> Done {
        let Some(jobs) = &call.jobs else {
            return Done::error(say(&self.texts.no_background, &[]))
                .said(said("shell/no-background"));
        };
        let started = self
            .program
            .command(
                &args.command,
                &workdir(call),
                env::passed(std::env::vars_os()),
                call.sandbox.as_deref(),
            )
            .and_then(background::start)
            .and_then(|command| jobs.start(command));
        match started {
            Ok(job) => {
                let id = job.to_string();
                Done::ok(say(&self.texts.started, &[("job", &id)]))
                    .said(said("shell/background").with("job", id))
                    .effect(Effect::JobStarted(JobStarted {
                        foreground: false,
                        job,
                        what: JobKind::Command,
                        title: args.description,
                        session: None,
                    }))
            }
            Err(error) => self.failed(&error),
        }
    }

    /// 跑完了：输出，加上它怎么结束的。
    fn finished(&self, finished: Finished, timeout: u64) -> Done {
        let Finished { ending, output } = finished;
        let body = self.body(&output);
        match ending {
            Ending::TimedOut => {
                let note = say(
                    &self.texts.timed_out,
                    &[("timeout", &timeout.to_string()), ("max", &MAX.to_string())],
                );
                Done::error(body + &note)
                    .said(said("shell/timed-out").with("seconds", seconds(timeout)))
            }
            Ending::Exited(status) => match status.code() {
                Some(0) if output.is_empty() => {
                    Done::ok(say(&self.texts.empty, &[])).said(said("shell/quiet"))
                }
                Some(0) => Done::ok(body).said(said_n("shell/done", "count", output.lines())),
                Some(code) => {
                    let code = code.to_string();
                    Done::error(body + &say(&self.texts.exit, &[("code", &code)]))
                        .said(said("shell/exited").with("code", code))
                }
                None => {
                    let signal = signal(status);
                    Done::error(body + &say(&self.texts.signal, &[("signal", &signal)]))
                        .said(said("shell/signal").with("signal", signal))
                }
            },
        }
    }

    /// 给她看的输出：太长的截成头尾两段，中间说省了多少，末尾说一共多少、怎么看全。每一段都以换行结尾。
    fn body(&self, output: &Capture) -> String {
        match output.shown() {
            Shown::Whole(text) => line(text),
            Shown::Cut {
                head,
                tail,
                omitted,
                total,
            } => {
                let omitted = say(&self.texts.omitted, &[("count", &omitted.to_string())]);
                let truncated = say(&self.texts.truncated, &[("total", &total.to_string())]);
                line(head) + &omitted + &line(tail) + &truncated
            }
        }
    }
}

impl Tool for Shell {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn outside_sandbox(&self, call: &Call) -> bool {
        serde_json::from_str::<Args>(&call.args).is_ok_and(|args| args.outside_sandbox)
    }

    fn asking(&self, call: &Call) -> Vec<(&'static str, String)> {
        serde_json::from_str::<Args>(&call.args).map_or_else(
            |_| Vec::new(),
            |args| vec![("command", args.command), ("title", args.description)],
        )
    }

    fn run(&self, call: Call, progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return self.texts.common.bad_args(&error),
            };
            if args.run_in_background {
                return self.background(&call, args);
            }
            let timeout = limit(args.timeout);
            let command = match self.program.command(
                &args.command,
                &workdir(&call),
                env::passed(std::env::vars_os()),
                call.sandbox.as_deref(),
            ) {
                Ok(command) => command,
                Err(error) => return self.failed(&error),
            };
            let started = match process::start(command, move |text| progress.push(text)) {
                Ok(started) => started,
                Err(error) => return self.failed(&error),
            };
            // 起来了就看着：这次调用被叫停（future 被丢掉）时，整组杀掉。
            let guard = Guard::new(started.group());
            let wait = Duration::from_millis(timeout);
            let finished = blocking(call.stop.clone(), move |_| started.wait(wait)).await;
            guard.disarm();
            match finished {
                Ok(finished) => self.finished(finished, timeout),
                Err(error) => self.failed(&error),
            }
        })
    }
}

/// 命令在哪个目录里跑：这一轮的工作目录，`~` 开头的照家目录接上，和别的工具一个规矩（施工 4-9 再补二）；没有家目录
/// 的照原样，起不来时说工作目录不在。
fn workdir(call: &Call) -> PathBuf {
    match (tilde(&call.cwd), call.home.as_deref()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(&call.cwd),
    }
}

/// 她写的 `timeout`：没写、写 0 的按默认，超过上限的按上限。
fn limit(timeout: Option<u64>) -> u64 {
    match timeout {
        None | Some(0) => DEFAULT,
        Some(timeout) => timeout.min(MAX),
    }
}

/// 给人看的秒数：整秒的不带小数。
fn seconds(millis: u64) -> String {
    match millis % 1000 {
        0 => (millis / 1000).to_string(),
        _ => format!("{:.1}", millis as f64 / 1000.0),
    }
}

/// 不是空的，就以换行结尾。
fn line(mut text: String) -> String {
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// 杀掉它的信号（Unix）。
#[cfg(unix)]
fn signal(status: std::process::ExitStatus) -> String {
    use std::os::unix::process::ExitStatusExt;
    status
        .signal()
        .map_or_else(|| "?".to_string(), |signal| signal.to_string())
}

/// 别的系统没有信号：退出码总是有的，走不到这里。
#[cfg(not(unix))]
fn signal(_status: std::process::ExitStatus) -> String {
    "?".to_string()
}
