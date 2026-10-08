//! 人格、预设共用的三层（`docs/blueprint/personas.md`、`presets.md`，`16-人格与预设.md` 第四节）：出厂的、系统区的、管理员家目录
//! 里的，同名的后面的叠在前面的上面。这里只有层本身和两样读盘的小事，各自怎么叠在 `personas`、`presets`。

use std::io;
use std::path::{Path, PathBuf};

/// 一层：从哪来。
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

/// 真的位置；换不成的照原样。`miyu check` 写了文件时两边都照它比（施工 8-30）。
pub(crate) fn real(path: &Path) -> PathBuf {
    miyu_fs::resolve(Path::new("/"), None, &path.to_string_lossy())
        .unwrap_or_else(|_| path.to_path_buf())
}

/// 读一个文件：没有的是没有。
pub(crate) fn read_text(path: &Path) -> io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// 人格、预设的编号合不合写法：小写字母开头，小写字母、数字、`-`、`_`，最多 64 个。它是一层目录或一个文件名，不许带路径。
pub fn valid(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && id.len() <= 64
}
