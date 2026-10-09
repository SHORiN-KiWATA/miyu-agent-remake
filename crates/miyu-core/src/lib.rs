//! 核心进程（`docs/designs/12-进程形态与分发.md` 第二节，施工 3-9 上）：`miyu core`，由头拉起，平时不用人敲。
//!
//! 起来的先后：
//!
//! 1. 找数据根，建骨架；
//! 2. 拿单实例锁：已经有一个核心在跑的，说一声 `running` 就走；先拿锁再装日志，免得两个核心写同一份；
//! 3. 装运行日志 `state/logs/core.log`，记一条「起来了」：版本、进程号、数据根、和 UTC 差多少；
//! 4. 管理员 `admin` 的家目录，没有就建；资源目录；读配置、照 `log.level` 换运行日志的级别，照配置清单生成两份 JSON
//!    Schema 和参考文件（[`settings`]，施工 8-1、8-2）；供应商的档案、认原厂的表，造会话的路由（[`models`]，施工 8-6、8-7）；开始监视配置文件，配置换了当场换级别、重写
//!    生成的文件（施工 8-4）；
//! 5. 换本机令牌、在套接字上等连接（施工 3-8 下）；找沙盒的助手、探一次，只记日志（施工 5-1）；照编进来的可选
//!    软件包往查询表里登记（[`packages::register`]，cargo 开关 `mermaid`、`net`，施工 W-4、W-7），交给 `Core`；清掉管理员
//!    分块上传留下的暂存（[`packages::clear_uploads`]，施工 W-5）；
//! 6. 往标准输出写一行 `ready`：拉起它的头等着这一行；接着照开关拉起开着的扩展（施工 9-4 上）；在后台读 models.dev 的目录、
//!    用出来的、供应商的列表，读完再答要它的，之后在后台更新目录（施工 8-7）；在后台清一次回收处（施工 3-8 三补，`trash.rs`）。
//!
//! 之后 [`serve()`] 一个个接连接：没有连接、也没有在跑的回合，空闲够久了就退出；收到停的信号，先让在跑的
//! 会话有计划地停下再退出。起不来的，把原因写成那一行（`error …`）交给头。

mod embed;
pub mod models;
pub mod packages;
mod sandbox;
mod serve;
pub mod settings;
mod trash;

pub use serve::{Stopped, serve};

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use miyu_endpoint::Core;
use miyu_ipc::{Dirs, Lock, OpenError, Ready};
use miyu_kernel::id::AccountId;
use miyu_store::env::Env;
use miyu_store::packages::Found;
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
    // 装上时照 `MIYU_LOG`（没设、读不懂的是 INFO），读完配置再照 `log.level` 换（施工 8-2）。
    let level = miyu_log::level(std::env::var("MIYU_LOG").ok().as_deref());
    let logs = root.state().join("logs");
    let log = match miyu_log::install(&logs, "core", level.filter, env.home.as_deref()) {
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
    // 核心没了，它起的子进程跟着结束（施工 7-8，`core.md`「起来的先后」第 4 条）：Windows 上核心进作业对象，别的平台什么都
    // 不做（Unix 上每条命令的组里有看门的）。进不去照样起来。
    if let Err(error) = miyu_sandbox::lifeline::bind_children() {
        tracing::warn!(target: TARGET, error = %error, "children not bound");
    }
    if let Err(error) = root.prepare_home(&admin()) {
        return failed("home", error.to_string());
    }
    let resources = match ResourceRoot::locate(&env) {
        Ok(resources) => resources,
        Err(error) => return failed("resources", error.to_string()),
    };
    let mut found = load_packages(&resources, &root);
    let packaged = settings::Packaged::of(&mut found);
    let config = settings::read(&root, &admin(), env.home.as_deref(), &packaged);
    settings::log_level(&config, &level, &log);
    let locale = miyu_store::env::locale();
    settings::generate(
        &root,
        &resources,
        locale.as_deref(),
        &config.resolved().values(),
        &packaged,
    );
    let live = Live {
        levels: log.levels(),
        locale,
        found,
        packaged,
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(WORKERS)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return failed("runtime", error.to_string()),
    };
    let outcome = runtime.block_on(run(env, root, resources, lock, options, (config, live)));
    drop(log);
    outcome
}

/// 这一份核心编进来的内置包（施工 F-2，设计 `30-插件框架.md` 第二节）：清单是内置包、编号不在这里的，照读坏了的清单报
/// （`miyu_endpoint::packages::compiled`）。画 mermaid、联网照 cargo 开关。
pub fn built_in() -> Vec<&'static str> {
    let mut built_in = vec![miyu_tool::BASESYSTEM, miyu_memory::PACKAGE, ROLEPLAY];
    #[cfg(feature = "mermaid")]
    built_in.push("mermaid");
    #[cfg(feature = "net")]
    built_in.push("net");
    built_in
}

/// 人设防失忆提醒这个内置包的编号（施工 F-2）：代码在策略、会话两层，核心这里只认编号。
const ROLEPLAY: &str = "roleplay";

