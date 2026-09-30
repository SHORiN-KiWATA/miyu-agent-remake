//! 配置服务（`docs/blueprint/config.md`「怎么走」第二、三、五、六条，施工 8-2、8-3）：核心起来时读系统配置、管理员的
//! 个人设置和信任的记录，合出不算项目配置的最终值；造会话、说话、`config.get` 时照目录找项目配置，带上信任着的那一份
//! 再合一次。
//!
//! - [`Config::load`]：起来时读一次。读不进来不影响起不起得来（G8）：问题记下，那一层照空的算，每份有问题的文件记一条
//!   `WARN config problems`。
//! - [`Config::resolved`]：不算项目配置的最终值；`Config::with_project`：照一个目录带上项目配置。
//! - 协议上的 `config.schema`、`config.get`、`config.check` 在 `config/methods.rs`，`config.set` 在 `config/set.rs`，
//!   `config.trust` 在 `config/trusting.rs`（施工 8-3），写成 JSON 的几样在 `config/wire.rs`，留痕在 `config/journal.rs`。
//!
//! 只有核心写配置文件（G4），核心里只有这一个配置服务：它住在一把锁里，改、查排着队一件件办。改之前先把文件重读一遍，
//! 手改过的照新的字改（G5 第 4 条）；写成了换上新的最终值，新的连接、新的会话照它。监视、当场推给头随 8-4。

mod file;
mod journal;
pub(crate) mod methods;
mod project;
pub(crate) mod set;
mod trust;
pub(crate) mod trusting;
mod wire;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use miyu_config::merge::{Layers, Resolved, Trust, merge};
use miyu_config::{Item, Layer};
use miyu_kernel::id::AccountId;
use miyu_store::root::DataRoot;

use file::File;

/// 运行日志的目标（`config.md`「出错」）。
const TARGET: &str = "miyu::config";

/// 系统配置在数据根里的位置。
const SYSTEM: [&str; 2] = ["system", "config.toml"];

/// 个人设置的文件名：在账号的家目录里。
const PERSONAL: &str = "settings.toml";

/// 核心新建系统配置、个人设置时第一行指向的 Schema，相对这份文件（第五条第 2 条第 6 款）。
const SYSTEM_SCHEMA: &str = "../state/config/config.schema.json";
const PERSONAL_SCHEMA: &str = "../../state/config/settings.schema.json";

/// 手里的配置。
#[derive(Debug, Clone)]
pub struct Config {
    /// 登记的全部配置项。
    items: Vec<Item>,
    /// 系统的家目录（真实的位置）：项目配置往上找到它就停，写成 `~/…`。
    home: Option<PathBuf>,
    /// 数据根（真实的位置）：落在它里面的目录不找项目配置。
    data_root: PathBuf,
    /// 起来时的环境变量：只有带 `env` 的项的，名字到值。
    env: BTreeMap<&'static str, String>,
    /// 系统配置。
    system: File,
    /// 管理员的个人设置。
    personal: File,
    /// 信任的记录。
    trust: Vec<trust::Record>,
    /// 几份核心自己写的文件在哪（施工 8-3）。
    places: Places,
    /// 不算项目配置的最终值。
    resolved: Resolved,
}

/// 能改的两层的文件：哪一层、在哪、给人看的写法（数据根里的写成相对数据根的）。
fn layers(root: &DataRoot, account: &AccountId) -> [(Layer, PathBuf, String); 2] {
    let system = SYSTEM
        .iter()
        .fold(root.path().to_path_buf(), |p, s| p.join(s));
    [
        (Layer::System, system, SYSTEM.join("/")),
        (
            Layer::Personal,
            root.account_dir(account).join(PERSONAL),
            format!("home/{}/{PERSONAL}", account.as_str()),
        ),
    ]
}

/// 核心自己写的几份文件在哪：信任的记录、系统日志、账号日志（施工 8-3）。
#[derive(Debug, Clone)]
struct Places {
    /// 账号：日志里记是谁改的。
    account: AccountId,
    /// `home/<账号>/trust.toml`。
    trust: PathBuf,
    /// 系统日志 `system/journal.jsonl`。
    system_journal: PathBuf,
    /// 账号日志 `home/<账号>/journal.jsonl`。
    account_journal: PathBuf,
}

impl Places {
    fn of(root: &DataRoot, account: &AccountId) -> Places {
        let home = root.account_dir(account);
        Places {
            account: account.clone(),
            trust: home.join(trust::FILE),
            system_journal: root.system().join(miyu_store::journal::FILE),
            account_journal: home.join(miyu_store::journal::FILE),
        }
    }

    /// 给人看的写法：相对数据根。
    fn shown(&self, file: &str) -> String {
        format!("home/{}/{file}", self.account.as_str())
    }
}

/// 一个目录找到的项目配置。
#[derive(Debug, Clone)]
pub(crate) struct Project {
    /// 仓库在哪：`.miyu` 所在的那一层，真实的位置（施工 8-3：信任记的是它）。
    pub(crate) repo: PathBuf,
    /// 读好的那一份。
    pub(crate) file: File,
    /// 信不信任。
    pub(crate) trust: Trust,
}

impl Project {
    /// 还没问过信不信任的（没有记录、内容变了）：它在哪。信任着的、选了不信任的是空的。
    pub(crate) fn untrusted(self) -> Option<String> {
        (self.trust == Trust::Unknown).then_some(self.file.shown)
    }
}

