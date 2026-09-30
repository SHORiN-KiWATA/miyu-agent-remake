//! `miyu ask`（`docs/designs/22-命令行.md` 第三节，施工 3-9 下）：连上核心（没在跑就拉起来），开一个一次性
//! 会话，或者接着说；把一句话发给她，边收边打，她做的每一步印成一行（施工 4-5 下）；问完印一行用量。她派了子代理的，
//! 等它们都报回来、被叫醒的几轮也印完才退出（施工 7-9）。

pub(crate) mod follow;
mod steps;
mod talk;
mod usage;

pub use talk::talk;
pub(crate) use talk::{Watching, follow_turn};
pub(crate) use usage::Sum;

/// 问完那一行用量，例如 `· 输入 1,830 · 命中缓存 1,792（98%）· 输出 12`。
pub(crate) fn usage_line(language: &Language, sum: &Sum) -> String {
    let (input, hit, output) = (
        usage::thousands(sum.input()),
        usage::thousands(sum.cache_read),
        usage::thousands(sum.output),
    );
    match (language, sum.percent()) {
        (Language::Chinese, Some(percent)) => {
            format!("· 输入 {input} · 命中缓存 {hit}（{percent}%）· 输出 {output}")
        }
        (Language::Chinese, None) => format!("· 输入 {input} · 命中缓存 {hit} · 输出 {output}"),
        (Language::English, Some(percent)) => {
            format!("· input {input} · cache hit {hit} ({percent}%) · output {output}")
        }
        (Language::English, None) => format!("· input {input} · cache hit {hit} · output {output}"),
    }
}

use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::Duration;

use clap::{Args, ValueEnum};
use tokio::sync::mpsc;

use miyu_ipc::{ConnectError, connect_or_start};
use miyu_store::env::Env;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::language::{self, Language};
use crate::shown;

/// 退出码（`22-命令行.md` 第二节）。
pub mod exit {
    /// 成功。
    pub const OK: u8 = 0;
    /// 出错了：核心、模型或工具出了问题。
    pub const ERROR: u8 = 1;
    /// 被打断了：按了 Ctrl+C；等子代理的时候不等了，到了 `--timeout`（施工 7-9）。
    pub const INTERRUPTED: u8 = 3;
    /// 有几步要人确认，这里不问，没做（`22-命令行.md` O3）。
    pub const UNATTENDED: u8 = 4;
    /// 没有可用的模型。
    pub const NO_MODEL: u8 = 5;
}

/// `miyu ask` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Args)]
pub struct Ask {
    /// 要说的话：几个词用空格连起来。
    #[arg(required = true, num_args = 1..)]
    pub words: Vec<String>,
    /// 接着某个会话说。
    #[arg(short = 's', long, conflicts_with = "resume")]
    pub session: Option<String>,
    /// 接着上一次 `miyu ask` 开的会话说。
    #[arg(short = 'c', long = "continue", id = "resume")]
    pub resume: bool,
    /// 输出的格式。
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
    /// 加进来的目录：和工作区一样能读能写，可以写好几次（施工 5-10 上）。读参数时就换成绝对的、查它是目录。
    #[arg(long = "add-dir", value_name = "DIR", value_parser = directory)]
    pub add_dir: Vec<PathBuf>,
    /// 附件：可以写好几次，照写的先后（施工 3-9 三补）。读参数时只换成绝对的，读不读得了、多大由核心说。
    #[arg(long = "file", value_name = "FILE", value_parser = absolute)]
    pub file: Vec<PathBuf>,
    /// 最多等多久（施工 7-9）：从发出算到全部了结，到了打断、不再等。
    #[arg(long, value_name = "TIME", value_parser = duration)]
    pub timeout: Option<Duration>,
}

/// `--file` 的值：相对的照敲命令时的目录接成绝对的。
fn absolute(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|dir| dir.join(path))
        .map_err(|error| error.to_string())
}

/// `--add-dir` 的值：相对的照敲命令时的目录接成绝对的；要是一个已经有的目录，不然照「参数写错时」说。
fn directory(value: &str) -> Result<PathBuf, String> {
    let path = absolute(value)?;
    if path.is_dir() {
        Ok(path)
    } else {
        Err("not a directory".to_string())
    }
}

/// `--timeout` 的值（施工 7-9）：正整数，后面可以跟单位 `s`、`m`、`h`，不写是秒，照 GNU `timeout` 的写法；0、负数、小数、
/// 别的单位读不成，照「参数写错时」说（`cli/main.md`）。
fn duration(value: &str) -> Result<Duration, String> {
    let (number, seconds) = match value.as_bytes().last() {
        Some(b's') => (&value[..value.len() - 1], 1),
        Some(b'm') => (&value[..value.len() - 1], 60),
        Some(b'h') => (&value[..value.len() - 1], 3600),
        _ => (value, 1),
    };
    let bad = || "not a duration".to_string();
    // `u64` 自己读的时候认前面的 `+`：先查只有数字。
    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(bad());
    }
    number
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .and_then(|n| n.checked_mul(seconds))
        .map(Duration::from_secs)
        .ok_or_else(bad)
}

/// 输出的格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// 给人看：边收边打。
    Text,
    /// 给脚本：最后一行 JSON。
    Json,
}

/// 接哪个会话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// 开一个一次性的新会话（O2）。
    New,
    /// 接着上一次 `miyu ask` 开的会话。
    Continue,
    /// 接着这个会话。
    Session(String),
}

