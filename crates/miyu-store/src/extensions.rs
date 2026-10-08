//! 扩展的开关 `system/extensions.json`（`docs/blueprint/extensions.md`「对外的样子」，施工 9-4 上）：哪几个 `process` 包开着，
//! 批过哪些扩展能力（施工 9-4 下上）。
//! 没写的照清单的 `start`，由用它的一方定；这里只存人明着开、关过的。放在 `system/`、不放 `state/`：`state/` 是派生的、
//! 能删掉重建，开关是人做的决定。读写照只给自己看的 JSON 小文件的规矩。

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config_file::WriteError;
use crate::private_json;
pub use crate::private_json::BadFile;

/// 文件名：在 `system/` 里。
pub const FILE: &str = "extensions.json";

/// 现在的写法。
const VERSION: u32 = 1;

/// 读好的一份。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Switches {
    /// 写法的版本：现在是 1。
    pub version: u32,
    /// 包的编号到开没开：人明着开、关过的才有。
    pub on: BTreeMap<String, bool>,
    /// 包的编号到批过的扩展能力（施工 9-4 下上，`extensions.md`「能力」）：批的时候它声明的全部，照能力表的先后。一个都没
    /// 批过的不写这一格，以前的文件读进来是空的。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub approved: BTreeMap<String, Vec<String>>,
}

impl Default for Switches {
    fn default() -> Switches {
        Switches {
            version: VERSION,
            on: BTreeMap::new(),
            approved: BTreeMap::new(),
        }
    }
}

/// 读到的：开关和这份文件的版本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// 读好的。没有这个文件的是空的。
    pub switches: Switches,
    /// 整份字节的 SHA-256；没有这个文件的是空的。
    pub version: Option<String>,
}

/// 读 `path`。没有这个文件的是空的。
///
/// # Errors
///
/// 读不了、太大、不是 UTF-8；不是这个形状、版本不是 1。
pub fn read(path: &Path) -> Result<Read, BadFile> {
    let (switches, version): (Switches, _) = private_json::read(path)?;
    if switches.version != VERSION {
        return Err(BadFile::Shape(format!(
            "unknown version {}",
            switches.version
        )));
    }
    Ok(Read { switches, version })
}

/// 把 `switches` 整份写进 `path`，Unix 上 0600。`read` 是上一次读到的版本（那时还没有、读不成的是空的）。
///
/// # Errors
///
/// 这一瞬间有人手改了（[`WriteError::Changed`]）；写不进。都是什么都没变。
pub fn write(path: &Path, switches: &Switches, read: Option<&str>) -> Result<(), WriteError> {
    private_json::write(path, switches, read)
}
