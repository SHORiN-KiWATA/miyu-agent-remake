//! 预设在哪、几层怎么叠（施工 P-2 上，`docs/blueprint/presets.md`，`16-人格与预设.md` 第三、四节）：出厂的
//! `<资源目录>/presets/<编号>.toml`、系统区 `system/presets/<编号>.toml`、管理员家目录 `home/<管理员>/presets/<编号>.toml`，同名的
//! 后面的叠在前面的上面，逐格盖。文件怎么读成样子在 `miyu_policy::preset`。

use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use miyu_kernel::id::AccountId;
use miyu_policy::preset::{self, PresetFile, Problem};

pub use crate::layers::Layer;
use crate::layers::{read_text, real, valid};
use crate::resources::ResourceRoot;
use crate::root::DataRoot;

/// 预设文件的扩展名。
const EXTENSION: &str = "toml";

/// 叠好的一个预设。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// 编号。
    pub id: String,
    /// 叠好的文件。
    pub file: PresetFile,
    /// 有它的几层，从下往上。
    pub layers: Vec<Layer>,
}

/// 找预设出了错。
#[derive(Debug)]
pub enum PresetError {
    /// 编号不合写法。
    BadId(String),
    /// 哪一层都没有。
    NotFound(String),
    /// 文件写错了：哪一层、哪个编号、错在哪。
    Invalid(Layer, String, Problem),
    /// 读不了：哪个文件。
    Unreadable(PathBuf, io::Error),
}

impl fmt::Display for PresetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PresetError::BadId(id) => write!(f, "preset id {id:?} is not valid"),
            PresetError::NotFound(id) => write!(f, "preset {id:?} not found"),
            PresetError::Invalid(layer, id, problem) => {
                write!(f, "{} {id}.{EXTENSION}:", layer.as_str())?;
                match problem.line {
                    Some(line) => write!(f, "{line}: {}", problem.message),
                    None => write!(f, " {}", problem.message),
                }
            }
            PresetError::Unreadable(path, error) => {
                write!(f, "cannot read {}: {error}", path.display())
            }
        }
    }
}

impl std::error::Error for PresetError {}

/// 预设的几层：出厂、系统区、管理员的家目录。
#[derive(Debug, Clone)]
pub struct Presets {
    dirs: Vec<(Layer, PathBuf)>,
}

impl Presets {
    /// 照资源目录 `resources`、数据根 `root`、管理员 `admin` 定几层的位置。
    pub fn new(resources: &ResourceRoot, root: &DataRoot, admin: &AccountId) -> Presets {
        Presets {
            dirs: vec![
                (Layer::Shipped, resources.path().join("presets")),
                (Layer::System, root.system().join("presets")),
                (Layer::Home, root.account_dir(admin).join("presets")),
            ],
        }
    }

    /// 找预设 `id`，几层叠好。
    ///
    /// # Errors
    ///
    /// 编号不合写法、哪一层都没有、文件写错了、读不了。
    pub fn find(&self, id: &str) -> Result<Found, PresetError> {
        self.stack(id, None)
    }

    /// 同 [`Presets::find`]，只是家目录那一层的 `id` 照 `home` 这段字算，不读盘（施工 P-3 中）：`preset.set` 写之前照它查
    /// 改完的一份叠不叠得成。
    ///
    /// # Errors
    ///
    /// 同 [`Presets::find`]。
    pub fn find_with(&self, id: &str, home: &str) -> Result<Found, PresetError> {
        self.stack(id, Some(home))
    }

    /// 家目录那一层里预设 `id` 的文件（施工 P-3 中）：`preset.set` 写它、`preset.delete` 删它。编号由调用的一方查过。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：[`Presets::new`] 总排上家目录那一层。
    pub fn home_file(&self, id: &str) -> PathBuf {
        let (_, dir) = self
            .dirs
            .iter()
            .find(|(layer, _)| *layer == Layer::Home)
            .expect("几层里总有家目录那一层");
        file_of(dir, id)
    }

    /// 新建时由核心起的编号（施工 P-3 补，2026-10-08 项目主人定：新建不填编号）：`preset-<n>`，n 从 1 数起，取几层里都还没有
    /// 的最小的那个。只是挑，不占：写的一方照「已经有了就换下一个」再挑。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：编号数不完。
    pub fn unused_id(&self) -> String {
        (1..)
            .map(|n: u64| format!("preset-{n}"))
            .find(|id| !self.exists(id))
            .expect("总有没用过的编号")
    }

    /// 有没有哪一层有预设 `id` 的文件（施工 P-3 中，`preset.delete` 删完看还剩不剩）。不读文件。
    pub fn exists(&self, id: &str) -> bool {
        valid(id) && self.dirs.iter().any(|(_, dir)| file_of(dir, id).is_file())
    }

