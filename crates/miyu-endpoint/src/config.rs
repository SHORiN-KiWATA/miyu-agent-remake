//! 配置服务（`docs/blueprint/config.md`「怎么走」第二、三条，施工 8-2）：核心起来时读系统配置、管理员的个人设置和
//! 信任的记录，合出不算项目配置的最终值；造会话、说话、`config.get` 时照目录找项目配置，带上信任着的那一份再合一次。
//!
//! - [`Config::load`]：起来时读一次。读不进来不影响起不起得来（G8）：问题记下，那一层照空的算，每份有问题的文件记一条
//!   `WARN config problems`。
//! - [`Config::resolved`]：不算项目配置的最终值；`Config::with_project`：照一个目录带上项目配置。
//! - 协议上的 `config.schema`、`config.get`、`config.check` 在 `config/methods.rs`，写成 JSON 的几样在 `config/wire.rs`。
//!
//! 这一步只读：改、写盘、监视、推送随 8-3、8-4。最终值起来以后不变，会话用不着经 `watch` 拿（随 8-4）。

mod file;
pub(crate) mod methods;
mod project;
mod trust;
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
    /// 不算项目配置的最终值。
    resolved: Resolved,
}

/// 一个目录找到的项目配置。
#[derive(Debug, Clone)]
pub(crate) struct Project {
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
        let system_path = SYSTEM
            .iter()
            .fold(root.path().to_path_buf(), |p, s| p.join(s));
        let system = File::read(&items, Layer::System, system_path, SYSTEM.join("/"));
        let personal = File::read(
            &items,
            Layer::Personal,
            root.account_dir(account).join(PERSONAL),
            format!("home/{}/{PERSONAL}", account.as_str()),
        );
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
        Config::assemble(root, home, items, env, system, personal, trust)
    }

    /// 什么都没读：全是默认值（核心不给配置的时候，例如协议端点的测试）。
    pub fn defaults(root: &DataRoot, account: &AccountId, items: Vec<Item>) -> Config {
        let system = File::nothing(Layer::System, &root.system(), &SYSTEM.join("/"));
        let personal = File::nothing(
            Layer::Personal,
            &root.account_dir(account).join(PERSONAL),
            &format!("home/{}/{PERSONAL}", account.as_str()),
        );
        Config::assemble(
            root,
            None,
            items,
            BTreeMap::new(),
            system,
            personal,
            Vec::new(),
        )
    }

    fn assemble(
        root: &DataRoot,
        home: Option<&Path>,
        items: Vec<Item>,
        env: BTreeMap<&'static str, String>,
        system: File,
        personal: File,
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
        Some(Project { file, trust })
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

    /// 要合的几层。
    fn layers<'a>(&'a self, project: Option<&'a Project>) -> Layers<'a> {
        Layers {
            system: Some(&self.system.parsed),
            personal: Some(&self.personal.parsed),
            project: project.map(|project| (&project.file.parsed, project.trust)),
        }
    }
}
