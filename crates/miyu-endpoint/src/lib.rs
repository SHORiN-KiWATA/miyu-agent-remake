//! 协议端点（`docs/designs/04-核心协议.md`，施工 3-8 上）：头和核心之间说的话。
//!
//! 一个连接上说 JSON-RPC 2.0，一行一条消息。先握手：协议的主版本，本机令牌；之后能造会话、说话（可以带附件，
//! 施工 3-9 三补）、打断，订阅会话的事件流（施工 3-8 中）。会话表照编号找会话，这次运行里没在跑的，从磁盘载入。只认字节流，不管它从哪来：本机套接字
//! （施工 3-8 下）、命名管道、以后的 WebSocket，都把连接交给 [`serve`]。
//!
//! - [`Core`]：核心的家底：数据根、资源目录、给会话造请求模型的端口、管理员、本机令牌、会话表；
//! - [`serve`]：和一个连接说话，直到它关了；
//! - [`run`]：在本机的监听器上一个个接连接，每个交给 [`serve`]；
//! - [`Core::idle`]：没有连接、没有在跑的回合、也没有在跑的后台命令，核心据此空闲退出（施工 3-9 上、7-3）；
//! - [`settings`]：端点的配置项，界面语言 `ui.language`（施工 8-1）、新会话开局只读 `permission.start_read_only`
//!   （施工 8-2）；
//! - [`config`]：配置服务：起来时读的几份配置、最终值，`config.schema`、`config.get`、`config.check`（施工 8-2）；
//!   `config.set`、`config.trust`（施工 8-3）；监视配置文件、推 `config.changed`、把当前的一份交给会话和核心（施工 8-4）；
//! - 密钥：`secret.set`、`secret.delete`、`secret.list`，只能写、删、列名字，从不交出值（施工 8-5，`secrets.rs`）；
//! - 模型：`model.list`，配好的供应商、模型、每一格资料的值和来源（施工 8-7，`models.rs`）；第一次接入的
//!   `provider.detect`、`provider.catalog`、`provider.test`（施工 8-11，`providers.rs`）；
//! - 给人看的字：`human.get`，工具的样子、说法的模板原文，头不用再自己去资源目录里读（施工 W-1，`human.rs`）。
//! - 文件：`fs.list` 列一层目录，`fs.find` 模糊找文件，数据根只有账号自己的工作区能列、能找（施工 W-2，`files.rs`）。
//! - 可选软件包登记的查询：方法名到怎么答的一张表（[`queries`]，施工 W-4）。核心起来时照编进来的包
//!   （`miyu-core` 的 cargo 开关）往里登记，`mermaid.render` 就是这样接进来的；没编进来的方法，这张表里
//!   压根没有它，握手以后的方法里找不到、这张表里也找不到的，一律 `unknown_method`。
//! - 分块上传：`blob.open`、`blob.write`、`blob.close`，跟着连接走，收齐了存成 blob，回应和 `blob.put` 一样
//!   （施工 W-5，`uploads.rs`）。连接断了、60 秒没写都作废。
//! - [`extensions`]：核心拉起的 `process` 包，经标准输入输出说同一套协议；开关、退避重启、随核心退出（施工 9-4 上）。

mod appending;
mod attach;
mod backstage;
pub mod builtins;
mod check;
mod commands;
pub mod config;
mod connection;
mod diffs;
pub mod extensions;
mod files;
mod from;
mod hello;
mod human;
mod job_output;
mod list;
mod listen;
mod listing;
mod login;
mod memory;
mod meta;
mod methods;
mod models;
pub mod packages;
mod personas;
mod presets;
mod provide;
mod providers;
pub mod queries;
mod refusal;
mod responding;
mod reverse;
mod secrets;
mod sessions;
pub mod settings;
mod spawn;
mod subscriptions;
mod system_accounts;
#[cfg(test)]
mod test_support;
mod toml_changes;
mod undo;
mod uploads;
mod usage;
mod venues;
mod view;
mod wire;
mod workspace;

