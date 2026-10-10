//! 一层配置的一份文件（`docs/blueprint/config.md`「怎么走」第二条第 2 到 4 条）：在哪、版本、读好的项、整份的问题。
//!
//! 读不进来的（读不了、太大、不是 UTF-8、TOML 写法不对）记一条整份的问题：起来时就读不好的照空的算；重读时读不好的照上一次
//! 读好的用（[`File::keeping`]，施工 8-4）。写错的项在解析时已经丢掉，别的照用（G8）。

use std::path::{Path, PathBuf};

use miyu_config::parse::{Parsed, parse};
use miyu_config::problem::{Code, Problem, Severity};
use miyu_config::{Item, Layer};
use miyu_store::config_file::{self, ConfigText, ReadError};

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
    /// 字，开头的 BOM 去掉了；还没有、读不进来的是空的（施工 8-3：改一项在它上面改）。
    pub(crate) text: String,
    /// 开头有没有 BOM：写回时照样加回。
    pub(crate) bom: bool,
    /// 读好的项和一项一项的问题；整份读不进来的是空的。
    pub(crate) parsed: Parsed,
    /// 整份的问题。
    pub(crate) broken: Option<Problem>,
    /// 读不进来、`parsed` 是上一次读好的那一份（施工 8-4）：问题的「现在照什么用着」说 `last_good`，不说 `nothing`。
    pub(crate) last_good: bool,
}

impl File {
    /// 照清单 `items` 读 `layer` 这一层的 `path`，写法是 `shown`。
    pub(crate) fn read(items: &[Item], layer: Layer, path: PathBuf, shown: String) -> File {
        let mut file = File::nothing(layer, &path, &shown);
        match config_file::read(&file.path) {
            Ok(Some(text)) => File::of(items, file, text),
            Ok(None) => file,
            Err(error) => {
                file.broken = Some(broken(layer, &error));
                file
            }
        }
    }

    /// 照清单 `items` 认读好的字 `text`：写成了以后记下新的字当「上一次读的」（第五条第 8 条），不再读一次盘。
    pub(crate) fn written(items: &[Item], old: &File, text: ConfigText) -> File {
        File::of(items, File::nothing(old.layer, &old.path, &old.shown), text)
    }

    /// 什么都没读的一层：核心没给配置的时候（测试里）。
    pub(crate) fn nothing(layer: Layer, path: &Path, shown: &str) -> File {
        File {
            layer,
            path: path.to_path_buf(),
            shown: shown.to_string(),
            version: None,
            text: String::new(),
            bom: false,
            parsed: Parsed::default(),
            broken: None,
            last_good: false,
        }
    }

    /// 重读的这一份读不进来的，照上一次读好的 `old` 用（第二条第 4 条）：项照 `old` 的，问题、版本、字照这一次的。读得进来
    /// 的原样交回。`old` 自己也是起来时就读不好的（照空的），还是空的。
    pub(crate) fn keeping(mut self, old: &File) -> File {
        if self.broken.is_some() {
            self.parsed = Parsed {
                problems: Vec::new(),
                ..old.parsed.clone()
            };
            self.last_good = old.broken.is_none() || old.last_good;
        }
        self
    }

    /// 照新的清单 `items` 重新认手里的字（施工 F-5 补：装卸软件包以后），不重读盘：版本、字照旧。整份读不进来的、还没有的
    /// 照旧。
    pub(crate) fn refit(&self, items: &[Item]) -> File {
        let mut file = self.clone();
        if self.broken.is_some() || self.version.is_none() {
            return file;
        }
        match parse(items, self.layer, &self.text) {
            Ok(parsed) => file.parsed = parsed,
            Err(problem) => file.broken = Some(*problem),
        }
        file
    }

    /// 和 `other` 比，读到的是不是同一份：版本一样、整份的问题一样（读不了的没有版本，照问题比）。
    pub(crate) fn same_as(&self, other: &File) -> bool {
        self.version == other.version && self.broken == other.broken
    }

    /// 在空的一份 `file` 上记下读好的字，解析。
    fn of(items: &[Item], mut file: File, text: ConfigText) -> File {
        file.version = Some(text.version);
        file.bom = text.bom;
        match parse(items, file.layer, &text.text) {
            Ok(parsed) => file.parsed = parsed,
            Err(problem) => file.broken = Some(*problem),
        }
        file.text = text.text;
        file
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
