//! 软件包清单在哪、两层怎么认（施工 9-1 上，`docs/blueprint/packages.md`「在哪」）：出厂的放资源目录的
//! `packages/<编号>.toml`，管理员自己装的放 `home/<管理员>/packages/<编号>.toml`。一个包只有一份清单，不像人格那样一层层
//! 叠：同一个编号两层都有的，认出厂的，家目录那一份报 `duplicate`。两个包要同一个子命令名的，出厂的先于家目录、同一层照
//! 编号，后读到的那一份报 `command_taken`。读法同配置文件：顺着链接读。文件怎么读成样子在 `miyu_config::package`。
//!
//! 包自己在这台机器上的状态放 `<数据根>/state/packages/<编号>/`，包自己建、自己用（[`Packages::state_dir`]）。

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use miyu_config::package::{self, Code, Manifest, Problem};
use miyu_kernel::id::AccountId;

use crate::resources::ResourceRoot;
use crate::root::DataRoot;

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
    /// 编号：文件名去掉 `.toml`。
    pub id: String,
    /// 哪一层。
    pub layer: Layer,
    /// 文件在哪。
    pub path: PathBuf,
    /// 读成的样子，或者问题。
    pub read: Result<Manifest, Issue>,
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
        }
    }

    /// 只有出厂那一层（施工 9-1 下）：生成配置的样本照出厂带的包，和数据根无关。状态目录没有意义，是空的。
    pub fn shipped(resources: &ResourceRoot) -> Packages {
        Packages {
            dirs: vec![(Layer::Shipped, resources.path().join("packages"))],
            state: PathBuf::new(),
        }
    }

    /// 两层里所有的清单，照编号排（同一个编号出厂的在前）。目录读不了的一层当没有；不是 `.toml` 的、编号不合写法的不算。
    pub fn read(&self) -> Vec<Found> {
        let mut found: Vec<Found> = self
            .dirs
            .iter()
            .flat_map(|(layer, dir)| {
                files(dir)
                    .into_iter()
                    .map(move |(id, path)| (*layer, id, path))
            })
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
        found
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

/// 一层目录里的清单：编号和路径。读不了的目录当没有。
fn files(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let id = path
                .file_name()?
                .to_str()?
                .strip_suffix(".toml")?
                .to_string();
            (crate::personas::valid(&id) && path.is_file()).then_some((id, path))
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

/// 子命令名：照读的先后（出厂的先于家目录，同一层照编号）先到先得，后到的那一份报 `command_taken`，报在子命令名那一行。
fn taken(found: &mut [Found]) {
    let mut order: Vec<usize> = (0..found.len()).collect();
    order.sort_by(|&one, &other| {
        (found[one].layer, &found[one].id).cmp(&(found[other].layer, &found[other].id))
    });
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

#[cfg(test)]
mod tests;