pub use connection::serve;
pub use listen::run;

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use miyu_kernel::id::AccountId;
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_policy::preset::MEMORY;
use miyu_sandbox::{Availability, Unusable};
use miyu_session::{Jobs, Memory, ModelData, Models, Observed, SandboxCache, SummaryTexts};
use miyu_store::index::SessionIndex;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_store::usage::UsageIndex;
use miyu_tool::{Catalog, Shelf};

use config::Config;
use config::hub::Hub;
use queries::Queries;
use sessions::Sessions;

/// 核心的家底：一个核心一份，各个连接一起用。
pub struct Core {
    /// 数据根。
    root: DataRoot,
    /// 资源目录：造会话时读人格。
    resources: ResourceRoot,
    /// 给会话造请求模型的端口。
    models: Arc<dyn Models>,
    /// 工具目录的架子：造会话时照现在的那一份存下工具面（施工 4-1）。提供者登记了换一代（施工 O-2 上），会话们拿着同一个
    /// 架子，下一个回合换上（施工 O-2 中）。
    tools: Shelf,
    /// 哪个包现在由哪个连接提供工具（施工 O-2 上，`provide.rs`）。
    provided: Arc<provide::Provided>,
    /// 后台页的方法：哪个包登记了哪些、由哪个连接答（施工 F-6 中，`backstage.rs`）。
    backstage: backstage::Methods,
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
    /// 管理员的会话列表的索引（施工 3-8 七补）：起来时开一次，一直开着；会话落盘时更新、删会话时删行、列会话时读。
    index: Arc<SessionIndex>,
    /// 系统账号各自的会话列表的索引（施工 O-4 下）：在它们自己的家目录下，拉起扩展前开（`system_accounts::prepare`）；
    /// 装上新的跟着开，卸掉的留着、装回来接着用（施工 F-5 下）。
    system_indexes: std::sync::Mutex<std::collections::BTreeMap<AccountId, Arc<SessionIndex>>>,
    /// 用量汇总（施工 8-15，`state/usage.db`）：起来时开一次，一直开着；会话落盘时写、一次性入口记账（交给模型资料）、
    /// `usage.query` 和 `session_usage` 查之前补。
    usage: Arc<UsageIndex>,
    /// 记忆（施工 R-2 上、R-3 中，`memory.md`）：回合库的登记、记忆日志的登记，起来时各建一份空的，用到哪个人格的才开；
    /// 会话落盘时更新回合库、删会话时拿掉，三件工具经它记、忘、搜。
    memory: Arc<Memory>,
    /// 执行器的任务表（施工 7-3）：所有会话的后台命令，核心里一张。
    jobs: Arc<Jobs>,
    /// 连着几个连接：`serve` 开始时加一，走的时候减一（施工 3-9 上）。
    connections: AtomicUsize,
    /// 连上以后最多等多久握手（施工 4-9 再补三上）：等不来就断开，不然一个连上不说话的本机进程能让核心一直
    /// 不空闲退出。
    hello_wait: Duration,
    /// 配置（施工 8-2）：起来时读的几份和最终值。施工 8-3 起能改，住在一把锁里（[`Core::config`]）。
    config: std::sync::Mutex<Config>,
    /// 配置换了交给谁（施工 8-4）：会话、核心取当前的一份，订阅着配置的连接收推送。
    hub: Hub,
    /// 核心一份的模型资料（施工 8-7）：`model.list` 照它列。路由手里是同一份。
    model_data: Arc<ModelData>,
    /// 找文件的清单记几份（施工 W-2，`files.rs`）：各个连接共用。
    files: files::Cache,
    /// `fresh` 时，清单建好多久以上才重建（施工 W-2）：出厂值 [`miyu_fs::FRESH_SECS`]，测试里设短的，不用真等
    /// 十秒。
    files_fresh: Duration,
    /// 可选软件包登记的查询（施工 W-4）：核心起来时照编进来的包往里登记，空表就是没编进来任何一个。
    queries: Queries,
    /// 分块上传（施工 W-5）：这个连接上的一个上传多久没有 `blob.write` 就作废。出厂 60 秒，测试里设短的，
    /// 不用真等一分钟。
    upload_idle: Duration,
    /// 身份（施工 W-8）：一次性码、登录失败的计数、作废登录令牌的广播。
    identity: login::Identity,
    /// 会话列表的推送（施工 9-5）：排队算一项、广播给订阅着的连接。
    listing: listing::Listing,
    /// 找回、造场所会话排着来（施工 O-3）：同一个场所同时来两次，不造出两个主线会话。
    venues: tokio::sync::Mutex<()>,
    /// 软件包清单（施工 9-1 上，`packages.rs`）：起来时读一次；装、卸以后当场换成重读的一份（施工 F-5 上）。
    packages: std::sync::RwLock<Arc<Vec<miyu_store::packages::Found>>>,
    /// 这一份核心编进来的内置包（施工 F-2、F-5 上）：装卸以后重读清单时照它标没编进来的。没设的不标（测试里造的核心）。
    built_in: Option<Vec<&'static str>>,
    /// 装、卸一次只做一件（施工 F-5 上）。
    packaging: tokio::sync::Mutex<()>,
    /// 内置包的工具从哪来（施工 F-5 中）：装卸以后照它换工具目录；没设的不换。
    builtins: Option<Arc<dyn builtins::Builtins>>,
    /// 扩展进程（施工 9-4 上，`extensions.rs`）：核心拉起的 `process` 包。
    extensions: extensions::Extensions,
    /// 软件包列表里哪个包变了（施工 F-8 三补，`packages/changes.rs`）。
    package_changes: packages::changes::Changes,
}

