//! 程序 `miyu-onebot`（`onebot.md` 第一条「对外的样子」）：`miyu-onebot serve` 前台跑，Ctrl+C、SIGTERM 停。9-4 以前由人手动
//! 起；9-4 以后由核心拉起。`miyu-onebot web [--print]` 打开桥的 WebUI（第二条，施工 O-16，[`open`]）。
//!
//! 先找资源目录、读给人看的字（[`Texts`]），之后说给人听的都照它；再装运行日志 `state/logs/onebot.log`，读三项配置（端口
//! 读不出来的照实说、退出码 1；令牌没设的照样往下走，NapCat 连进来 401，设了不用重启，O-16 补、补二）和 `bridge.json`，
//! 交给 [`run`]。说给人听的都在标准错误上：读配置以前照系统的语言，读了配置照 `ui.language`，握手以后照核心回的语言。找不到
//! 资源目录、给人看的字读不懂，这时还没有字可用，印原话。

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex, PoisonError};

use miyu_endpoint::config::Environment;
use miyu_onebot::open::{Open, SystemBrowser, open};
use miyu_onebot::serve::{CoreCommand, Failure, Notice, Serve, run};
use miyu_onebot::settings::{Settings, load, system_language};
use miyu_onebot::texts::Texts;
use miyu_onebot::tuning::Tuning;
use miyu_onebot::{PROGRAM, TARGET};
use miyu_store::env::Env;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

/// 用法不对的退出码。
const USAGE: u8 = 2;

/// 起不来、停了的退出码（「出错」）。
const FAILED: u8 = 1;

/// 要做的那一件：`serve`，或者 `web`（带不带 `--print`）。
enum Command {
    Serve,
    Web(Open),
}

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
    let command = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["serve"] => Command::Serve,
        ["web"] => Command::Web(Open::default()),
        ["web", "--print"] => Command::Web(Open { print: true }),
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
        Command::Web(wanted) => runtime.block_on(web(root, &env, locale, &wanted, &mut texts)),
    };
    ExitCode::from(code)
}

/// 读配置（三项、说话的语言）：端口读不出来的照实说。令牌没设不算：交给后面（`serve` 只开 WebUI，`web` 照常打开）。交回
/// 三项，或者交回退出码。
fn read(
    root: &DataRoot,
    env: &Env,
    locale: Option<&str>,
    texts: &mut Texts,
) -> Result<Settings, u8> {
    let loaded = load(root, env.home.as_deref(), locale, Environment::process());
    if let Err(error) = texts.speak(&loaded.language) {
        eprintln!("{PROGRAM}: {error}");
        return Err(FAILED);
    }
    loaded.settings.map_err(|unready| {
        eprintln!("{}", texts.unready(&unready));
        FAILED
    })
}

/// `web`：读 `onebot.web`，照 [`open`] 开浏览器。交回退出码。
async fn web(
    root: DataRoot,
    env: &Env,
    locale: Option<String>,
    wanted: &Open,
    texts: &mut Texts,
) -> u8 {
    let settings = match read(&root, env, locale.as_deref(), texts) {
        Ok(settings) => settings,
        Err(code) => return code,
    };
    let core: CoreCommand = Arc::new(core);
    open(
        &root,
        settings.web,
        wanted,
        &core,
        &SystemBrowser,
        texts,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await
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
    let settings = match read(&root, env, locale.as_deref(), texts) {
        Ok(settings) => settings,
        Err(code) => return code,
    };
    let tuning = match Tuning::load(texts.resources().path()) {
        Ok(tuning) => tuning,
        Err(reason) => {
            eprintln!("{}", texts.failure(&Failure::Start(reason)));
            return FAILED;
        }
    };
    // NapCat 的令牌对不上时、WebUI 的 `/status`、`/token`、`/apply` 照起来时的数据根、家目录、系统的语言和环境重读配置
    // （O-16 补二）。
    let reload = {
        let (root, home, locale) = (root.clone(), env.home.clone(), locale.clone());
        Arc::new(move || {
            load(
                &root,
                home.as_deref(),
                locale.as_deref(),
                Environment::process(),
            )
            .settings
        })
    };
    let serve = Serve {
        root,
        settings,
        core: Arc::new(core),
        locale,
        tuning,
        resources: texts.resources().clone(),
        reload,
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
