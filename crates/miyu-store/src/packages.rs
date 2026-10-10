//! 软件包清单在哪、两层怎么认（施工 9-1 上，`docs/blueprint/packages.md`「在哪」）：一个文件夹就是一个包（施工 F-8 上，设计
//! `31-软件包.md` 第二节），出厂的放资源目录的 `packages/<编号>/package.toml`，管理员自己装的放
//! `home/<管理员>/packages/<编号>/package.toml`，包的文件都在这个文件夹里。一个包只有一份清单，不像人格那样一层层
//! 叠：同一个编号两层都有的，认出厂的，家目录那一份报 `duplicate`。两个包要同一个子命令名的，出厂的先于家目录、同一层照
//! 编号，后读到的那一份报 `command_taken`。读法同配置文件：顺着链接读。文件怎么读成样子在 `miyu_config::package`。
//!
//! 包自己在这台机器上的状态放 `<数据根>/state/packages/<编号>/`，包自己建、自己用（[`Packages::state_dir`]）。
//!
//! 装、卸（施工 F-5 上）：只动家目录那一层（`install.rs`）。卸掉的出厂的包在家目录记一笔 `<编号>.removed`，读的时候不算装了
//! （[`Packages::read`]），另外照样读得出来（[`Packages::read_removed`]），好让头给人装回来。家目录里以前的写法
//! （`<编号>.toml` 加同名目录）由 `migrate.rs` 挪成新的（施工 F-8 上）。

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use miyu_config::package::{self, Code, Manifest, Problem};
use miyu_kernel::id::AccountId;

use crate::resources::ResourceRoot;
use crate::root::DataRoot;

pub mod install;
pub mod migrate;

/// 清单在包文件夹里的名字（施工 F-8 上）。
pub const MANIFEST: &str = "package.toml";

/// 一层：清单从哪来。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// 随发行附带的，只读。
    Shipped,
    /// 管理员自己装的。
    Home,
}

impl Layer {
    /// 协议里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Shipped => "shipped",
            Layer::Home => "home",
        }
    }
}

/// 读到的一份清单。
#[derive(Debug)]
pub struct Found {
    /// 编号：包文件夹的名字。
    pub id: String,
    /// 哪一层。
    pub layer: Layer,
    /// 清单在哪：包文件夹里的 `package.toml`。
    pub path: PathBuf,
    /// 读成的样子，或者问题。
    pub read: Result<Manifest, Issue>,
}

impl Found {
    /// 包自己的文件放在哪个目录（施工 R-5 三补，`packages.md`「在哪」）：包文件夹本身，清单就在里面，例如
    /// `home/<管理员>/packages/embed/package.toml` 的是 `home/<管理员>/packages/embed/`（施工 F-8 上）。两层都是这样。
    pub fn files_dir(&self) -> PathBuf {
        self.path
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf)
    }
}

/// 一份清单的问题。
#[derive(Debug)]
pub enum Issue {
    /// 写错了，或者和别的包撞了。
    Wrong(Problem),
    /// 读不了。
    Unreadable(io::Error),
}

/// 软件包清单的两层。
#[derive(Debug, Clone)]
pub struct Packages {
    dirs: Vec<(Layer, PathBuf)>,
    state: PathBuf,
    /// 管理员（施工 O-4 下）：声明了系统账号、编号和它一样的包不收。只有出厂那一层的没有。
    admin: Option<AccountId>,
}

impl Packages {
    /// 照资源目录 `resources`、数据根 `root`、管理员 `admin` 定两层的位置。
    pub fn new(resources: &ResourceRoot, root: &DataRoot, admin: &AccountId) -> Packages {
        Packages {
            dirs: vec![
                (Layer::Shipped, resources.path().join("packages")),
                (Layer::Home, root.account_dir(admin).join("packages")),
            ],
            state: root.state().join("packages"),
            admin: Some(admin.clone()),
        }
    }

    /// 只有出厂那一层（施工 9-1 下）：生成配置的样本照出厂带的包，和数据根无关。状态目录没有意义，是空的。
    pub fn shipped(resources: &ResourceRoot) -> Packages {
        Packages {
            dirs: vec![(Layer::Shipped, resources.path().join("packages"))],
            state: PathBuf::new(),
            admin: None,
        }
    }