/// 空的模型资料：没有档案、没有目录，读完了。
fn empty_model_data() -> Arc<ModelData> {
    let data = ModelData::new(Profiles::default(), Vendors::default(), None);
    data.loaded(None, Observed::default());
    Arc::new(data)
}

/// 连上以后最多等多久握手。
const HELLO_WAIT: Duration = Duration::from_secs(10);

/// 分块上传多久没写就作废（施工 W-5，`web-module.md`「怎么走」第六条第 4 款）。
const UPLOAD_IDLE: Duration = Duration::from_secs(60);

impl Core {
    /// 现在的工具目录（施工 O-2 上：提供者登记了会换）。
    pub(crate) fn tools(&self) -> Catalog {
        self.tools.current()
    }
}

impl Core {
    /// 一份家底：会话表是空的，会话用到时再载入；打开管理员的会话列表的索引（施工 3-8 七补），坏了的删掉重建。
    pub fn new(
        root: DataRoot,
        resources: ResourceRoot,
        models: Arc<dyn Models>,
        tools: Catalog,
        home: Option<PathBuf>,
        admin: AccountId,
        token: String,
    ) -> Core {
        let items = [
            settings::UiSettings::ITEMS,
            settings::PersonaSettings::ITEMS,
            settings::PresetSettings::ITEMS,
            settings::PermissionSettings::ITEMS,
            settings::EXTERNAL_BINDINGS,
        ]
        .concat();
        let mut config = Config::defaults(&root, &admin, items);
        let model_data = empty_model_data();
        config.set_models(Arc::clone(&model_data));
        let index = Arc::new(list::open_index(&root, &admin));
        let found = packages::load(&resources, &root, &admin);
        let usage = Arc::new(usage::open(&root));
        model_data.keep_ledger(Arc::clone(&usage));
        // 常驻的摘要那一块的字（施工 R-4 上）：读不出来的（安装坏了）这个核心不交摘要，别的照常。
        let summary = SummaryTexts::load(resources.path())
            .inspect_err(|error| tracing::warn!(target: "miyu::endpoint", error = %error, "memory summary texts unreadable"))
            .ok();
        let memory = Memory::new(&root, summary);
        memory::give_recall(&memory, &resources);
        // 人格记忆装没装（施工 R-10）：照这时读到的清单；交进来另一份的照那一份（[`Core::with_packages`]）。
        memory.set_installed(packages::is_installed(&found, MEMORY));
        // 抽取（施工 R-6 上）：照一次性入口发；字、key 的写法读不出来的，这个核心不抽。
        if let Some(extraction) = memory::extraction(&*models, &resources, &root, &admin) {
            memory.give_extraction(extraction);
        }
        Core {
            memory,
            index,
            system_indexes: std::sync::Mutex::default(),
            usage,
            hub: Hub::new(&config),
            config: std::sync::Mutex::new(config),
            root,
            resources,
            models,
            tools: Shelf::new(tools),
            provided: Arc::default(),
            backstage: backstage::Methods::default(),
            home,
            sandbox: Availability::Unusable(Unusable::HelperMissing),
            sandbox_cache: None,
            admin,
            token,
            sessions: Sessions::default(),
            jobs: Arc::new(Jobs::new()),
            connections: AtomicUsize::new(0),
            hello_wait: HELLO_WAIT,
            model_data,
            files: files::Cache::default(),
            files_fresh: Duration::from_secs(miyu_fs::FRESH_SECS),
            queries: Queries::default(),
            upload_idle: UPLOAD_IDLE,
            identity: login::Identity::new(login::CODE_TTL),
            listing: listing::Listing::default(),
            venues: tokio::sync::Mutex::new(()),
            packages: std::sync::RwLock::new(Arc::new(found)),
            built_in: None,
            packaging: tokio::sync::Mutex::new(()),
            builtins: None,
            extensions: extensions::Extensions::new(extensions::Timing::default()),
            package_changes: packages::changes::Changes::default(),
        }
    }

