//! 核心一份的模型资料（`docs/blueprint/models.md`「在哪」`route/shared.rs`、「怎么走」第二条，施工 8-7）：档案、认原厂的表、
//! 在用的目录、用出来的、供应商的列表。路由每次请求照它查资料，协议的 `model.list` 照它列模型，核心起来时读好交进来，
//! 后台拉到新目录换上。
//!
//! - 目录在写了 `ready` 以后才读完（「起草时定的」第 13 条）：要它的（造会话、载入、`model.list`）先等它（[`ModelData::wait`]）。
//!   都读不了也算读完：目录是空的。
//! - 用出来的、供应商的列表是派生数据，在 `state/models/` 下：读的时候坏了当没有（核心起来时 [`read_observed`]），变了
//!   就写（先写临时文件再改名，`miyu_store::generated`），写不成的记一行 `WARN`，内存里照样用。
//! - 池的指针（施工 8-8，`models.md` 第三条第 6 条）：一个池一个，往前走一次（[`ModelData::take`]）写一次
//!   `state/models/pools.json`（[`ModelData::save_pointers`]，在阻塞线程里，拿着指针的锁写：几次写不会把新的盖成旧的）。
//! - 冷却表（施工 8-9，`models.md` 第五条第 2 条）：一个核心一份，只在内存里，另一把锁；`[models.cooldown]` 的规矩也在
//!   这里，核心照配置的变化当场换（[`ModelData::set_cooldown_rules`]）。路由出错时记、挑端点时查，`model.list` 照它说状态。
//! - 两个 GET 的客户端：拉列表的（照环境变量的代理，施工 8-11），探本机的服务的（不走代理：代理不会自动绕过回环，施工
//!   8-11，[`ModelData::local`]）。拉某一家的列表、`provider.test` 照地址挑用哪个（[`ModelData::fetcher_for`]，施工 8-11
//!   补）：地址落在本机的也不走代理，和探本机的服务一样。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::sync::watch;

use miyu_http::{Client, is_loopback_url};

use miyu_kernel::time::Timestamp;
use miyu_models::Knowledge;
use miyu_models::catalog::Loaded;
use miyu_models::cooldown::{Cooldowns, Rules};
use miyu_models::matching::Vendors;
use miyu_models::observed::{Learned, ProviderList};
use miyu_models::pools::Pointers;
use miyu_models::profile::Profiles;

use crate::TARGET;

/// 核心一份的模型资料。
#[derive(Debug)]
pub struct ModelData {
    profiles: Profiles,
    vendors: Vendors,
    /// 在用的目录：外面一层是「读完了没有」，里面一层是「读没读成」。
    catalog: watch::Sender<Option<Option<Arc<Loaded>>>>,
    observed: Mutex<Observed>,
    /// 池的指针（施工 8-8）：另一把锁，写盘时拿着它，不挡查资料。
    pointers: Mutex<Pointers>,
    /// 冷却表和 `[models.cooldown]` 的规矩（施工 8-9）：另一把锁，只在内存里。
    cooldowns: Mutex<(Cooldowns, Rules)>,
    /// `state/models`：用出来的、供应商的列表、池的指针写在这里。没有的不写（测试里）。
    dir: Option<PathBuf>,
    /// 拉供应商的列表用的客户端（`miyu_http::fetcher`）；没有的不拉。
    fetcher: Option<Client>,
    /// 探本机的服务用的客户端（不走代理，施工 8-11）；没有的不探。
    local: Option<Client>,
}

/// 用出来的、供应商的列表、池的指针：核心起来时从 `state/models/` 读回来的。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Observed {
    /// 用出来的。
    pub learned: Learned,
    /// 供应商的列表：配好的编号 → 列表。
    pub lists: BTreeMap<String, ProviderList>,
    /// 池的指针（施工 8-8）。
    pub pointers: Pointers,
}