    /// 两层里所有的清单，照编号排（同一个编号出厂的在前）。目录读不了的一层当没有；不是文件夹的、编号不合写法的、文件夹里
    /// 没有 `package.toml` 的不算。
    /// 卸掉的出厂的包（施工 F-5 上，家目录记了一笔的）不在里面。
    pub fn read(&self) -> Vec<Found> {
        let removed = self.removed();
        let mut found: Vec<Found> = self
            .dirs
            .iter()
            .flat_map(|(layer, dir)| {
                files(dir)
                    .into_iter()
                    .map(move |(id, path)| (*layer, id, path))
            })
            .filter(|(layer, id, _)| !(*layer == Layer::Shipped && removed.contains(id)))
            .map(|(layer, id, path)| Found {
                read: std::fs::read_to_string(&path)
                    .map_err(Issue::Unreadable)
                    .and_then(|text| package::read(&text).map_err(Issue::Wrong)),
                id,
                layer,
                path,
            })
            .collect();
        found.sort_by(|one, other| (&one.id, one.layer).cmp(&(&other.id, other.layer)));
        duplicates(&mut found);
        taken(&mut found);
        features_taken(&mut found);
        if let Some(admin) = &self.admin {
            accounts_taken(&mut found, admin);
        }
        found
    }

    /// 卸掉的出厂的包（施工 F-5 上）：家目录记了一笔、出厂那一层有清单的，照编号排，各自读成样子（不和别的包比撞没撞）。
    pub fn read_removed(&self) -> Vec<Found> {
        let removed = self.removed();
        let Some((_, dir)) = self.dirs.iter().find(|(layer, _)| *layer == Layer::Shipped) else {
            return Vec::new();
        };
        let mut listed = files(dir);
        listed.sort();
        listed
            .into_iter()
            .filter(|(id, _)| removed.contains(id))
            .map(|(id, path)| Found {
                read: std::fs::read_to_string(&path)
                    .map_err(Issue::Unreadable)
                    .and_then(|text| package::read(&text).map_err(Issue::Wrong)),
                id,
                layer: Layer::Shipped,
                path,
            })
            .collect()
    }

    /// 家目录那一层的目录（施工 F-5 上：装、卸只动它）；只有出厂那一层的没有。
    pub fn home_dir(&self) -> Option<&Path> {
        self.dirs
            .iter()
            .find(|(layer, _)| *layer == Layer::Home)
            .map(|(_, dir)| dir.as_path())
    }

    /// 家目录里记着卸掉的出厂的包的编号（施工 F-5 上）。
    pub fn removed(&self) -> Vec<String> {
        self.home_dir().map(install::removed).unwrap_or_default()
    }

    /// 两层的目录：`miyu check` 认写了的文件是不是一份清单（两边换成真的路径比，`miyu-endpoint` 的 `check.rs`）。
    pub fn dirs(&self) -> impl Iterator<Item = (Layer, &Path)> {
        self.dirs.iter().map(|(layer, dir)| (*layer, dir.as_path()))
    }

    /// 包 `id` 在这台机器上放状态的目录：`<数据根>/state/packages/<编号>/`。不建：包自己建。
    pub fn state_dir(&self, id: &str) -> PathBuf {
        self.state.join(id)
    }
}

/// 读成了的清单里声明了系统账号的那些包的账号（施工 O-4 下）：账号名就是包的编号，照编号排。
pub fn system_accounts(found: &[Found]) -> Vec<AccountId> {
    found
        .iter()
        .filter(|found| declares_account(found))
        .filter_map(|found| AccountId::parse(&found.id).ok())
        .collect()
}

/// 读成了、声明了系统账号。
fn declares_account(found: &Found) -> bool {
    found.read.as_ref().is_ok_and(|manifest| {
        manifest
            .process
            .as_ref()
            .is_some_and(|process| process.system_account)
    })
}

