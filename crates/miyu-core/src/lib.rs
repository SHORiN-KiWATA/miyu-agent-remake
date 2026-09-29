//! 核心进程（`docs/designs/12-进程形态与分发.md` 第二节，施工 3-9 上）：`miyu core`，由头拉起，平时不用人敲。
//!
//! 起来的先后：
//!
//! 1. 找数据根，建骨架；
//! 2. 拿单实例锁：已经有一个核心在跑的，说一声 `running` 就走；先拿锁再装日志，免得两个核心写同一份；
//! 3. 装运行日志 `state/logs/core.log`，记一条「起来了」：版本、进程号、数据根、和 UTC 差多少；
//! 4. 管理员 `admin` 的家目录，没有就建；资源目录；模型（[`models`]）；
//! 5. 换本机令牌、在套接字上等连接（施工 3-8 下）；找沙盒的助手、探一次，只记日志（施工 5-1）；
//! 6. 往标准输出写一行 `ready`：拉起它的头等着这一行；接着在后台清一次回收处（施工 3-8 三补，`trash.rs`）。
//!
//! 之后 [`serve()`] 一个个接连接：没有连接、也没有在跑的回合，空闲够久了就退出；收到停的信号，先让在跑的
//! 会话有计划地停下再退出。起不来的，把原因写成那一行（`error …`）交给头。

pub mod models;
mod sandbox;
mod serve;
mod trash;

pub use serve::{Stopped, serve};

use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use miyu_endpoint::Core;
use miyu_ipc::{Dirs, Lock, OpenError, Ready};
use miyu_kernel::id::AccountId;
use miyu_store::env::Env;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_tool::Catalog;

/// 运行日志的目标。
const TARGET: &str = "miyu::core";

/// 空闲多久退出：没有连接、也没有在跑的回合，连续这么久。以后放进配置。
pub const IDLE: Duration = Duration::from_secs(600);

/// 核心的运行时开几个线程：接连接、会话、请求都是等 I/O，两个够用，占用也低。
const WORKERS: usize = 2;

/// `miyu core` 的参数。
#[derive(Debug, Clone)]
pub struct Options {
    /// 空闲多久退出。
    pub idle: Duration,
}

/// 管理员：本机连上来的都是他，账号固定叫 `admin`（`06-多用户与身份.md` U13）。
pub fn admin() -> AccountId {
    AccountId::parse("admin").unwrap_or_else(|e| unreachable!("「admin」合账号的写法：{e}"))
}

/// 跑核心进程，交回退出码。
pub fn main(options: Options) -> ExitCode {
    let env = Env::current();
    let root = match DataRoot::locate(&env) {
        Ok(root) => root,
        Err(error) => return failed("data_root", error.to_string()),
    };
    if let Err(error) = root.prepare() {
        return failed("data_root", error.to_string());
    }
    let lock = match Lock::acquire(&root) {
        Ok(lock) => lock,
        Err(OpenError::Running) => {
            say(&Ready::Running);
            return ExitCode::SUCCESS;
        }
        Err(error) => return failed("lock", error.to_string()),
    };
    let level = std::env::var("MIYU_LOG").ok();
    let logs = root.state().join("logs");
    let _log = match miyu_log::install(&logs, "core", level.as_deref(), env.home.as_deref()) {
        Ok(guard) => guard,
        Err(error) => return failed("log", error.to_string()),
    };
    tracing::info!(
        target: TARGET,
        version = env!("CARGO_PKG_VERSION"),
        pid = std::process::id(),
        root = %root.path().display(),
        tz = %miyu_log::utc_offset(),
        "starting"
    );
    if let Err(error) = root.prepare_home(&admin()) {
        return failed("home", error.to_string());
    }
    let resources = match ResourceRoot::locate(&env) {
        Ok(resources) => resources,
        Err(error) => return failed("resources", error.to_string()),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(WORKERS)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return failed("runtime", error.to_string()),
    };
    runtime.block_on(run(env, root, resources, lock, options))
}

/// 工具目录：核心起来时登记一次，登记完就冻结（`05-内核接口.md` 第八节）。施工 4-4 起登记基础系统，工具的字从
/// 资源目录 `resources` 读。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对；登记时查不过（重名、名字或参数格式不合写法）。
pub fn tools(resources: &ResourceRoot) -> Result<Catalog, String> {
    let base = miyu_basesystem::tools(resources.path()).map_err(|error| error.to_string())?;
    Catalog::new(base).map_err(|error| error.to_string())
}

/// 后半段，在运行时里：在套接字上等连接，说「好了」，接连接，直到停下。`env` 是起来时读的那一份环境快照。
async fn run(
    env: Env,
    root: DataRoot,
    resources: ResourceRoot,
    lock: Lock,
    options: Options,
) -> ExitCode {
    let opened = match miyu_ipc::open_locked(&root, &Dirs::current(), lock) {
        Ok(opened) => opened,
        Err(error) => return failed("socket", error.to_string()),
    };
    let table = match resources
        .models()
        .map_err(|error| error.to_string())
        .and_then(|text| models::ModelTable::parse(&text))
    {
        Ok(table) => table,
        Err(error) => return failed("models", error),
    };
    let env_models = models::ModelEnv {
        key: std::env::var("DEEPSEEK_API_KEY").ok(),
        base_url: std::env::var("MIYU_DEV_BASE_URL").ok(),
        model: std::env::var("MIYU_DEV_MODEL").ok(),
        window: std::env::var("MIYU_DEV_WINDOW").ok(),
    };
    let models = match models::from_env(&env_models, &table) {
        Ok(models) => models,
        Err(error) => return failed("models", error),
    };
    let sandbox = sandbox::probe(env.exe.as_deref());
    let sandbox_cache = sandbox::cache(&env, std::env::var_os("CARGO_HOME"));
    let tools = match tools(&resources) {
        Ok(tools) => tools,
        Err(error) => return failed("tools", error),
    };
    let trashed = root.clone();
    let mut core = Core::new(
        root,
        resources,
        models,
        tools,
        env.home,
        admin(),
        opened.token,
    )
    .with_sandbox(sandbox);
    if let Some((cache, cargo_home)) = sandbox_cache {
        core = core.with_sandbox_cache(cache, cargo_home);
    }
    let core = Arc::new(core);
    say(&Ready::Ready);
    let purging = trash::purge(trashed, admin());
    serve(opened.listener, core, options.idle, serve::signal()).await;
    if let Err(error) = purging.await {
        tracing::error!(target: TARGET, error = %error, "trash purge panicked");
    }
    ExitCode::SUCCESS
}

/// 起不来：原因写成那一行交给头，头印给人看；运行日志（装上了的话）只记没过的是哪一步 `stage`，原因是给人看的
/// 中文，不进日志（施工 4-9 再补四上）。
fn failed(stage: &'static str, reason: String) -> ExitCode {
    tracing::warn!(target: TARGET, stage, "not started");
    say(&Ready::Failed(reason));
    ExitCode::FAILURE
}

/// 往标准输出写那一行：拉起核心的头在管道的另一头等着。
fn say(ready: &Ready) {
    let mut out = io::stdout();
    if let Err(error) = out
        .write_all(ready.line().as_bytes())
        .and_then(|()| out.flush())
    {
        tracing::warn!(target: TARGET, error = %error, "ready line not written");
    }
}
