//! 报错（`docs/blueprint/config.md`「报错」、「怎么走」第四条，G8，施工 8-2）：读、校验配置时发现的每一处写成一条
//! [`Problem`]：原因码、在哪、哪一项、收到了什么、离得最近的键名。
//!
//! 这里的问题还没说成话：说成哪种语言由连接定，同一条问题要照几种语言说（[`tell()`]）。文件在哪也不在
//! 这里：解析只拿到字，文件的路径由拿着文件的一方（配置服务）接上。

mod tell;

pub use tell::{Told, Using, tell};

use crate::item::{Item, Layer};
use crate::value::Value;

/// 收到的原文最多照抄几个字符，多了截掉加 `…`（「报错」的 `got`）。
const GOT_CHARS: usize = 80;

/// 离得最近的键名最远差几个字（「怎么走」第四条第 2 条）。
const NEAREST: usize = 3;

/// 原因码（「报错」那张表里标 8-2 的几种）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Code {
    /// 文件读不了：没有权限、是个目录……（没有这个文件不算）。
    Unreadable,
    /// 文件超过 1 MiB。
    TooBig,
    /// 不是 UTF-8。
    NotUtf8,
    /// TOML 写法不对。
    Syntax,
    /// 清单里没有这一项：警告，原样留在文件里。
    UnknownKey,
    /// 类型不对，还有一组键下面写成了一个值（`ui = "zh"`）。
    WrongType,
    /// 选项不在列出的几个里。
    NotAnOption,
    /// 这一项不能写在这一层。
    WrongLayer,
    /// 项目配置写得比下面几层宽。
    NotTightening,
    /// 项目配置还没信任，或者信任以后内容变了：这一份先不用。警告。
    UntrustedProject,
}

impl Code {
    /// 协议上的写法，例如 `not_an_option`。
    pub fn as_str(self) -> &'static str {
        match self {
            Code::Unreadable => "unreadable",
            Code::TooBig => "too_big",
            Code::NotUtf8 => "not_utf8",
            Code::Syntax => "syntax",
            Code::UnknownKey => "unknown_key",
            Code::WrongType => "wrong_type",
            Code::NotAnOption => "not_an_option",
            Code::WrongLayer => "wrong_layer",
            Code::NotTightening => "not_tightening",
            Code::UntrustedProject => "untrusted_project",
        }
    }

    /// 是错误还是警告。
    pub fn severity(self) -> Severity {
        match self {
            Code::UnknownKey | Code::UntrustedProject => Severity::Warning,
            _ => Severity::Error,
        }
    }

    /// 整份文件的问题：TOML 读不懂，这份文件整份照上一次读好的用（第二条第 4 条）。
    pub fn whole_file(self) -> bool {
        matches!(
            self,
            Code::Unreadable | Code::TooBig | Code::NotUtf8 | Code::Syntax
        )
    }
}

/// 错误还是警告。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// 错误：`config_errors` 数它。
    Error,
    /// 警告。
    Warning,
}

impl Severity {
    /// 协议上的写法：`error`、`warning`。
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// 在哪：行、列从 1 数，列照 Unicode 字符数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct At {
    /// 第几行。
    pub line: usize,
    /// 第几列。
    pub column: usize,
}

impl At {
    /// 字 `text` 里第 `offset` 个字节在哪：之前有几个换行、这一行里之前有几个字符。`\r\n` 的 `\r` 在行尾，不影响列。
    pub fn of(text: &str, offset: usize) -> At {
        let before = text.get(..offset).unwrap_or(text);
        let line_start = before.rfind('\n').map_or(0, |at| at + 1);
        At {
            line: before.matches('\n').count() + 1,
            column: before[line_start..].chars().count() + 1,
        }
    }
}

/// 一处问题，还没说成话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// 原因码。
    pub code: Code,
    /// 哪一层的文件（或者当成哪一层查的字）。
    pub layer: Layer,
    /// 在哪。整份文件的问题（读不了、太大）没有。
    pub at: Option<At>,
    /// 哪一项的键：写错的那个键原样。整份文件的问题没有。
    pub key: Option<String>,
    /// 收到了什么：原文照抄，最多 80 个字符（[`got`]）。
    pub got: Option<String>,
    /// 为什么：读不了的系统原话、TOML 写法不对时 `toml_edit` 说的那一行。
    pub why: Option<String>,
    /// 键名拼错了：离得最近的那一个。
    pub suggest: Option<&'static str>,
    /// 下面几层合出来的值：`not_tightening` 说「现在是什么」。
    pub current: Option<Value>,
}

impl Problem {
    /// 整份文件的问题：`code` 是 [`Code::whole_file`] 那几种，`why` 是原话（没有的是空的）。
    pub fn file(code: Code, layer: Layer, why: Option<String>) -> Problem {
        Problem {
            code,
            layer,
            at: None,
            key: None,
            got: None,
            why,
            suggest: None,
            current: None,
        }
    }

    /// 一项的问题：`key` 在 `at`，收到的原文是 `raw`（截短见 [`got`]）。
    pub fn item(code: Code, layer: Layer, key: &str, at: At, raw: &str) -> Problem {
        Problem {
            code,
            layer,
            at: Some(at),
            key: Some(key.to_string()),
            got: Some(got(raw)),
            why: None,
            suggest: None,
            current: None,
        }
    }

    /// 错误还是警告。
    pub fn severity(&self) -> Severity {
        self.code.severity()
    }
}

/// 收到的原文：最多 80 个字符，多了截掉加 `…`。
pub fn got(raw: &str) -> String {
    match raw.char_indices().nth(GOT_CHARS) {
        Some((at, _)) => format!("{}…", &raw[..at]),
        None => raw.to_string(),
    }
}

/// 清单里和 `key` 离得最近的键（「怎么走」第四条第 2 条）：编辑距离照插入、删除、替换一个字，相邻两个字换位，都算 1；
/// 不超过 3、也不超过 `key` 长度的三分之一的才给；几个一样近的，取清单里排在前面的。
pub fn nearest(items: &[Item], key: &str) -> Option<&'static str> {
    let written: Vec<char> = key.chars().collect();
    let mut best: Option<(usize, &'static str)> = None;
    for item in items {
        let distance = distance(&written, &item.key.chars().collect::<Vec<_>>());
        let close = distance <= NEAREST && distance * 3 <= written.len();
        if close && best.is_none_or(|(so_far, _)| distance < so_far) {
            best = Some((distance, item.key));
        }
    }
    best.map(|(_, key)| key)
}

/// 编辑距离，相邻两个字换位算一次（optimal string alignment）。
fn distance(a: &[char], b: &[char]) -> usize {
    let width = b.len() + 1;
    let mut table = vec![0usize; (a.len() + 1) * width];
    let at = |i: usize, j: usize| i * width + j;
    for i in 0..=a.len() {
        table[at(i, 0)] = i;
    }
    for j in 0..=b.len() {
        table[at(0, j)] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (table[at(i - 1, j)] + 1)
                .min(table[at(i, j - 1)] + 1)
                .min(table[at(i - 1, j - 1)] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(table[at(i - 2, j - 2)] + 1);
            }
            table[at(i, j)] = best;
        }
    }
    table[at(a.len(), b.len())]
}

#[cfg(test)]
mod tests;
