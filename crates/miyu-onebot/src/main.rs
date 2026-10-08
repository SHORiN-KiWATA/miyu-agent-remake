//! 程序 `miyu-onebot`（`onebot.md` 第一条「对外的样子」）：`miyu-onebot serve` 前台跑，Ctrl+C、SIGTERM 停。9-4 以前由人手动
//! 起；9-4 以后由核心拉起。
//!
//! 先找资源目录、读给人看的字（[`Texts`]），之后说给人听的都照它；再装运行日志 `state/logs/onebot.log`，读两项配置（令牌
//! 没设的说清怎么设、退出码 1）和 `bridge.json`，交给 [`run`]。说给人听的都在标准错误上：读配置以前照系统的语言，读了
//! 配置照 `ui.language`，握手以后照核心回的语言。找不到资源目录、给人看的字读不懂，这时还没有字可用，印原话。

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::{Arc, Mutex, PoisonError};

use miyu_endpoint::config::Environment;
use miyu_onebot::TARGET;
use miyu_onebot::serve::{Failure, Notice, Serve, run};
use miyu_onebot::settings::{load, system_language};
use miyu_onebot::texts::Texts;
use miyu_onebot::tuning::Tuning;
use miyu_store::env::Env;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

/// 用法不对的退出码。
const USAGE: u8 = 2;

/// 起不来、停了的退出码（「出错」）。
const FAILED: u8 = 1;

/// 还没有字可用时印原话的开头。
const PROGRAM: &str = "miyu-onebot";

fn main() -> ExitCode {
    let env = Env::current();
    let locale = miyu_store::env::locale();
    let texts = ResourceRoot::locate(&env)
        .map_err(|error| error.to_string())
        .and_then(|resources| {
            Texts::load(resources, &system_language(locale.as_deref()))
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
    if args != ["serve"] {
        eprintln!("{}", texts.usage());
        return ExitCode::from(USAGE);
    }
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
    ExitCode::from(runtime.block_on(serve(root, &env, locale, &mut texts)))
}

/// 装运行日志、读配置和 `bridge.json`、跑到停。`locale` 是系统的语言。交回退出码。
async fn serve(root: DataRoot, env: &Env, locale: Option<String>, texts: &mut Texts) -> u8 {
    let log = miyu_log::install(
        &root.state().join("logs"),
        "onebot",
        miyu_log::LevelFilter::INFO,
        env.home.as_deref(),
    );
    if let Err(error) = &log {
        eprintln!("{}", texts.no_log(&error.to_string()));
    }
    let loaded = load(
        &root,
        env.home.as_deref(),
        locale.as_deref(),
        Environment::process(),
    );
    if let Err(error) = texts.speak(&loaded.language) {
        eprintln!("{PROGRAM}: {error}");
        return FAILED;
    }
    let settings = match loaded.settings {
        Ok(settings) => settings,
        Err(unready) => {
            eprintln!("{}", texts.unready(&unready));
            return FAILED;
        }
    };
    let tuning = match Tuning::load(texts.resources().path()) {
        Ok(tuning) => tuning,
        Err(reason) => {
            eprintln!("{}", texts.failure(&Failure::Start(reason)));
            return FAILED;
        }
    };
    let serve = Serve {
        root,
        settings,
        core: Arc::new(core),
        locale,
        tuning,
    };
    let speaking = Arc::new(Mutex::new(texts.clone()));
    let telling = Arc::clone(&speaking);
    let tell = move |notice: Notice| {
        let mut texts = telling.lock().unwrap_or_else(PoisonError::into_inner);
        if let Notice::Listening { language, .. } = &notice
            && let Err(error) = texts.speak(language)
        {
            // 握手回的那种语言的字读不懂：照实说一行、记进运行日志，接着照原来的语言说。
            tracing::warn!(target: TARGET, error = %error, "human texts not read");
            eprintln!("{PROGRAM}: {error}");
        }
        eprintln!("{}", texts.notice(&notice));
    };
    match run(serve, tell, stopped()).await {
        Ok(()) => 0,
        Err(failure) => {
            let texts = speaking.lock().unwrap_or_else(PoisonError::into_inner);
            eprintln!("{}", texts.failure(&failure));
            FAILED
        }
    }
}

/// 拉起核心：自己真实位置旁边的主程序 `miyu` 加 `core`（照网页软件）。
fn core() -> Command {
    let myself = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("miyu-onebot"));
    let mut core = Command::new(sibling(&myself, "miyu"));
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