/// 工具目录：核心起来时登记一次，登记完就冻结（`05-内核接口.md` 第八节）。施工 4-4 起登记基础系统，施工 R-3 中起登记记忆
/// 这个软件包，工具的字从资源目录 `resources` 读。施工 F-2 起照清单 `found`：没装的内置包（没有读成了的清单）不登记。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对；登记时查不过（重名、名字或参数格式不合写法）。
pub fn tools(resources: &ResourceRoot, found: &[Found]) -> Result<Catalog, String> {
    let installed = |id| miyu_endpoint::packages::is_installed(found, id);
    let mut groups = Vec::new();
    if installed(miyu_tool::BASESYSTEM) {
        let basesystem =
            miyu_basesystem::tools(resources.path()).map_err(|error| error.to_string())?;
        groups.push((miyu_tool::BASESYSTEM, basesystem));
    }
    // 记忆这个软件包的三件（施工 R-3 中，`memory.md`「工具」）：工具面上只给本机的主会话。
    if installed(miyu_memory::PACKAGE) {
        let memory = miyu_memory::tools(resources.path()).map_err(|error| error.to_string())?;
        groups.push((miyu_memory::PACKAGE, memory));
    }
    // 照软件包登记（施工 P-2 中）：预设照包开关。
    Catalog::in_packages(groups).map_err(|error| error.to_string())
}

/// 读两层清单（施工 9-1 上），再标出这一份核心没编进来的内置包（施工 F-2）。必需的基础系统没装，记一行 `WARN`，照样起来。
fn load_packages(resources: &ResourceRoot, root: &DataRoot) -> Vec<Found> {
    let mut found = miyu_endpoint::packages::load(resources, root, &admin());
    miyu_endpoint::packages::compiled(&mut found, &built_in());
    if !miyu_endpoint::packages::is_installed(&found, miyu_tool::BASESYSTEM) {
        tracing::warn!(target: TARGET, package = miyu_tool::BASESYSTEM, "required package missing");
    }
    found
}

/// 运行中配置换了当场生效要的（施工 8-4）：换运行日志级别的把手，核心这边的系统语言；起来时读的软件包清单和照它拼的
/// 配置项（施工 9-1 下）：交给核心，语言换了重写生成的文件时也用。
struct Live {
    levels: miyu_log::Levels,
    locale: Option<String>,
    found: Vec<miyu_store::packages::Found>,
    packaged: settings::Packaged,
}

/// 后半段，在运行时里：在套接字上等连接，说「好了」，接连接，直到停下。`env` 是起来时读的那一份环境快照，`config` 是
/// 起来时读的配置和当场生效要的。开始监视配置文件、跟着配置换级别和重写生成的文件（施工 8-4）在说「好了」之前。
async fn run(
    env: Env,
    root: DataRoot,
    resources: ResourceRoot,
    lock: Lock,
    options: Options,
    (config, live): (miyu_endpoint::config::Config, Live),
) -> ExitCode {
    let opened = match miyu_ipc::open_locked(&root, &Dirs::current(), lock) {
        Ok(opened) => opened,
        Err(error) => return failed("socket", error.to_string()),
    };
    let routes = match models::prepare(&resources, Some(root.state().join("models"))) {
        Ok(routes) => routes,
        Err(error) => return failed("models", error),
    };
    let model_data = Arc::clone(&routes.data);
    let catalog_places = (
        resources.catalog_snapshot().parent().map(Path::to_path_buf),
        models::cache(&env),
        root.state().join("models"),
    );
    let sandbox = sandbox::probe(env.exe.as_deref());
    let sandbox_cache = sandbox::cache(&env, std::env::var_os("CARGO_HOME"));
    let embedder = embed::setup(&env, &resources);
    let tools = match tools(&resources, &live.found) {
        Ok(tools) => tools,
        Err(error) => return failed("tools", error),
    };
    let trashed = root.clone();
    let (generated, words) = (root.clone(), resources.clone());
    let queries = packages::register(&resources, &root, &admin(), &live.found);
    packages::clear_uploads(&root, &admin());
    let mut core = Core::new(
        root,
        resources,
        Arc::new(routes),
        tools,
        env.home,
        admin(),
        opened.token,
    )
    .with_sandbox(sandbox)
    .with_config(config)
    .with_packages(live.found)
    .with_model_data(Arc::clone(&model_data))
    .with_queries(queries);
    if let Some((cache, cargo_home)) = sandbox_cache {
        core = core.with_sandbox_cache(cache, cargo_home);
    }
    core = core.with_vectors(embedder);
    let core = Arc::new(core);
    tokio::spawn(settings::follow(
        core.config_now(),
        live.levels,
        generated,
        words,
        live.locale,
        Arc::new(live.packaged),
    ));
    models::follow_cooldown(core.config_now(), Arc::clone(&model_data));
    // 监视配置文件（第七条）：拿着它一直到停，丢掉就不看了。
    let _watching = core.watch_config();
    say(&Ready::Ready);
    // 照开关拉起开着的扩展（施工 9-4 上）：写了 `ready` 以后，拉起它的头不等它们。
    core.start_extensions();
    // 池的成员下架了的，路由确认以后从池里拿掉（施工 8-23）。
    core.start_retirement();
    // 写了 `ready` 以后读目录、在后台更新（施工 8-7，「起草时定的」第 13 条）。
    let (snapshot, cache, state) = catalog_places;
    models::start(
        model_data,
        snapshot.unwrap_or_default(),
        cache,
        Some(state),
        models::catalog_settings(core.config_now()),
    );
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
