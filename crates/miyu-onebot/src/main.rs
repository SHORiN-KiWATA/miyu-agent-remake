//! 程序 `miyu-onebot`（`onebot.md` 第一条「对外的样子」）。`miyu onebot …` 经核心的子命令转交到这里（9-2）。
//!
//! - `serve`：只由核心拉起（施工 O-18，`extensions.md`）：标准输入输出是跟核心说协议的管道（[`Pipe`]），标准输出上只有
//!   协议；说给人听的在标准错误上，核心收进 `state/logs/onebot.stderr`。核心关了标准输入、Ctrl+C、SIGTERM 好好停下。
//! - `start`、`stop`、`restart`、`status`：调核心的 `extension.*`（施工 O-18，[`control`]）。
//! - `logs [-f]`：印运行日志和标准错误（施工 O-18，[`logs`]）。
//! - `venue show <场所>`：一个场所每一项的值和来处（施工 O-21，[`show`]）。
//! - `-h`、`--help`：用法印在标准输出上，退出码 0（`miyu help onebot` 转成 `--help`）。
//!
//! 先找资源目录、读给人看的字（[`Texts`]，照系统的语言），之后说给人听的都照它。`serve` 再装运行日志
//! `state/logs/onebot.log`，读 `bridge.json`、清单里 NapCat 端口的默认值（[`Defaults`]）和出厂的场所规则、出厂参数、违规词表
//! （[`Factory`]，施工 O-21：有问题是打包的错，说 [`Failure::Factory`]、退出码 1），交给 [`run`]：配置由核心在握手的回应里
//! 交、变了推过来，桥不读系统配置（施工 O-20）。握手以前不说话：起不来的照系统的语言说一句；运行日志装不上的那一句等握手回了
//! 语言再说；握手回了语言就照它说，端口被占那一句也是（「施工时定的」第 42 条）。`start`、`stop`、`restart`、`status`
//! 握手以后照核心回的语言说，`logs`、`venue show` 照系统的语言。找不到资源目录、给人看的字读不懂，这时还没有字可用，印原话。

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex, PoisonError};

use miyu_onebot::control::{Control, CoreCommand, control};
use miyu_onebot::logs::logs;
use miyu_onebot::rules::Factory;
use miyu_onebot::serve::{Failure, Notice, Pipe, Serve, run};
use miyu_onebot::settings::Defaults;
use miyu_onebot::texts::{Texts, system_language};
use miyu_onebot::tuning::Tuning;
use miyu_onebot::venue::show;
use miyu_onebot::{PROGRAM, TARGET};
use miyu_store::env::Env;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

/// 用法不对的退出码。
const USAGE: u8 = 2;

/// 起不来、停了的退出码（「出错」）。
const FAILED: u8 = 1;

/// 要做的那一件。
enum Command {
    /// `serve`。
    Serve,
    /// `start`、`stop`、`restart`、`status`。
    Control(Control),
    /// `logs`，带不带 `-f`。
    Logs { follow: bool },
    /// `venue show`：场所编号的原文。
    Venue(String),
}

fn main() -> ExitCode {
    let env = Env::current();
    let locale = miyu_store::env::locale();
    let texts = ResourceRoot::locate(&env)
        .map_err(|error| error.to_string())
        .and_then(|resources| {
            Texts::load(resources, system_language(locale.as_deref()))
                .map_err(|error| error.to_string())
        });
    let mut texts = match texts {
        Ok(texts) => texts,
        Err(error) => {
            eprintln!("{PROGRAM}: {error}");
            return ExitCode::from(FAILED);
        }
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["serve"] => Command::Serve,
        ["start"] => Command::Control(Control::Start),
        ["stop"] => Command::Control(Control::Stop),
        ["restart"] => Command::Control(Control::Restart),
        ["status"] => Command::Control(Control::Status),
        ["logs"] => Command::Logs { follow: false },
        ["logs", "-f"] => Command::Logs { follow: true },
        ["venue", "show", venue] => Command::Venue(venue.to_string()),
        ["-h"] | ["--help"] => {
            println!("{}", texts.usage());
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("{}", texts.usage());
            return ExitCode::from(USAGE);
        }
    };
    let fail = |texts: &Texts, reason: String| {
        eprintln!("{}", texts.failure(&Failure::Start(reason)));
        ExitCode::from(FAILED)
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => return fail(&texts, error.to_string()),
    };
    let root = match DataRoot::locate(&env) {
        Ok(root) => root,
        Err(error) => return fail(&texts, error.to_string()),
    };
    if let Err(error) = root.prepare() {
        return fail(&texts, error.to_string());
    }
    let code = match command {
        Command::Serve => runtime.block_on(serve(root, &env, locale, &mut texts)),
        Command::Control(which) => {
            let core: CoreCommand = Arc::new(core);
            runtime.block_on(control(
                &root,
                which,
                &core,
                &mut texts,
                &mut std::io::stdout(),
                &mut std::io::stderr(),
            ))
        }
        Command::Logs { follow } => {
            let every = match follow.then(|| Tuning::load(texts.resources().path())) {
                None => None,
                Some(Ok(tuning)) => Some(tuning.follow()),
                Some(Err(reason)) => return fail(&texts, reason),
            };
            logs(
                &root,
                every,
                &texts,
                &mut std::io::stdout(),
                &mut std::io::stderr(),
            )
        }
        Command::Venue(venue) => show(
            &root,
            &venue,
            &texts,
            &mut std::io::stdout(),
            &mut std::io::stderr(),
        ),
    };
    // 标准输入在阻塞线程里读（`serve` 的管道）：核心还开着它时，等那个线程会一直等下去，不等它，进程退出时一起收掉。
    runtime.shutdown_background();
    ExitCode::from(code)
}