    /// 预设 `id` 的几层叠好；`home` 是写了的，家目录那一层照它算。
    fn stack(&self, id: &str, home: Option<&str>) -> Result<Found, PresetError> {
        if !valid(id) {
            return Err(PresetError::BadId(id.to_string()));
        }
        let mut found = Found {
            id: id.to_string(),
            file: PresetFile::default(),
            layers: Vec::new(),
        };
        for (layer, dir) in &self.dirs {
            let text = match home {
                Some(text) if *layer == Layer::Home => text.to_string(),
                _ => {
                    let path = file_of(dir, id);
                    if !path.is_file() {
                        continue;
                    }
                    let Some(text) = read(&path)? else {
                        continue;
                    };
                    text
                }
            };
            let file = preset::read(&text)
                .map_err(|problem| PresetError::Invalid(*layer, id.to_string(), problem))?;
            found.file = file.over(std::mem::take(&mut found.file));
            found.layers.push(*layer);
        }
        if found.layers.is_empty() {
            return Err(PresetError::NotFound(id.to_string()));
        }
        Ok(found)
    }

    /// 几层里所有预设的编号，照编号排好，不重复。文件名不是 `<合写法的编号>.toml` 的、不是文件的不算；读不了的一层当没有。
    pub fn ids(&self) -> Vec<String> {
        let mut ids = BTreeSet::new();
        for (_, dir) in &self.dirs {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                if let Some(id) = id_of(&entry.path())
                    && entry.path().is_file()
                {
                    ids.insert(id);
                }
            }
        }
        ids.into_iter().collect()
    }
}

/// 查出来的一处（`miyu check`）：哪一层、哪个文件（真的路径）、错在哪。
#[derive(Debug)]
pub struct Checked {
    /// 哪一层。
    pub layer: Layer,
    /// 文件真的路径。
    pub path: PathBuf,
    /// 写错了，还是读不了。
    pub issue: Issue,
}

/// 一处的错。
#[derive(Debug)]
pub enum Issue {
    /// 写错了。
    Wrong(Problem),
    /// 读不了：原因。
    Unreadable(io::Error),
}

impl Presets {
    /// 查每一层里的每一份：各层各查各的，上面一层盖住了照样报。照层、文件的先后。
    pub fn check(&self) -> Vec<Checked> {
        let mut found = Vec::new();
        for id in self.ids() {
            for (layer, dir) in &self.dirs {
                let path = file_of(dir, &id);
                if path.is_file() {
                    found.extend(check_one(*layer, &path));
                }
            }
        }
        found.sort_by(|a, b| (a.layer, &a.path).cmp(&(b.layer, &b.path)));
        found
    }

    /// `path` 是不是某一层 `presets/` 下的 `<编号>.toml`：是的查它（没有这个文件的报读不了），不是的没有。两边都照真的
    /// 位置比（还没有的照最近一层在的上级换）。
    pub fn check_file(&self, path: &Path) -> Option<Vec<Checked>> {
        let path = real(path);
        for (layer, dir) in &self.dirs {
            if path.parent() != Some(real(dir).as_path()) {
                continue;
            }
            id_of(&path)?;
            if !path.exists() {
                return Some(vec![Checked {
                    layer: *layer,
                    path,
                    issue: Issue::Unreadable(io::Error::from(io::ErrorKind::NotFound)),
                }]);
            }
            return Some(check_one(*layer, &path));
        }
        None
    }
}

/// 一层里的一份。
fn check_one(layer: Layer, path: &Path) -> Vec<Checked> {
    let issue = match read_text(path) {
        Ok(Some(text)) => match preset::read(&text) {
            Ok(_) => return Vec::new(),
            Err(problem) => Issue::Wrong(problem),
        },
        Ok(None) => return Vec::new(),
        Err(error) => Issue::Unreadable(error),
    };
    vec![Checked {
        layer,
        path: path.to_path_buf(),
        issue,
    }]
}

/// 一层的目录里编号 `id` 的那一份。
fn file_of(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.{EXTENSION}"))
}

/// 文件名是 `<合写法的编号>.toml` 的，交回编号。
fn id_of(path: &Path) -> Option<String> {
    if path.extension()? != EXTENSION {
        return None;
    }
    let id = path.file_stem()?.to_str()?;
    valid(id).then(|| id.to_string())
}

/// 读一个文件：没有的是没有。
fn read(path: &Path) -> Result<Option<String>, PresetError> {
    read_text(path).map_err(|error| PresetError::Unreadable(path.to_path_buf(), error))
}

#[cfg(test)]
mod tests;