impl ModelData {
    /// 档案、认原厂的表读好了，目录还没读完。用出来的、供应商的列表写进 `dir`（`state/models`）。
    pub fn new(profiles: Profiles, vendors: Vendors, dir: Option<PathBuf>) -> ModelData {
        ModelData {
            profiles,
            vendors,
            catalog: watch::channel(None).0,
            observed: Mutex::new(Observed::default()),
            pointers: Mutex::new(Pointers::default()),
            cooldowns: Mutex::new((Cooldowns::default(), Rules::default())),
            dir,
            fetcher: None,
            local: None,
        }
    }

    /// 同一份，拉供应商的列表用 `client`。
    #[must_use]
    pub fn with_fetcher(mut self, client: Client) -> ModelData {
        self.fetcher = Some(client);
        self
    }

    /// 拉供应商的列表用的客户端；没有的不拉。
    pub fn fetcher(&self) -> Option<&Client> {
        self.fetcher.as_ref()
    }

    /// 照地址 `url` 挑用哪个 GET 客户端（施工 8-11 补，`http.md`「客户端」第 5 条）：回环地址不走代理，用探本机的那个
    /// （没有的退回平时那个）；别的照旧用拉列表的那个。`provider.test`、拉某一家的模型列表都照它挑：地址落在本机时也不该
    /// 被代理挡住，和探本机的服务一样。
    pub fn fetcher_for(&self, url: &str) -> Option<&Client> {
        match is_loopback_url(url) {
            true => self.local.as_ref().or(self.fetcher.as_ref()),
            false => self.fetcher.as_ref(),
        }
    }

    /// 同一份，探本机的服务用 `client`（施工 8-11）：要不走代理的那种（`miyu_http::fetcher(Proxy::Off)`）。
    #[must_use]
    pub fn with_local(mut self, client: Client) -> ModelData {
        self.local = Some(client);
        self
    }

    /// 探本机的服务用的客户端；没有的不探（`provider.detect` 的 `local` 是空的）。
    pub fn local(&self) -> Option<&Client> {
        self.local.as_ref()
    }

    /// 目录读完了（读没读成都算）：连同读好的用出来的、供应商的列表一起换上，等着的都放行。
    pub fn loaded(&self, catalog: Option<Loaded>, mut observed: Observed) {
        *self.pointers() = std::mem::take(&mut observed.pointers);
        *self.lock() = observed;
        self.catalog.send_replace(Some(catalog.map(Arc::new)));
    }

    /// 后台拉到了新目录：换上。会话下一次查资料就用它（新会话当场用，开着的会话下一个回合）。
    pub fn replace_catalog(&self, catalog: Loaded) {
        self.catalog.send_replace(Some(Some(Arc::new(catalog))));
    }

    /// 等目录读完。
    pub async fn wait(&self) {
        let mut receiver = self.catalog.subscribe();
        // 发的一方就是自己，`wait_for` 只会因为它没了出错：那时也不用等了。
        if receiver.wait_for(Option::is_some).await.is_err() {
            tracing::warn!(target: TARGET, "model data dropped while waiting");
        }
    }

    /// 在用的目录；还没读完、读不成的没有。
    pub fn catalog(&self) -> Option<Arc<Loaded>> {
        self.catalog.borrow().clone().flatten()
    }