impl Config {
    /// 起来时读：数据根 `root` 里的系统配置、账号 `account` 的个人设置和信任的记录，照清单 `items` 认，带 `env` 的项照
    /// `env` 读环境变量（只读这一次）。`home` 是系统的家目录。
    pub fn load(
        root: &DataRoot,
        account: &AccountId,
        home: Option<&Path>,
        items: Vec<Item>,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Config {
        let env: BTreeMap<&'static str, String> = items
            .iter()
            .filter_map(|item| item.env)
            .filter_map(|name| env(name).map(|value| (name, value)))
            .collect();
        let [system, personal] = layers(root, account)
            .map(|(layer, path, shown)| File::read(&items, layer, path, shown));
        let trust_path = root.account_dir(account).join(trust::FILE);
        let trust = trust::read(&trust_path).unwrap_or_else(|error| {
            tracing::warn!(
                target: TARGET,
                file = %format!("home/{}/{}", account.as_str(), trust::FILE),
                error = %error,
                "trust not read"
            );
            Vec::new()
        });
        for file in [&system, &personal] {
            let (errors, warnings) = file.counts();
            if errors + warnings > 0 {
                tracing::warn!(target: TARGET, file = %file.shown, errors, warnings, "config problems");
            }
        }
        Config::assemble(root, account, home, items, env, [system, personal], trust)
    }

    /// 什么都没读：全是默认值（核心不给配置的时候，例如协议端点的测试）。
    pub fn defaults(root: &DataRoot, account: &AccountId, items: Vec<Item>) -> Config {
        let files =
            layers(root, account).map(|(layer, path, shown)| File::nothing(layer, &path, &shown));
        Config::assemble(
            root,
            account,
            None,
            items,
            BTreeMap::new(),
            files,
            Vec::new(),
        )
    }

    /// 起来时读的、默认的两条路共用：`files` 是系统配置、个人设置。
    fn assemble(
        root: &DataRoot,
        account: &AccountId,
        home: Option<&Path>,
        items: Vec<Item>,
        env: BTreeMap<&'static str, String>,
        [system, personal]: [File; 2],
        trust: Vec<trust::Record>,
    ) -> Config {
        let real = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let mut config = Config {
            home: home.map(real),
            data_root: real(root.path()),
            items,
            env,
            system,
            personal,
            trust,
            places: Places::of(root, account),
            resolved: Resolved::default(),
        };
        config.resolved = config.merged(None);
        config
    }

    /// 登记的全部配置项。
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// 不算项目配置的最终值。
    pub fn resolved(&self) -> &Resolved {
        &self.resolved
    }

    /// 系统配置、个人设置里现在有几处错误（不算警告）：握手的 `config_errors`。
    pub fn errors(&self) -> usize {
        self.system.counts().0 + self.personal.counts().0
    }

    /// 带上目录 `dir`（头报的写法，`~` 照家目录换）的项目配置合出来的最终值；项目配置没有、没信任的，和
    /// [`Config::resolved`] 一样。交回找到的项目配置。
    pub(crate) fn with_project(&self, dir: &str) -> (Resolved, Option<Project>) {
        let project = self.project(dir);
        (self.merged(project.as_ref()), project)
    }

    /// 目录 `dir` 的项目配置：换成真实的位置往上找（第三条第 1 条），读、认信不信任。换不成、找不到的是空的。
    pub(crate) fn project(&self, dir: &str) -> Option<Project> {
        let start = miyu_fs::resolve(Path::new("/"), self.home.as_deref(), dir).ok()?;
        let repo = project::find(&start, self.home.as_deref(), &self.data_root)?;
        let path = project::config_in(&repo);
        let shown = project::shown(&path, self.home.as_deref());
        let file = File::read(&self.items, Layer::Project, path, shown);
        let trust = match &file.version {
            Some(version) => trust::trust_of(&self.trust, &repo, version, self.home.as_deref()),
            None => Trust::Unknown,
        };
        Some(Project { repo, file, trust })
    }

    /// 目录 `dir` 的项目配置还没问过信不信任的：它在哪（`session.create`、`session.send` 回应的 `untrusted_project`）。
    pub(crate) fn untrusted(&self, dir: &str) -> Option<String> {
        self.project(dir).and_then(|project| project.untrusted())
    }

    /// 合一次：默认值、系统配置、个人设置，再加上 `project`（有的话），最后是环境变量。
    fn merged(&self, project: Option<&Project>) -> Resolved {
        merge(&self.items, &self.layers(project), &|name| {
            self.env.get(name).cloned()
        })
    }

    /// 能改的一层的那一份文件（系统配置、个人设置；项目配置核心不写，照个人设置给）。
    pub(crate) fn file(&self, layer: Layer) -> &File {
        match layer {
            Layer::System => &self.system,
            _ => &self.personal,
        }
    }

    /// 能改的一层现在磁盘上的样子：重读一遍换上，最终值跟着重算（第五条第 2 条第 1 款：手改过的先重读，再在新的字上改）。
    pub(crate) fn reread(&mut self, layer: Layer) {
        let old = self.file(layer);
        let fresh = File::read(&self.items, layer, old.path.clone(), old.shown.clone());
        self.replace(fresh);
    }

    /// 换上一份新的文件（写成了、重读了），最终值跟着重算。
    pub(crate) fn replace(&mut self, file: File) {
        match file.layer {
            Layer::System => self.system = file,
            _ => self.personal = file,
        }
        self.resolved = self.merged(None);
    }

    /// 新建这一层的文件时第一行指向的 Schema。
    pub(crate) fn schema(layer: Layer) -> &'static str {
        match layer {
            Layer::System => SYSTEM_SCHEMA,
            _ => PERSONAL_SCHEMA,
        }
    }

    /// 要合的几层。
    fn layers<'a>(&'a self, project: Option<&'a Project>) -> Layers<'a> {
        Layers {
            system: Some(&self.system.parsed),
            personal: Some(&self.personal.parsed),
            project: project.map(|project| (&project.file.parsed, project.trust)),
        }
    }
}
