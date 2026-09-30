//! 项目配置的信任（`docs/blueprint/config.md`「怎么走」第三条第 2、3 条，G3）：读 `home/<账号>/trust.toml`，照仓库在哪、
//! 内容的版本认一份项目配置信不信任（施工 8-2）；记下一个回答（[`recorded`]，`config.trust` 用，施工 8-3）。
//!
//! 一个仓库一条 `[[project]]`：`path` 仓库在哪（`.miyu` 所在的那一层，家目录下的写成 `~/…`），`version` 答的是哪一份，
//! `trusted` 信不信任。同一个仓库有几条的，后面的盖掉前面的。写法不对的那一条不算。整份读不进来的，照没有记录，由
//! 调用的一方记一条 `WARN`。
//!
//! 记一个回答照改配置的规矩：同一个仓库已经有一条的，原地换它的 `version`、`trusted`，别的字节一个不动；没有的在末尾
//! 加一条，前面空一行。新建的文件先写一行开头的注释（`config/trust-header`）。

use std::ops::Range;
use std::path::{Path, PathBuf};

use toml_edit::Document;

use miyu_config::Value;
use miyu_config::edit::newline;
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
    match config_file::read(path).map_err(|error| error.to_string())? {
        Some(text) => records(&text.text),
        None => Ok(Vec::new()),
    }
}

/// 一份记录的字里的每一条。
///
/// # Errors
///
/// TOML 写法不对：原因。
pub(crate) fn records(text: &str) -> Result<Vec<Record>, String> {
    let document = Document::parse(text).map_err(|error| error.message().to_string())?;
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

/// 手改了记录以后，哪几个仓库的回答变了（施工 8-4）：`new` 里每个仓库（照写的 `path` 认）算数的那一条（最后一条），和
/// `old` 里的不一样、或者 `old` 里没有的，照在 `new` 里的先后。删掉了的仓库不算：删掉就是还没问过，不是一个回答。
pub(crate) fn changed(old: &[Record], new: &[Record]) -> Vec<Record> {
    let last = |records: &[Record], path: &str| -> Option<Record> {
        records
            .iter()
            .rev()
            .find(|record| record.path == path)
            .cloned()
    };
    let mut changed: Vec<Record> = Vec::new();
    for record in new {
        if changed.iter().any(|seen| seen.path == record.path) {
            continue;
        }
        let now = last(new, &record.path);
        if now != last(old, &record.path)
            && let Some(now) = now
        {
            changed.push(now);
        }
    }
    changed
}

/// 一个回答。
pub(crate) struct Answer<'a> {
    /// 仓库在哪，真实的位置：找已有的那一条照它比。
    pub(crate) repo: &'a Path,
    /// 写进去的写法：家目录下的写成 `~/…`。
    pub(crate) shown: &'a str,
    /// 答的是哪一份。
    pub(crate) version: &'a str,
    /// 信不信任。
    pub(crate) trusted: bool,
}

/// 在记录的字 `text`（文件还没有的是空的，新建时开头写 `header` 那一行注释）上记下 `answer`，交回新的字。`home` 是系统的
/// 家目录，换开记录里的 `~`。
///
/// # Errors
///
/// 字读不懂、记完读不懂（手改坏了的）：原因。
pub(crate) fn recorded(
    text: Option<&str>,
    header: &str,
    answer: &Answer<'_>,
    home: Option<&Path>,
) -> Result<String, String> {
    let Some(text) = text else {
        return Ok(format!("# {header}\n{}", block(answer, "\n")));
    };
    let document = Document::parse(text).map_err(|error| error.message().to_string())?;
    let found = document
        .get("project")
        .and_then(|item| item.as_array_of_tables())
        .and_then(|projects| {
            projects
                .iter()
                .filter_map(|table| {
                    let path = table.get("path")?.as_str()?;
                    let same = same_repo(path, answer.repo, home);
                    let spans = (value_span(table, "version")?, value_span(table, "trusted")?);
                    same.then_some(spans)
                })
                .last()
        });
    let edited = match found {
        Some((version, trusted)) => {
            // 后面的先换：前面的位置不跟着挪。
            let quoted = Value::Text(answer.version.to_string().into()).toml();
            let (first, second) = match version.start < trusted.start {
                true => ((version, quoted), (trusted, answer.trusted.to_string())),
                false => ((trusted, answer.trusted.to_string()), (version, quoted)),
            };
            splice(&splice(text, second.0, &second.1), first.0, &first.1)
        }
        None => {
            let nl = newline(text);
            let mut out = text.to_string();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push_str(nl);
            }
            if !out.is_empty() {
                out.push_str(nl);
            }
            out.push_str(&block(answer, nl));
            out
        }
    };
    Document::parse(edited.as_str()).map_err(|error| error.message().to_string())?;
    Ok(edited)
}

/// 一条 `[[project]]`。
fn block(answer: &Answer<'_>, nl: &str) -> String {
    let text = |value: &str| Value::Text(value.to_string().into()).toml();
    format!(
        "[[project]]{nl}path = {}{nl}version = {}{nl}trusted = {}{nl}",
        text(answer.shown),
        text(answer.version),
        answer.trusted
    )
}

/// 一张表里一格的值在哪。
fn value_span(table: &toml_edit::Table, key: &str) -> Option<Range<usize>> {
    table.get(key)?.as_value()?.span()
}

/// 把 `range` 那一段换成 `with`。
fn splice(text: &str, range: Range<usize>, with: &str) -> String {
    format!("{}{with}{}", &text[..range.start], &text[range.end..])
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
        .find(|record| same_repo(&record.path, repo, home));
    match found {
        Some(record) if record.version == version => match record.trusted {
            true => Trust::Trusted,
            false => Trust::Distrusted,
        },
        _ => Trust::Unknown,
    }
}

/// 记录里写的 `path` 是不是仓库 `repo`（真实的位置）。两边都去掉 Windows 真实位置开头的 `\\?\` 再比：记下的写法照给人
/// 看的（`project::shown` 去掉了它，施工 8-3），真实的位置带着它。
fn same_repo(path: &str, repo: &Path, home: Option<&Path>) -> bool {
    let plain = |path: &Path| {
        let text = path.to_string_lossy();
        text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
    };
    expanded(path, home).is_some_and(|path| plain(&path) == plain(repo))
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