    /// 同一份家底，模型资料照 `data`（施工 8-7）：核心起来时把路由手里的那一份交进来。没设的是空的：没有档案、没有目录。
    /// 配置服务也拿着它：查模型默认的思考强度在不在档位里（施工 8-18）。要在 [`Core::with_vectors`] 前面接：那一路照接上
    /// 时手里的这一份查供应商、记账，接反了会悄悄拿到空的那一份（调试构建里当场 panic，测试、真核心的测试都逮得住）。
    #[must_use]
    pub fn with_model_data(mut self, data: Arc<ModelData>) -> Core {
        debug_assert!(
            self.memory.vectors().is_none(),
            "with_model_data 要接在 with_vectors 前面"
        );
        let config = self
            .config
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        config.set_models(Arc::clone(&data));
        self.hub = Hub::new(config);
        // 一次性入口记账（施工 8-15）：和会话写的是同一份汇总。
        data.keep_ledger(Arc::clone(&self.usage));
        self.model_data = data;
        self
    }

    /// 同一份家底，连上以后最多等 `wait` 握手：测试里设短的，不用真等 10 秒。
    #[must_use]
    pub fn with_hello_wait(mut self, wait: Duration) -> Core {
        self.hello_wait = wait;
        self
    }

    /// 同一份家底，`fs.find` 的 `fresh` 照 `fresh` 这个时长判断要不要重建清单（施工 W-2）：测试里设短的，不用
    /// 真等十秒。
    #[must_use]
    pub fn with_files_fresh(mut self, fresh: Duration) -> Core {
        self.files_fresh = fresh;
        self
    }

    /// 同一份家底，可选软件包登记的查询照 `queries`（施工 W-4）：核心起来时照编进来的包往里登记一份，这里
    /// 整个换上。没设的是空表，没有任何可选软件包的方法。
    #[must_use]
    pub fn with_queries(mut self, queries: Queries) -> Core {
        self.queries = queries;
        self
    }

    /// 同一份家底，分块上传多久没写就作废照 `idle`（施工 W-5）：测试里设短的，不用真等 60 秒。
    #[must_use]
    pub fn with_upload_idle(mut self, idle: Duration) -> Core {
        self.upload_idle = idle;
        self
    }

    /// 同一份家底，一次性码 `ttl` 有效（施工 W-8）：测试里设短的，不用真等 5 分钟。
    #[must_use]
    pub fn with_code_ttl(mut self, ttl: Duration) -> Core {
        self.identity = login::Identity::new(ttl);
        self
    }

