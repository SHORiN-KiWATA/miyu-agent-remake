//! 人格在哪、几层怎么叠（施工 P-1 上，`docs/blueprint/personas.md`，`16-人格与预设.md` 第四节）：出厂的
//! `<资源目录>/personas/<编号>/`、系统区 `system/personas/<编号>/`、管理员家目录 `home/<管理员>/personas/<编号>/`，同名的后面
//! 的叠在前面的上面：`persona.toml` 逐项盖，`prompts/` 里的文件同名替换、没写的沿用。读法同配置文件：只有本人写自己的
//! 家目录，顺着链接读。文件怎么读成样子在 `miyu_policy::persona`。

use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use miyu_kernel::id::AccountId;
use miyu_policy::PersonaTexts;
use miyu_policy::persona;
pub use miyu_policy::persona::{PersonaFile, Phrases, Problem};

use crate::resources::ResourceRoot;
use crate::root::DataRoot;

/// 人设在人格目录里的位置。
pub const PERSONA_MD: &str = "prompts/persona.md";

/// 一层：人格从哪来。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// 随发行附带的，只读。
    Shipped,
    /// 系统区，管理员给大家的。
    System,
    /// 管理员自己的家目录。
    Home,
}

impl Layer {
    /// 协议里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Shipped => "shipped",
            Layer::System => "system",
            Layer::Home => "home",
        }
    }
}

/// 叠好的一个人格。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// 编号。
    pub id: String,
    /// 叠好的 `persona.toml`。
    pub file: PersonaFile,
    /// 叠好的字，造快照用。
    pub texts: PersonaTexts,
    /// 有它的几层，从下往上。
    pub layers: Vec<Layer>,
    /// 人设来自哪一层，没有的是没有。
    pub persona_from: Option<Layer>,
    /// 示范对话来自哪一层，没有的是没有。
    pub examples_from: Option<Layer>,
    /// 它住在谁的家目录里：有家目录那一层的是那个账号，只有出厂、系统区的没有（记忆归哪个账号照它，`personas.md`）。
    pub home: Option<AccountId>,
}

/// 找人格出了错。
#[derive(Debug)]
pub enum PersonaError {
    /// 编号不合写法。
    BadId(String),
    /// 哪一层都没有。
    NotFound(String),
    /// 文件写错了：哪一层、错在哪。
    Invalid(Layer, Problem),
    /// 读不了：哪个文件。
    Unreadable(PathBuf, io::Error),
}

impl fmt::Display for PersonaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PersonaError::BadId(id) => write!(f, "persona id {id:?} is not valid"),
            PersonaError::NotFound(id) => write!(f, "persona {id:?} not found"),
            PersonaError::Invalid(layer, problem) => write!(f, "{} {problem}", layer.as_str()),
            PersonaError::Unreadable(path, error) => {
                write!(f, "cannot read {}: {error}", path.display())
            }
        }
    }
}

impl std::error::Error for PersonaError {}

/// 人格的几层：出厂、系统区、管理员的家目录。
#[derive(Debug, Clone)]
pub struct Personas {
    dirs: Vec<(Layer, PathBuf)>,
    admin: AccountId,
}

impl Personas {
    /// 照资源目录 `resources`、数据根 `root`、管理员 `admin` 定几层的位置。
    pub fn new(resources: &ResourceRoot, root: &DataRoot, admin: &AccountId) -> Personas {
        Personas {
            dirs: vec![
                (Layer::Shipped, resources.path().join("personas")),
                (Layer::System, root.system().join("personas")),
                (Layer::Home, root.account_dir(admin).join("personas")),
            ],
            admin: admin.clone(),
        }
    }

    /// 找人格 `id`，几层叠好。
    ///
    /// # Errors
    ///
    /// 编号不合写法、哪一层都没有、文件写错了、读不了。
    pub fn find(&self, id: &str) -> Result<Found, PersonaError> {
        if !valid(id) {
            return Err(PersonaError::BadId(id.to_string()));
        }
        let mut found = Found {
            id: id.to_string(),
            file: PersonaFile::default(),
            texts: PersonaTexts::default(),
            layers: Vec::new(),
            persona_from: None,
            examples_from: None,
            home: None,
        };
        for (layer, dir) in &self.dirs {
            let dir = dir.join(id);
            if !dir.is_dir() {
                continue;
            }
            found.layers.push(*layer);
            if *layer == Layer::Home {
                found.home = Some(self.admin.clone());
            }
            if let Some(text) = read(&dir.join(persona::TOML))? {
                let file = persona::read_toml(&text)
                    .map_err(|problem| PersonaError::Invalid(*layer, problem))?;
                found.file = file.over(std::mem::take(&mut found.file));
            }
            if let Some(text) = read(&dir.join(PERSONA_MD))? {
                found.texts.persona = text;
                found.persona_from = Some(*layer);
            }
            if let Some(text) = read(&dir.join(persona::EXAMPLES))? {
                found.texts.examples = persona::read_examples(&text)
                    .map_err(|problem| PersonaError::Invalid(*layer, problem))?;
                found.examples_from = Some(*layer);
            }
        }
        if found.layers.is_empty() {
            return Err(PersonaError::NotFound(id.to_string()));
        }
        Ok(found)
    }

    /// 人格 `id` 住在谁的家目录（有家目录那一层）：只看目录在不在，不读文件。
    pub fn home_of(&self, id: &str) -> Option<AccountId> {
        let (_, dir) = self.dirs.iter().find(|(layer, _)| *layer == Layer::Home)?;
        (valid(id) && dir.join(id).is_dir()).then(|| self.admin.clone())
    }

    /// 用人格 `id`、属主是 `owner` 的会话，记忆归哪个账号（和记忆的会话对过，`17-记忆.md` L16，`personas.md`「怎么走」第 5 条）：
    /// 人格住在谁的家目录就归谁；出厂、系统区的人格归会话的属主。属主是系统账号的归管理员，随 O-4。造会话、载入都照它。
    pub fn memory_account(&self, id: &str, owner: &AccountId) -> AccountId {
        self.home_of(id).unwrap_or_else(|| owner.clone())
    }

    /// 几层里所有人格的编号，照编号排好，不重复。目录名不合写法的不算；读不了的一层当没有。
    pub fn ids(&self) -> Vec<String> {
        let mut ids = BTreeSet::new();
        for (_, dir) in &self.dirs {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if valid(&name) && entry.path().is_dir() {
                    ids.insert(name);
                }
            }
        }
        ids.into_iter().collect()
    }
}

/// 读一个文件：没有的是没有。
fn read(path: &Path) -> Result<Option<String>, PersonaError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(PersonaError::Unreadable(path.to_path_buf(), error)),
    }
}

/// 人格的编号合不合写法：小写字母开头，小写字母、数字、`-`、`_`，最多 64 个。它是一层目录，不许带路径。
pub fn valid(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && id.len() <= 64
}

#[cfg(test)]
mod tests;