    /// 借着手头的资料做一件事：用出来的、列表的锁拿着，做完就放开，别在里面等。
    pub fn with<R>(&self, work: impl FnOnce(&Knowledge<'_>) -> R) -> R {
        let catalog = self.catalog();
        let observed = self.lock();
        work(&Knowledge {
            profiles: &self.profiles,
            vendors: &self.vendors,
            catalog: catalog.as_deref(),
            learned: &observed.learned,
            lists: &observed.lists,
        })
    }

    /// 档案。
    pub fn profiles(&self) -> &Profiles {
        &self.profiles
    }

    /// 记下用出来的窗口：比手头的小才记，记了写盘、记一行 `INFO learned window`（第二条第 9 条）。
    pub fn learn(&self, provider: &str, model: &str, window: u64, at: Timestamp) {
        let mut observed = self.lock();
        if !observed.learned.learn(provider, model, window, at) {
            return;
        }
        tracing::info!(target: TARGET, provider, model, window, "learned window");
        let text = observed.learned.to_json();
        drop(observed);
        self.write("learned.json", &text);
    }

    /// 编号 `provider` 的列表什么时候拉的；没拉过的没有。
    pub fn list_fetched(&self, provider: &str) -> Option<Timestamp> {
        self.lock().lists.get(provider).map(|list| list.fetched)
    }

    /// 换上 `provider` 这一家新拉的列表，写盘。
    pub fn set_list(&self, provider: &str, list: ProviderList) {
        let text = list.to_json();
        self.lock().lists.insert(provider.to_string(), list);
        self.write(&format!("providers/{provider}.json"), &text);
    }

    /// 池 `pool` 有 `count` 个成员：这一次取第几个，指针往前走一个（施工 8-8，[`Pointers::take`]）。只改内存：写盘由调的一方
    /// 另在阻塞线程里叫 [`ModelData::save_pointers`]。
    pub fn take(&self, pool: &str, count: usize) -> usize {
        self.pointers().take(pool, count)
    }

    /// 把池的指针写进 `state/models/pools.json`（在阻塞线程里调）：拿着指针的锁写，写的总是这一刻最新的。
    pub fn save_pointers(&self) {
        let pointers = self.pointers();
        self.write("pools.json", &pointers.to_json());
    }

    /// 换上 `[models.cooldown]` 的规矩（施工 8-9）：核心起来时、配置换了时当场换，下一次出错用新的。
    pub fn set_cooldown_rules(&self, rules: Rules) {
        self.cooldowns().1 = rules;
    }

    /// 借着冷却表和这时的规矩做一件事（施工 8-9）：锁拿着，做完就放开，别在里面等。
    pub fn cooldown<R>(&self, work: impl FnOnce(&mut Cooldowns, &Rules) -> R) -> R {
        let mut held = self.cooldowns();
        let (table, rules) = &mut *held;
        work(table, rules)
    }

    fn cooldowns(&self) -> MutexGuard<'_, (Cooldowns, Rules)> {
        self.cooldowns
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn lock(&self) -> MutexGuard<'_, Observed> {
        self.observed.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn pointers(&self) -> MutexGuard<'_, Pointers> {
        self.pointers.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 写 `state/models/<name>`。写不成的记一行，内存里照样用：它是派生数据。
    fn write(&self, name: &str, text: &str) {
        let Some(dir) = &self.dir else {
            return;
        };
        let path = name
            .split('/')
            .fold(dir.clone(), |path, part| path.join(part));
        if let Err(error) = miyu_store::generated::write(&path, text.as_bytes()) {
            tracing::warn!(target: TARGET, file = name, error = %error, "model data not written");
        }
    }
}

/// 读 `state/models/` 下的用出来的、供应商的列表、池的指针（在阻塞线程里调）：没有的、读不了的、坏的当没有，坏的记一行
/// `WARN`。
pub fn read_observed(dir: &Path) -> Observed {
    let read = |path: &Path| std::fs::read_to_string(path).ok();
    let learned = read(&dir.join("learned.json"))
        .and_then(|text| warn_if_broken(Learned::parse(&text)))
        .unwrap_or_default();
    let mut lists = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(dir.join("providers")) {
        for entry in entries.flatten() {
            let path = entry.path();
            let id = path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_suffix(".json"));
            let (Some(id), Some(text)) = (id, read(&path)) else {
                continue;
            };
            if let Some(list) = warn_if_broken(ProviderList::parse(&text)) {
                lists.insert(id.to_string(), list);
            }
        }
    }
    let pointers = read(&dir.join("pools.json"))
        .and_then(|text| warn_if_broken(Pointers::parse(&text)))
        .unwrap_or_default();
    Observed {
        learned,
        lists,
        pointers,
    }
}

/// 坏了的记一行，当没有。
fn warn_if_broken<T>(read: Result<T, String>) -> Option<T> {
    read.map_err(|error| tracing::warn!(target: TARGET, error = %error, "model data unreadable"))
        .ok()
}