/// 装运行日志、读 `bridge.json`、清单里 NapCat 端口的默认值和出厂的场所规则这几样、跑到停。`locale` 是系统的语言。交回退出码。
async fn serve(root: DataRoot, env: &Env, locale: Option<String>, texts: &mut Texts) -> u8 {
    let log = miyu_log::install(
        &root.state().join("logs"),
        "onebot",
        miyu_log::LevelFilter::INFO,
        env.home.as_deref(),
    );
    // 运行日志装不上的那一句先欠着：握手回了语言照它说；握手以前就起不来的，说起不来的原因以前先说（「施工时定的」第 42 条）。
    let owed = Arc::new(Mutex::new(log.err().map(|error| error.to_string())));
    let loaded = Tuning::load(texts.resources().path())
        .and_then(|tuning| Defaults::load(texts.resources()).map(|defaults| (tuning, defaults)));
    let (tuning, defaults) = match loaded {
        Ok(loaded) => loaded,
        Err(reason) => {
            settle(texts, &owed);
            eprintln!("{}", texts.failure(&Failure::Start(reason)));
            return FAILED;
        }
    };
    let factory = match Factory::load(texts.resources()) {
        Ok(factory) => factory,
        Err(problems) => {
            settle(texts, &owed);
            eprintln!("{}", texts.failure(&Failure::Factory(problems)));
            return FAILED;
        }
    };
    let serve = Serve {
        root,
        pipe: Pipe::new(tokio::io::stdin(), tokio::io::stdout()),
        locale,
        tuning,
        resources: texts.resources().clone(),
        defaults,
        factory,
    };
    let speaking = Arc::new(Mutex::new(texts.clone()));
    let shaken = {
        let (speaking, owed) = (Arc::clone(&speaking), Arc::clone(&owed));
        move |language: &str| {
            let mut texts = lock(&speaking);
            if let Err(error) = texts.speak(language) {
                // 握手回的那种语言的字读不懂：照实说一行、记进运行日志，接着照原来的语言说。
                tracing::warn!(target: TARGET, error = %error, "human texts not read");
                eprintln!("{PROGRAM}: {error}");
            }
            settle(&texts, &owed);
        }
    };
    let telling = Arc::clone(&speaking);
    let tell = move |notice: Notice| eprintln!("{}", lock(&telling).notice(&notice));
    match run(serve, shaken, tell, stopped()).await {
        Ok(()) => 0,
        Err(failure) => {
            let texts = lock(&speaking);
            settle(&texts, &owed);
            eprintln!("{}", texts.failure(&failure));
            FAILED
        }
    }
}

/// 欠着的「运行日志写不了」照 `texts` 说出来，只说一次。
fn settle(texts: &Texts, owed: &Mutex<Option<String>>) {
    if let Some(reason) = lock(owed).take() {
        eprintln!("{}", texts.no_log(&reason));
    }
}

/// 锁里不 `await`、不会崩；真崩了，里面的照样能用。
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 拉起核心：自己真实位置旁边的主程序 `miyu` 加 `core`（照网页软件）。`start` 这几样连核心时用。
fn core() -> std::process::Command {
    let myself = std::env::current_exe().unwrap_or_else(|_| PathBuf::from(PROGRAM));
    let mut core = std::process::Command::new(sibling(&myself, "miyu"));
    core.arg("core");
    core
}

/// `program` 真实位置旁边叫 `name` 的程序（Windows 上加 `.exe`）。
fn sibling(program: &Path, name: &str) -> PathBuf {
    let real = std::fs::canonicalize(program).unwrap_or_else(|_| program.to_path_buf());
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    real.parent()
        .map_or_else(|| PathBuf::from(&file), |dir| dir.join(&file))
}

/// 收到停的信号：Ctrl+C，Unix 上另有 SIGTERM（照网页软件）。
async fn stopped() {
    #[cfg(unix)]
    {
        let Ok(mut term) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        else {
            return std::future::pending().await;
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}
