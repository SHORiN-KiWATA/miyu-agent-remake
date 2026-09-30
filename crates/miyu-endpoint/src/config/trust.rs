//! 项目配置的信任（`docs/blueprint/config.md`「怎么走」第三条第 2、3 条，G3）：读 `home/<账号>/trust.toml`，照仓库在哪、
//! 内容的版本认一份项目配置信不信任。写（`config.trust`）随 8-3。
//!
//! 一个仓库一条 `[[project]]`：`path` 仓库在哪（`.miyu` 所在的那一层，家目录下的写成 `~/…`），`version` 答的是哪一份，
//! `trusted` 信不信任。同一个仓库有几条的，后面的盖掉前面的。写法不对的那一条不算。整份读不进来的，照没有记录，由
//! 调用的一方记一条 `WARN`。

use std::path::{Path, PathBuf};

use toml_edit::Document;

use miyu_config::merge::Trust;
use miyu_store::config_file;

/// 文件名。
pub(crate) const FILE: &str = "trust.toml";

/// 一条记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Record {
    /// 仓库在哪，写法照文件里的。
    pub(crate) path: String,
    /// 答的是哪一份。
    pub(crate) version: String,
    /// 信不信任。
    pub(crate) trusted: bool,
}

/// 读 `path` 这一份记录；没有的是空的。
///
/// # Errors
///
/// 读不进来、TOML 写法不对：原因。
pub(crate) fn read(path: &Path) -> Result<Vec<Record>, String> {
    let Some(text) = config_file::read(path).map_err(|error| error.to_string())? else {
        return Ok(Vec::new());
    };
    let document = Document::parse(text.text).map_err(|error| error.message().to_string())?;
    let Some(projects) = document
        .get("project")
        .and_then(|item| item.as_array_of_tables())
    else {
        return Ok(Vec::new());
    };
    Ok(projects
        .iter()
        .filter_map(|table| {
            Some(Record {
                path: table.get("path")?.as_str()?.to_string(),
                version: table.get("version")?.as_str()?.to_string(),
                trusted: table.get("trusted")?.as_bool()?,
            })
        })
        .collect())
}

/// 仓库 `repo`（真实的位置）里版本是 `version` 的项目配置信不信任：最后一条对得上仓库的记录，版本一样的照它答的，
/// 版本不一样（内容变了）、没有记录（包括仓库挪了地方）的是还没问过。
pub(crate) fn trust_of(
    records: &[Record],
    repo: &Path,
    version: &str,
    home: Option<&Path>,
) -> Trust {
    let found = records
        .iter()
        .rev()
        .find(|record| expanded(&record.path, home).is_some_and(|path| path == repo));
    match found {
        Some(record) if record.version == version => match record.trusted {
            true => Trust::Trusted,
            false => Trust::Distrusted,
        },
        _ => Trust::Unknown,
    }
}

/// 记录里的路径：`~`、`~/` 开头的照家目录换开（一段段接：Windows 上真实的位置以 `\\?\` 开头，那里 `/` 不算分隔符），
/// 别的照原样。家目录不知道的换不开。
fn expanded(path: &str, home: Option<&Path>) -> Option<PathBuf> {
    match miyu_fs::tilde(path) {
        Some(rest) => home.map(|home| {
            rest.split(['/', '\\'])
                .filter(|part| !part.is_empty())
                .fold(home.to_path_buf(), |path, part| path.join(part))
        }),
        None => Some(PathBuf::from(path)),
    }
}
