//! 一层配置的一份文件（`docs/blueprint/config.md`「怎么走」第二条第 2 到 4 条）：在哪、版本、读好的项、整份的问题。
//!
//! 读不进来的（读不了、太大、不是 UTF-8、TOML 写法不对）记一条整份的问题，这一层照空的算：起来时就读不好的没有「上一次
//! 读好的」可用（上一次读好的随 8-4 的重读）。写错的项在解析时已经丢掉，别的照用（G8）。

use std::path::{Path, PathBuf};

use miyu_config::parse::{Parsed, parse};
use miyu_config::problem::{Code, Problem, Severity};
use miyu_config::{Item, Layer};
use miyu_store::config_file::{self, ReadError};

/// 读好的一份。
#[derive(Debug, Clone)]
pub(crate) struct File {
    /// 哪一层。
    pub(crate) layer: Layer,
    /// 在哪：真的路径。
    pub(crate) path: PathBuf,
    /// 给人、给协议看的写法：数据根里的写成相对数据根的，项目配置写成 `~/…`。
    pub(crate) shown: String,
    /// 版本；文件还没有的是空的。
    pub(crate) version: Option<String>,
    /// 读好的项和一项一项的问题；整份读不进来的是空的。
    pub(crate) parsed: Parsed,
    /// 整份的问题。
    pub(crate) broken: Option<Problem>,
}

impl File {
    /// 照清单 `items` 读 `layer` 这一层的 `path`，写法是 `shown`。
    pub(crate) fn read(items: &[Item], layer: Layer, path: PathBuf, shown: String) -> File {
        let mut file = File {
            layer,
            path,
            shown,
            version: None,
            parsed: Parsed::default(),
            broken: None,
        };
        let text = match config_file::read(&file.path) {
            Ok(Some(text)) => text,
            Ok(None) => return file,
            Err(error) => {
                file.broken = Some(broken(layer, &error));
                return file;
            }
        };
        file.version = Some(text.version);
        match parse(items, layer, &text.text) {
            Ok(parsed) => file.parsed = parsed,
            Err(problem) => file.broken = Some(*problem),
        }
        file
    }

    /// 什么都没读的一层：核心没给配置的时候（测试里）。
    pub(crate) fn nothing(layer: Layer, path: &Path, shown: &str) -> File {
        File {
            layer,
            path: path.to_path_buf(),
            shown: shown.to_string(),
            version: None,
            parsed: Parsed::default(),
            broken: None,
        }
    }

    /// 这份文件的全部问题：整份的在前。
    pub(crate) fn problems(&self) -> impl Iterator<Item = &Problem> {
        self.broken.iter().chain(&self.parsed.problems)
    }

    /// 几处错误、几处警告。
    pub(crate) fn counts(&self) -> (usize, usize) {
        self.problems().fold((0, 0), |(errors, warnings), problem| {
            match problem.severity() {
                Severity::Error => (errors + 1, warnings),
                Severity::Warning => (errors, warnings + 1),
            }
        })
    }
}

/// 读不进来的一份：整份的问题。
fn broken(layer: Layer, error: &ReadError) -> Problem {
    match error {
        ReadError::Unreadable(why) => Problem::file(Code::Unreadable, layer, Some(why.to_string())),
        ReadError::TooBig => Problem::file(Code::TooBig, layer, None),
        ReadError::NotUtf8 => Problem::file(Code::NotUtf8, layer, None),
    }
}
