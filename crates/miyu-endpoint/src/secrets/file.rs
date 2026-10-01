//! 手里的密钥文件（`docs/blueprint/config.md`「怎么走」第九条第 1 到 3 条，施工 8-5）：在哪、版本、读好的密钥、写错的
//! 几行、整份读不进来的问题。和配置文件的 `File` 一个样子：起来时读不进来的照空的，重读时读不进来的照上一次读好的用
//! （[`SecretsFile::keeping`]）。
//!
//! 字里就是密钥：`Debug` 不印字，只印版本和问题。

use std::fmt;
use std::path::{Path, PathBuf};

use miyu_config::Layer;
use miyu_config::problem::{Code, Problem, Severity};
use miyu_config::secret::{Stored, parse_file};
use miyu_store::config_file::ReadError;
use miyu_store::secrets;

/// 读好的一份密钥文件。
#[derive(Clone)]
pub(crate) struct SecretsFile {
    /// 在哪：真的路径。
    pub(crate) path: PathBuf,
    /// 给人、给协议看的写法：`system/secrets.toml`。
    pub(crate) shown: String,
    /// 版本；文件还没有的是空的。不交给头：它是整份密钥的哈希。
    pub(crate) version: Option<String>,
    /// 字，开头的 BOM 去掉了；还没有、读不进来的是空的（改一行在它上面改）。
    pub(crate) text: String,
    /// 开头有没有 BOM：写回时照样加回。
    pub(crate) bom: bool,
    /// Unix 上组、别人读得到。
    pub(crate) open: bool,
    /// 读好的密钥和写错的几行；整份读不进来的是空的。
    pub(crate) stored: Stored,
    /// 整份的问题。
    pub(crate) broken: Option<Problem>,
    /// 读不进来、`stored` 是上一次读好的那一份：问题说 `last_good`。
    pub(crate) last_good: bool,
}

impl fmt::Debug for SecretsFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretsFile")
            .field("shown", &self.shown)
            .field("version", &self.version)
            .field("names", &self.stored.entries.keys().collect::<Vec<_>>())
            .field("problems", &self.problems().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

impl SecretsFile {
    /// 读 `path`，写法是 `shown`。
    pub(crate) fn read(path: &Path, shown: &str) -> SecretsFile {
        let mut file = SecretsFile::nothing(path, shown);
        match secrets::read(path) {
            Ok(Some(read)) => {
                file.open = read.open;
                file.of(read.text.text, read.text.version, read.text.bom)
            }
            Ok(None) => file,
            Err(error) => {
                file.broken = Some(broken(&error));
                file
            }
        }
    }

    /// 什么都没读的：还没有这份文件、核心不给配置的时候。
    pub(crate) fn nothing(path: &Path, shown: &str) -> SecretsFile {
        SecretsFile {
            path: path.to_path_buf(),
            shown: shown.to_string(),
            version: None,
            text: String::new(),
            bom: false,
            open: false,
            stored: Stored::default(),
            broken: None,
            last_good: false,
        }
    }

    /// 写成了以后记下新的字当「上一次读的」，不再读一次盘：`version` 是写进去的字节的版本。
    pub(crate) fn written(&self, text: String, version: String) -> SecretsFile {
        SecretsFile::nothing(&self.path, &self.shown).of(text, version, self.bom)
    }

    /// 在空的一份上记下读好的字，解析。
    fn of(mut self, text: String, version: String, bom: bool) -> SecretsFile {
        self.version = Some(version);
        self.bom = bom;
        match parse_file(&text) {
            Ok(stored) => self.stored = stored,
            Err(problem) => self.broken = Some(*problem),
        }
        self.text = text;
        self
    }

    /// 重读的这一份读不进来的，照上一次读好的 `old` 用：密钥照 `old` 的，问题、版本、字照这一次的。
    pub(crate) fn keeping(mut self, old: &SecretsFile) -> SecretsFile {
        if self.broken.is_some() {
            self.stored = Stored {
                problems: Vec::new(),
                ..old.stored.clone()
            };
            self.last_good = old.broken.is_none() || old.last_good;
        }
        self
    }

    /// 和 `other` 比，读到的是不是同一份。
    pub(crate) fn same_as(&self, other: &SecretsFile) -> bool {
        self.version == other.version && self.broken == other.broken
    }

    /// 名字 `name` 的密钥设了没有。
    pub(crate) fn has(&self, name: &str) -> bool {
        self.stored.entries.contains_key(name)
    }

    /// 这份文件的全部问题：整份的在前。
    pub(crate) fn problems(&self) -> impl Iterator<Item = &Problem> {
        self.broken.iter().chain(&self.stored.problems)
    }

    /// 几处错误（`config_errors` 算上它）。
    pub(crate) fn errors(&self) -> usize {
        self.problems()
            .filter(|problem| problem.severity() == Severity::Error)
            .count()
    }
}

/// 读不进来的一份：整份的问题，照系统配置那一层记。
fn broken(error: &ReadError) -> Problem {
    match error {
        ReadError::Unreadable(why) => {
            Problem::file(Code::Unreadable, Layer::System, Some(why.to_string()))
        }
        ReadError::TooBig => Problem::file(Code::TooBig, Layer::System, None),
        ReadError::NotUtf8 => Problem::file(Code::NotUtf8, Layer::System, None),
    }
}