    /// 同一份家底，软件包清单照 `packages`（施工 9-1 下）：核心起来时读好、照核心自己的模块认过撞没撞（`packages::settle`）
    /// 交进来，`package.list`、包的配置项的字照它。没设的是 `Core::new` 自己读的那一份。
    #[must_use]
    pub fn with_packages(mut self, packages: Vec<miyu_store::packages::Found>) -> Core {
        self.memory
            .set_installed(packages::is_installed(&packages, MEMORY));
        self.packages = std::sync::RwLock::new(Arc::new(packages));
        self
    }

    /// 同一份家底，这一份核心编进来的内置包照 `built_in`（施工 F-5 上）：装卸以后重读清单时照它标没编进来的。
    #[must_use]
    pub fn with_built_in(mut self, built_in: Vec<&'static str>) -> Core {
        self.built_in = Some(built_in);
        self
    }

    /// 同一份家底，内置包的工具照 `builtins` 要（施工 F-5 中）：装卸以后当场换进工具目录。
    #[must_use]
    pub fn with_builtins(mut self, builtins: Arc<dyn builtins::Builtins>) -> Core {
        self.builtins = Some(builtins);
        self
    }

    /// 这时的软件包清单（施工 F-5 上：装卸以后当场换）。
    pub fn packages(&self) -> Arc<Vec<miyu_store::packages::Found>> {
        Arc::clone(
            &self
                .packages
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    /// 同一份家底，扩展进程等多久、退避多久照 `timing`（施工 9-4 上）：测试里设短的，不用真等。
    #[must_use]
    pub fn with_extension_timing(mut self, timing: extensions::Timing) -> Core {
        self.extensions = extensions::Extensions::new(timing);
        self
    }

    /// 同一份家底，配置照 `config`（施工 8-2）：核心起来时读好交进来。没设的全是默认值，只认端点自己的两项。
    #[must_use]
    pub fn with_config(mut self, mut config: Config) -> Core {
        config.set_models(Arc::clone(&self.model_data));
        self.hub = Hub::new(&config);
        self.config = std::sync::Mutex::new(config);
        self
    }

    /// 当前的配置（施工 8-4）：配置服务每换上一份新的（`config.set`、手改被看到的、`config.trust`），这里就是新的一份。核心
    /// 照它当场换运行日志的级别、重写生成的文件。
    pub fn config_now(&self) -> tokio::sync::watch::Receiver<Arc<Config>> {
        self.hub.current()
    }

    /// 配置服务（施工 8-3）：改、查排着队一件件办（`config.md` 第五条第 1 条）。拿着它的时候不许 `.await`：别的连接的
    /// 查询会一直等着。上一个拿着它的出了 bug、崩了的，照样拿：配置服务改到一半不会留下半截（先写好文件才换上）。
    pub(crate) fn config(&self) -> std::sync::MutexGuard<'_, Config> {
        self.config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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

    /// 账号 `owner` 的会话列表的索引，交给造的、载入的会话（施工 3-8 七补）：管理员的，和系统账号各自的（施工 O-4 下，在它
    /// 自己的家目录下）；别的账号的、还没开的没有，列会话时照目录读。
    pub(crate) fn index_for(&self, owner: &AccountId) -> Option<Arc<SessionIndex>> {
        if *owner == self.admin {
            return Some(Arc::clone(&self.index));
        }
        self.system_indexes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(owner)
            .map(Arc::clone)
    }

    /// 账号 `owner` 的会话写哪份用量汇总（施工 8-15）：核心一份、行里带属主，管理员和系统账号的会话写（施工 O-4 下）。
    pub(crate) fn usage_for(&self, owner: &AccountId) -> Option<Arc<UsageIndex>> {
        self.knows(owner).then(|| Arc::clone(&self.usage))
    }

    /// 连着几个连接。
    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::Acquire)
    }

    /// 空闲：没有连接，没有在跑的回合，也没有在跑的后台命令（施工 7-3），也没有开着、没停下的扩展（施工 9-4 上）。核心看它决定能不能空闲退出
    /// （`12-进程形态与分发.md` 第二节、R1，施工 3-9 上）。
    pub async fn idle(&self) -> bool {
        self.connections() == 0
            && !self.jobs.running()
            && !self.extensions.busy()
            && !self.sessions.busy().await
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
