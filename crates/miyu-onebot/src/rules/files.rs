//! 读文件（`onebot.md` 第一条「场所规则和出厂数据」第 1 到 3 条，施工 O-21）：一份文件照配置文件的读法读
//! （[`config_file::read`]：超过 1 MiB 不读、开头的 BOM 去掉、不是 UTF-8 不读），读不成的变成群聊内核的 [`Problem`]；列出
//! `venues.d/` 里的规则文件；系统的两处这一刻的样子（[`Stamp`]），变没变照它比。

use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use miyu_chat::{File, Problem, Source};
use miyu_config::problem::Code;
use miyu_store::config_file::{self, ReadError};

/// 规则文件的名字以它结尾。
const EXTENSION: &str = ".toml";

/// 读一份文件 `path`：没有这个文件的是空的。
///
/// # Errors
///
/// 读不了、超过 1 MiB、不是 UTF-8：照 [`trouble`] 记成问题。
pub(super) fn read(path: &Path) -> Result<Option<String>, ReadError> {
    config_file::read(path).map(|read| read.map(|read| read.text))
}

/// 读不成的一份（文件名 `name`）记成一条整份文件的问题：`unreadable` 带系统的原话。
pub(super) fn trouble(error: ReadError, source: Source, name: &str) -> Problem {
    match error {
        ReadError::Unreadable(error) => {
            whole(Code::Unreadable, source, name, Some(error.to_string()))
        }
        ReadError::TooBig => whole(Code::TooBig, source, name, None),
        ReadError::NotUtf8 => whole(Code::NotUtf8, source, name, None),
    }
}

/// 整份文件的一条问题：没有第几条规则、键、位置、原文。
pub(super) fn whole(code: Code, source: Source, file: &str, why: Option<String>) -> Problem {
    Problem {
        code,
        source,
        file: file.to_string(),
        rule: None,
        key: None,
        at: None,
        got: None,
        suggest: None,
        why,
    }
}

/// 读 `dir`（目录名 `name`，问题里写它）里的规则文件，交给群聊内核的样子。读不成的那一份记一条问题、照空的字交（照样替换
/// 同名的出厂那一份，「施工时定的」第 50 条）。系统的目录不在是没写规则；列不出来的（出厂的目录不在也是）记一条
/// `unreadable`，一份也没有。
pub(super) fn rules(
    dir: &Path,
    name: &str,
    source: Source,
    problems: &mut Vec<Problem>,
) -> Vec<File> {
    let listed = match listed(dir) {
        Ok(listed) => listed,
        Err(error) if source == Source::System && error.kind() == io::ErrorKind::NotFound => {
            Vec::new()
        }
        Err(error) => {
            problems.push(whole(
                Code::Unreadable,
                source,
                name,
                Some(error.to_string()),
            ));
            Vec::new()
        }
    };
    let mut files = Vec::new();
    for (name, path) in listed {
        let text = match read(&path) {
            Ok(Some(text)) => text,
            // 列出来以后被删了：当没有。
            Ok(None) => continue,
            Err(error) => {
                problems.push(trouble(error, source, &name));
                String::new()
            }
        };
        files.push(File { source, name, text });
    }
    files
}

/// `dir` 里的规则文件（名字、路径），照名字排：名字以 `.toml` 结尾、不以 `.` 开头（编辑器的锁文件、隐藏文件不算）的普通
/// 文件，跟着链接；名字不是 UTF-8 的不认（「施工时定的」第 51 条）。
///
/// # Errors
///
/// 列不出这个目录（不在、不是目录、没有权限）。
fn listed(dir: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(String::from) else {
            continue;
        };
        let path = entry.path();
        if name.ends_with(EXTENSION) && !name.starts_with('.') && path.is_file() {
            files.push((name, path));
        }
    }
    files.sort();
    Ok(files)
}

/// 系统的两处这一刻的样子（第 3 条）：`venues.d/` 里每一份规则文件、违规词表，各自的路径和 [`Seen`]（不在的没有）；列不出
/// `venues.d/` 的，记这个目录本身。和上一次的一样就不用重读。
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Stamp(Vec<(PathBuf, Option<Seen>)>);

/// 一份文件看到的样子：修改时刻（系统给不出的没有）和大小。大小顺手一起比：修改时刻粗的文件系统上，同一刻里改了多半也
/// 看得出（「施工时定的」第 53 条）。
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Seen {
    /// 修改时刻。
    modified: Option<SystemTime>,
    /// 多少字节。
    len: u64,
}

impl Stamp {
    /// 规则的目录 `venues` 和违规词表 `words` 这一刻的样子。
    pub(super) fn of(venues: &Path, words: &Path) -> Stamp {
        let mut paths: Vec<PathBuf> = match listed(venues) {
            Ok(listed) => listed.into_iter().map(|(_, path)| path).collect(),
            Err(_) => vec![venues.to_path_buf()],
        };
        paths.push(words.to_path_buf());
        Stamp(
            paths
                .into_iter()
                .map(|path| {
                    let seen = std::fs::metadata(&path).ok().map(|metadata| Seen {
                        modified: metadata.modified().ok(),
                        len: metadata.len(),
                    });
                    (path, seen)
                })
                .collect(),
        )
    }
}
