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
//! 手改过的照新的字改（G5 第 4 条）；写成了换上新的最终值，新的连接、新的会话照它。
//!
//! 施工 8-4：监视几份文件，手改了当场重读（`config/observe.rs`）；每换上一份新的，交给会话、核心，系统配置、个人设置变了
//! 推 `config.changed`（`config/hub.rs`、`config/push.rs`）。
//!
//! 施工 8-5：密钥文件也在这里（`crate::secrets`）：起来时读、手改了重读，引用的密钥、环境变量取不到的报警告
//! （`Config::missing`），密钥文件的错误算进 `config_errors`。

mod environment;
pub(crate) mod file;
pub(crate) mod hub;
pub(crate) mod journal;
pub(crate) mod methods;
pub(crate) mod observe;
mod project;
pub(crate) mod push;
pub(crate) mod set;
mod trust;
pub(crate) mod trusting;
pub(crate) mod wire;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use miyu_config::merge::{Layers, Resolved, Trust, merge};
use miyu_config::{Item, Layer};
use miyu_kernel::id::AccountId;
use miyu_store::root::DataRoot;

pub use environment::Environment;
use file::File;

use crate::secrets::SecretsFile;

/// 运行日志的目标（`config.md`「出错」）。
pub(crate) const TARGET: &str = "miyu::config";

/// 系统配置在数据根里的位置。
const SYSTEM: [&str; 2] = ["system", "config.toml"];

/// 个人设置的文件名：在账号的家目录里。
const PERSONAL: &str = "settings.toml";

/// 密钥文件在数据根里的位置（施工 8-5）。
const SECRETS: [&str; 2] = ["system", miyu_store::secrets::FILE];

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
    /// 核心的环境：`{ env = … }` 照它取（施工 8-5）。
    environment: Environment,
    /// 系统配置。
    system: File,
    /// 管理员的个人设置。
    personal: File,
    /// 信任的记录。
    trust: Vec<trust::Record>,
    /// 密钥文件（施工 8-5）。
    pub(crate) secrets: SecretsFile,
    /// 几份核心自己写的文件在哪（施工 8-3）。
    pub(crate) places: Places,
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

/// 密钥文件在哪：`system/secrets.toml`。
fn secrets_path(root: &DataRoot) -> PathBuf {
    SECRETS
        .iter()
        .fold(root.path().to_path_buf(), |p, s| p.join(s))
}

/// 核心自己写的几份文件在哪：信任的记录、系统日志、账号日志（施工 8-3）。
#[derive(Debug, Clone)]
pub(crate) struct Places {
    /// 账号：日志里记是谁改的。
    pub(crate) account: AccountId,
    /// `home/<账号>/trust.toml`。
    trust: PathBuf,
    /// 系统日志 `system/journal.jsonl`。
    pub(crate) system_journal: PathBuf,
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
    /// 起来时读：数据根 `root` 里的系统配置、账号 `account` 的个人设置、信任的记录和密钥文件（施工 8-5），照清单 `items`
    /// 认，带 `env` 的项照 `environment` 读环境变量（只读这一次），`{ env = … }` 也照它。`home` 是系统的家目录。
    pub fn load(
        root: &DataRoot,
        account: &AccountId,
        home: Option<&Path>,
        items: Vec<Item>,
        environment: Environment,
    ) -> Config {
        let env: BTreeMap<&'static str, String> = items
            .iter()
            .filter_map(|item| item.env)
            .filter_map(|name| environment.get(name).map(|value| (name, value)))
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
        let secrets = SecretsFile::read(&secrets_path(root), &SECRETS.join("/"));
        crate::secrets::told(&secrets);
        let mut config =
            Config::assemble(root, account, home, items, env, [system, personal], trust);
        config.environment = environment;
        config.secrets = secrets;
        config
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
            environment: Environment::of(&[]),
            secrets: SecretsFile::nothing(&secrets_path(root), &SECRETS.join("/")),
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

    /// 系统配置、个人设置、密钥文件（施工 8-5）里现在有几处错误（不算警告）：握手的 `config_errors`。
    pub fn errors(&self) -> usize {
        self.system.counts().0 + self.personal.counts().0 + self.secrets.errors()
    }

    /// 一份配置文件里引用的密钥、环境变量取不到的（施工 8-5）：`unknown_secret`、`env_not_set`，都是警告。
    pub(crate) fn missing(
        &self,
        parsed: &miyu_config::parse::Parsed,
        layer: Layer,
    ) -> Vec<miyu_config::problem::Problem> {
        miyu_config::secret::missing(
            &self.items,
            parsed,
            layer,
            &|name| self.secrets.has(name),
            &|name| self.environment.has(name),
        )
    }

    /// 照引用取一个密钥（施工 8-6，路由取 key）：`{ secret }` 照手里的密钥文件，`{ env }` 照核心的环境。没设的、设成空的、
    /// 有控制字符、太长的是空的。
    pub fn secret(
        &self,
        reference: &miyu_config::secret::Reference,
    ) -> Option<miyu_config::secret::Secret> {
        match reference {
            miyu_config::secret::Reference::Secret(name) => {
                self.secrets.stored.entries.get(name).cloned()
            }
            miyu_config::secret::Reference::Env(name) => self
                .environment
                .get(name)
                .and_then(|value| miyu_config::secret::Secret::new(&value).ok()),
        }
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
    pub(crate) fn layers<'a>(&'a self, project: Option<&'a Project>) -> Layers<'a> {
        Layers {
            system: Some(&self.system.parsed),
            personal: Some(&self.personal.parsed),
            project: project.map(|project| (&project.file.parsed, project.trust)),
        }
    }
}