/// 包里的程序 `program` 在哪（施工 9-2，`packages.md`「转交」）：主程序 `main` 真实位置旁边的（Windows 加 `.exe`，和
/// `miyu web` 找 `miyu-web` 一个办法）；没有的是没有。不找 `PATH`：别的程序冒充不了（2026-10-01 项目主人定，第二节「拆 M9
/// 时另带四样」第 4 条）。
pub fn locate(program: &str, main: &Path) -> Option<PathBuf> {
    let file = format!("{program}{}", std::env::consts::EXE_SUFFIX);
    let real = std::fs::canonicalize(main).unwrap_or_else(|_| main.to_path_buf());
    real.parent()
        .map(|dir| dir.join(&file))
        .filter(|path| path.is_file())
}

/// 一层目录里的清单：编号（包文件夹的名字）和清单的路径。读不了的目录当没有；点开头的（装到一半的暂存、备份）编号不合
/// 写法，不算。
fn files(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let folder = entry.path();
            let id = folder.file_name()?.to_str()?.to_string();
            let manifest = folder.join(MANIFEST);
            (crate::personas::valid(&id) && manifest.is_file()).then_some((id, manifest))
        })
        .collect()
}

/// 同一个编号：出厂的那一份照常，家目录那一份报 `duplicate`。
fn duplicates(found: &mut [Found]) {
    for index in 1..found.len() {
        if found[index].id == found[index - 1].id {
            let id = found[index].id.clone();
            found[index].read = Err(Issue::Wrong(Problem {
                line: None,
                code: Code::Duplicate,
                message: format!("package {id} is already installed with the shipped ones"),
                detail: id,
            }));
        }
    }
}

/// 读的先后：出厂的先于家目录，同一层照编号。先到先得的几样（子命令名、功能的编号）照它。
fn reading_order(found: &[Found]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..found.len()).collect();
    order.sort_by(|&one, &other| {
        (found[one].layer, &found[one].id).cmp(&(found[other].layer, &found[other].id))
    });
    order
}

/// 子命令名：照读的先后先到先得，后到的那一份报 `command_taken`，报在子命令名那一行。
fn taken(found: &mut [Found]) {
    let order = reading_order(found);
    let mut owners: BTreeMap<String, String> = BTreeMap::new();
    for index in order {
        let Ok(manifest) = &found[index].read else {
            continue;
        };
        let Some(command) = &manifest.command else {
            continue;
        };
        let (name, line) = (command.name.clone(), command.line);
        match owners.get(&name) {
            None => {
                owners.insert(name, found[index].id.clone());
            }
            Some(owner) => {
                let message = format!("command {name} is already taken by package {owner}");
                found[index].read = Err(Issue::Wrong(Problem {
                    line,
                    code: Code::CommandTaken,
                    detail: name,
                    message,
                }));
            }
        }
    }
}

/// 功能的编号（施工 F-1，设计 30 第三节）：照读的先后先到先得，后到的那一份报 `feature_taken`、整份不收，报在那个功能那一行；
/// 没写功能的包照包的编号算（[`miyu_config::package::Manifest::features_of`]）。
fn features_taken(found: &mut [Found]) {
    let mut owners: BTreeMap<String, String> = BTreeMap::new();
    for index in reading_order(found) {
        let Ok(manifest) = &found[index].read else {
            continue;
        };
        let features = manifest.features_of(&found[index].id);
        let clash = features.iter().find_map(|feature| {
            owners
                .get(&feature.id)
                .map(|owner| (feature, owner.clone()))
        });
        match clash {
            Some((feature, owner)) => {
                let message = format!("feature {} is already taken by package {owner}", feature.id);
                found[index].read = Err(Issue::Wrong(Problem {
                    line: feature.line,
                    code: Code::FeatureTaken,
                    detail: feature.id.clone(),
                    message,
                }));
            }
            None => {
                for feature in features {
                    owners.insert(feature.id, found[index].id.clone());
                }
            }
        }
    }
}

/// 声明了系统账号、编号和管理员的账号一样的：报 `account_taken`，整份不收（施工 O-4 下）。
fn accounts_taken(found: &mut [Found], admin: &AccountId) {
    for one in found.iter_mut() {
        if declares_account(one) && one.id == admin.as_str() {
            one.read = Err(Issue::Wrong(Problem {
                line: None,
                code: Code::AccountTaken,
                message: format!("system account {} is already a person's account", one.id),
                detail: one.id.clone(),
            }));
        }
    }
}

#[cfg(test)]
mod tests;