/// 这一次要说什么、怎么说。
#[derive(Debug, Clone)]
pub struct Plan {
    /// 要说的话。
    pub text: String,
    /// 接哪个会话。
    pub target: Target,
    /// 输出的格式。
    pub format: Format,
    /// 工作目录：会话的环境跟着它。
    pub cwd: String,
    /// 加进来的目录，绝对路径，照写的先后、去掉重复的（施工 5-10 上）：造会话、说话都带着，没有的也写空的。
    pub dirs: Vec<String>,
    /// 附件，绝对路径，照写的先后（施工 3-9 三补）：造会话之前一个个传给核心，说话时带着；没有的不写。
    pub files: Vec<String>,
    /// 界面语言。
    pub language: Language,
    /// 给人看的字，照界面语言读的那一份：她做的每一步怎么写（施工 4-5 下）。
    pub human: Human,
    /// 家目录：路径写成 `~/…`。
    pub home: Option<PathBuf>,
    /// 最多等多久（施工 7-9）：从发出算到全部了结；没有的一直等。
    pub timeout: Option<Duration>,
}

/// 写到哪里：回答写 `out`，思考、用量、出错写 `err`；`gray` 的思考、用量是灰色。
pub struct Screen<'a> {
    /// 标准输出。
    pub out: &'a mut dyn Write,
    /// 标准错误。
    pub err: &'a mut dyn Write,
    /// 标准错误是终端、没设 `NO_COLOR`。
    pub gray: bool,
    /// 标准错误是终端：压缩的进度在一行里原地刷新（施工 6-3 下）；不是的只印结果那一行。
    pub live: bool,
}

/// 跑一次 `miyu ask`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn ask(args: Ask, start: impl FnOnce() -> Command) -> ExitCode {
    let language = language::current();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(exit::ERROR);
        }
    };
    ExitCode::from(runtime.block_on(run(args, start, language)))
}

/// 在运行时里：找数据根、连上核心、说。
async fn run(args: Ask, start: impl FnOnce() -> Command, language: Language) -> u8 {
    let env = Env::current();
    let root = match DataRoot::locate(&env) {
        Ok(root) => root,
        Err(error) => return failed(&error.to_string()),
    };
    if let Err(error) = root.prepare() {
        return failed(&error.to_string());
    }
    let key = std::env::var("DEEPSEEK_API_KEY").is_ok_and(|key| !key.trim().is_empty());
    let connected = match key {
        true => connect_or_start(&root, start)
            .await
            .map_err(|error| error.to_string()),
        // 没有 key：核心在跑的照样连（它可能有），没在跑的不拉起。
        false => match miyu_ipc::connect(&root).await {
            Ok(connected) => Ok(connected),
            Err(ConnectError::NotRunning) => {
                eprintln!("{}", language.no_model());
                return exit::NO_MODEL;
            }
            Err(error) => Err(error.to_string()),
        },
    };
    let (connection, token) = match connected {
        Ok(connected) => connected,
        Err(reason) => return failed(&reason),
    };
    let plan = plan(args, &env, language);
    let presses = presses();
    let mut out = io::stdout();
    let mut err = io::stderr();
    let live = err.is_terminal();
    let gray = shown::colored(live, std::env::var_os("NO_COLOR").as_deref());
    let mut screen = Screen {
        out: &mut out,
        err: &mut err,
        gray,
        live,
    };
    talk(connection, &token, &plan, &mut screen, presses).await
}

/// 这一次要说什么、怎么说：照参数、进程的环境 `env`、界面语言；工作目录是敲命令时的目录。
fn plan(args: Ask, env: &Env, language: Language) -> Plan {
    Plan {
        text: args.words.join(" "),
        target: match (args.session, args.resume) {
            (Some(session), _) => Target::Session(session),
            (None, true) => Target::Continue,
            (None, false) => Target::New,
        },
        format: args.format,
        cwd: std::env::current_dir()
            .map_or_else(|_| ".".to_string(), |dir| dir.display().to_string()),
        dirs: added(&args.add_dir),
        files: args
            .file
            .iter()
            .map(|file| file.to_string_lossy().into_owned())
            .collect(),
        language,
        human: human(env, &language),
        home: env.home.clone(),
        timeout: args.timeout,
    }
}

/// 加进来的目录写成字：照写的先后，去掉重复的。
fn added(dirs: &[PathBuf]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for dir in dirs {
        let dir = dir.to_string_lossy().into_owned();
        if !out.contains(&dir) {
            out.push(dir);
        }
    }
    out
}

/// 给人看的字：照界面语言从资源目录读一份。读不出来的当没有，每一步照状态写最泛的一句（施工 4-5 下）。`miyu redo` 也用
/// （施工 4-7 再补）。
pub(crate) fn human(env: &Env, language: &Language) -> Human {
    ResourceRoot::locate(env)
        .ok()
        .and_then(|resources| Human::load(&resources, language.code()).ok())
        .unwrap_or_default()
}

/// Ctrl+C 一次送一个。装不上的就没有。`miyu compact` 也用（施工 6-8）。
pub(crate) fn presses() -> mpsc::Receiver<()> {
    let (press, presses) = mpsc::channel(4);
    tokio::spawn(async move {
        while tokio::signal::ctrl_c().await.is_ok() {
            if press.send(()).await.is_err() {
                break;
            }
        }
    });
    presses
}

/// 连上核心之前就出错了：原因写在标准错误上。
fn failed(reason: &str) -> u8 {
    eprintln!("{reason}");
    exit::ERROR
}

#[cfg(test)]
mod tests;
