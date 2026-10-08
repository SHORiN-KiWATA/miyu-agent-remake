//! 模型的清单（施工 R-5 上，`docs/blueprint/recall.md` 第四条第 2 款）：一份 TOML 写一个本机模型叫什么、几维、怎么取向量、
//! 最长几个词，和它的几个文件（在哪下、SHA-256、多大）。换模型就是换一份清单（2026-10-07 项目主人定：做成可更换的）。
//!
//! 下载、核对是核心的事（R-5 中）；这个小程序只照 `role` 在目录里找文件、照别的几格算。WordPiece 以外的分词、`cls` 以外的
//! 取法，换到那样的模型时再加：现在写别的，读的时候就拒，不悄悄算错。

use std::fmt;
use std::path::{Component, Path};

use serde::Deserialize;

/// 一份清单。读进来的都查过（[`Manifest::parse`]）：两种文件各正好一个，数在范围里，文件名是一个单纯的名字。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// 模型的名字，例如 `bge-small-zh-v1.5`。向量记的模型编号是 `local:<id>`（[`Manifest::model`]）。
    pub id: String,
    /// 向量几维。
    pub dims: usize,
    /// 怎么从模型的输出取一句的向量。
    pub pooling: Pooling,
    /// 一句最多几个词（连 `[CLS]`、`[SEP]`）：超过的截掉后面的，留住 `[SEP]`。
    pub max_tokens: usize,
    /// 模型的几个文件。
    pub files: Vec<ModelFile>,
}

/// 怎么取一句的向量。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pooling {
    /// 取第一个词（`[CLS]`）那一格，再归一化（bge 的做法）。
    Cls,
}

/// 一个文件是做什么的。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// ONNX 模型。
    Model,
    /// WordPiece 的词表：一行一个词，行号就是编号。
    Vocab,
}

impl Role {
    /// 清单里的写法。
    fn as_str(self) -> &'static str {
        match self {
            Role::Model => "model",
            Role::Vocab => "vocab",
        }
    }
}

/// 模型的一个文件。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelFile {
    /// 做什么的。
    pub role: Role,
    /// 放在模型目录里叫什么：一个单纯的名字，不带目录。
    pub name: String,
    /// 从哪下载（核心用，R-5 中）。
    pub url: String,
    /// 文件内容的 SHA-256，十六进制（核心下载完照它核对，R-5 中）。
    pub sha256: String,
    /// 多少字节（核心下载时照它看进度、防下得太多，R-5 中）。
    pub size: u64,
}

/// 清单读不出来、不合写法。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// 文件读不了。
    Read {
        /// 哪个文件。
        path: String,
        /// 系统说的原因。
        error: String,
    },
    /// 读出来了，不合写法：TOML 坏了、少格、多格，或者查下来不对。
    Invalid(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Read { path, error } => write!(f, "cannot read {path}: {error}"),
            ManifestError::Invalid(why) => write!(f, "invalid manifest: {why}"),
        }
    }
}

impl std::error::Error for ManifestError {}

impl Manifest {
    /// 读 `path` 那一份。
    ///
    /// # Errors
    ///
    /// 读不了，或者不合写法（[`Manifest::parse`]）。
    pub fn read(path: &Path) -> Result<Manifest, ManifestError> {
        let text = std::fs::read_to_string(path).map_err(|error| ManifestError::Read {
            path: path.display().to_string(),
            error: error.to_string(),
        })?;
        Manifest::parse(&text)
    }

    /// 照 TOML 的原文 `text` 读一份，再查一遍。
    ///
    /// # Errors
    ///
    /// TOML 坏了、少格或多格、取法不认识；`model`、`vocab` 不是各正好一个；`dims` 是 0；`max_tokens` 放不下 `[CLS]`、
    /// `[SEP]`；文件名带目录。
    pub fn parse(text: &str) -> Result<Manifest, ManifestError> {
        let manifest: Manifest = toml_edit::de::from_str(text)
            .map_err(|error| ManifestError::Invalid(error.to_string().trim_end().to_string()))?;
        manifest.check().map_err(ManifestError::Invalid)?;
        Ok(manifest)
    }

    /// 查读进来的几格，交回第一处不对的。
    fn check(&self) -> Result<(), String> {
        if self.dims == 0 {
            return Err("dims must be at least 1".to_string());
        }
        if self.max_tokens < 2 {
            return Err("max_tokens must be at least 2".to_string());
        }
        for role in [Role::Model, Role::Vocab] {
            match self.files.iter().filter(|file| file.role == role).count() {
                0 => return Err(format!("no file is {}", role.as_str())),
                1 => {}
                _ => return Err(format!("two files are {}", role.as_str())),
            }
        }
        match self.files.iter().find(|file| !plain(&file.name)) {
            Some(file) => Err(format!("file name {} is not a plain name", file.name)),
            None => Ok(()),
        }
    }

    /// 向量记的模型编号：`local:<id>`（`recall.md` 第三条第 1 款）。
    pub fn model(&self) -> String {
        format!("local:{}", self.id)
    }

    /// 做 `role` 的那个文件。
    ///
    /// # Panics
    ///
    /// 照 [`Manifest::parse`] 读进来的不会：每种正好一个。自己拼的清单少了那一种才会。
    pub fn file(&self, role: Role) -> &ModelFile {
        self.files
            .iter()
            .find(|file| file.role == role)
            .unwrap_or_else(|| panic!("清单里没有 {} 的文件", role.as_str()))
    }
}

/// `name` 是不是一个单纯的文件名：不空、不是 `.`、`..`，两种平台的目录分隔符都不带（清单在三个平台上一样用）。
fn plain(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    matches!(
        (parts.next(), parts.next()),
        (Some(Component::Normal(_)), None)
    ) && !name.contains(['/', '\\'])
}
