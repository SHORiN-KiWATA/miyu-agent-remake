//! 协议端点（`docs/designs/04-核心协议.md`，施工 3-8 上）：头和核心之间说的话。
//!
//! 一个连接上说 JSON-RPC 2.0，一行一条消息。先握手：协议的主版本，本机令牌；之后能造会话、说话（可以带附件，
//! 施工 3-9 三补）、打断，订阅会话的事件流（施工 3-8 中）。会话表照编号找会话，这次运行里没在跑的，从磁盘载入。只认字节流，不管它从哪来：本机套接字
//! （施工 3-8 下）、命名管道、以后的 WebSocket，都把连接交给 [`serve`]。
//!
//! - [`Core`]：核心的家底：数据根、资源目录、给会话造请求模型的端口、管理员、本机令牌、会话表；
//! - [`serve`]：和一个连接说话，直到它关了；
//! - [`run`]：在本机的监听器上一个个接连接，每个交给 [`serve`]；
//! - [`Core::idle`]：没有连接、没有在跑的回合、也没有在跑的后台命令，核心据此空闲退出（施工 3-9 上、7-3）。

mod attach;
mod connection;
mod hello;
mod list;
mod listen;
mod meta;
mod methods;
mod refusal;
mod sessions;
mod spawn;
mod subscriptions;
mod undo;
mod wire;

pub use connection::serve;
pub use listen::run;

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use miyu_kernel::id::AccountId;
use miyu_sandbox::{Availability, Unusable};
use miyu_session::{Jobs, Models, SandboxCache};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_tool::Catalog;

use sessions::Sessions;

/// 核心的家底：一个核心一份，各个连接一起用。
pub struct Core {
    /// 数据根。
    root: DataRoot,
    /// 资源目录：造会话时读人格。
    resources: ResourceRoot,
    /// 给会话造请求模型的端口。
    models: Arc<dyn Models>,
    /// 工具目录：造会话时照它存下工具面（施工 4-1）。
    tools: Catalog,
    /// 系统的家目录：权限策略照它换 `~`，头报来的工作目录是它的就退回管理员的工作区（施工 4-3 下）。
    home: Option<PathBuf>,
    /// 这台机器上的沙盒能不能用（核心起来时探的）：握手时报给头（施工 5-4 下）；能用的，造会话、载入时把助手交给会话
    /// （施工 5-4 上）。
    sandbox: Availability,
    /// 沙盒的缓存放在哪：`<缓存目录>/sandbox`，各账号一份在它下面；你的 cargo 目录在哪（施工 5-4 下）。算不出缓存目录的
    /// 没有。
    sandbox_cache: Option<(PathBuf, Option<PathBuf>)>,
    /// 管理员：本机连上来的都是他（`06-多用户与身份.md` 第二节）。
    admin: AccountId,
    /// 本机令牌：本机连接握手时要出示（`04-核心协议.md` 第四节）。
    token: String,
    /// 会话表。
    sessions: Sessions,
    /// 执行器的任务表（施工 7-3）：所有会话的后台命令，核心里一张。
    jobs: Arc<Jobs>,
    /// 连着几个连接：`serve` 开始时加一，走的时候减一（施工 3-9 上）。
    connections: AtomicUsize,
    /// 连上以后最多等多久握手（施工 4-9 再补三上）：等不来就断开，不然一个连上不说话的本机进程能让核心一直
    /// 不空闲退出。
    hello_wait: Duration,
}

/// 连上以后最多等多久握手。
const HELLO_WAIT: Duration = Duration::from_secs(10);

impl Core {
    /// 一份家底：会话表是空的，会话用到时再载入。
    pub fn new(
        root: DataRoot,
        resources: ResourceRoot,
        models: Arc<dyn Models>,
        tools: Catalog,
        home: Option<PathBuf>,
        admin: AccountId,
        token: String,
    ) -> Core {
        Core {
            root,
            resources,
            models,
            tools,
            home,
            sandbox: Availability::Unusable(Unusable::HelperMissing),
            sandbox_cache: None,
            admin,
            token,
            sessions: Sessions::default(),
            jobs: Arc::new(Jobs::new()),
            connections: AtomicUsize::new(0),
            hello_wait: HELLO_WAIT,
        }
    }

    /// 同一份家底，连上以后最多等 `wait` 握手：测试里设短的，不用真等 10 秒。
    #[must_use]
    pub fn with_hello_wait(mut self, wait: Duration) -> Core {
        self.hello_wait = wait;
        self
    }

    /// 同一份家底，这台机器上的沙盒照 `sandbox`（施工 5-4 上、下）。用不了的，会话里工作区、只读两级执行命令都要问人；
    /// 没设的当找不到助手。
    #[must_use]
    pub fn with_sandbox(mut self, sandbox: Availability) -> Core {
        self.sandbox = sandbox;
        self
    }

    /// 同一份家底，沙盒的缓存放在 `root` 下面、各账号一份，你的 cargo 目录是 `cargo_home`（施工 5-4 下）。
    #[must_use]
    pub fn with_sandbox_cache(mut self, root: PathBuf, cargo_home: Option<PathBuf>) -> Core {
        self.sandbox_cache = Some((root, cargo_home));
        self
    }

    /// 账号 `owner` 的那一份沙盒的缓存。
    pub(crate) fn sandbox_cache_of(&self, owner: &AccountId) -> Option<SandboxCache> {
        self.sandbox_cache
            .as_ref()
            .map(|(root, cargo_home)| SandboxCache {
                dir: root.join(owner.as_str()),
                cargo_home: cargo_home.clone(),
            })
    }

    /// 连着几个连接。
    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::Acquire)
    }

    /// 空闲：没有连接，没有在跑的回合，也没有在跑的后台命令（施工 7-3）。核心看它决定能不能空闲退出
    /// （`12-进程形态与分发.md` 第二节、R1，施工 3-9 上）。
    pub async fn idle(&self) -> bool {
        self.connections() == 0 && !self.jobs.running() && !self.sessions.busy().await
    }

    /// 有计划地停下全部在跑的会话：核心收到停的信号时。跑到一半的回合记成「重启了」，下次载入接着干。
    pub async fn stop_sessions(&self) {
        self.sessions.stop_all().await;
    }
}

/// 连着的一个连接：数着，走的时候减掉，连接的任务被叫停了也减。
pub(crate) struct Connected(Arc<Core>);

impl Connected {
    /// 数上一个。
    pub(crate) fn new(core: Arc<Core>) -> Connected {
        core.connections.fetch_add(1, Ordering::AcqRel);
        Connected(core)
    }
}

impl Drop for Connected {
    fn drop(&mut self) {
        self.0.connections.fetch_sub(1, Ordering::AcqRel);
    }
}

/// 令牌不打出来。
impl fmt::Debug for Core {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Core")
            .field("root", &self.root)
            .field("admin", &self.admin)
            .finish_non_exhaustive()
    }
}
