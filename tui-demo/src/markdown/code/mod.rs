//! 代码着色（蓝图 `tui.md`「代码着色」，照旧版 `render/code.rs` 起步）：词法上的近似，不做语法分析。认关键字、
//! 函数名、字符串、数字、注释、类型、常量、宏和属性、变量和参数、键、运算符、diff 的加减行。
//! 各语言认什么写在 `resources/code.json`；一个代码块从头到尾用一个 [`Highlighter`]，块注释、三引号字符串接到下一行。
//! 一行里怎么认在 `lexer.rs`。

mod lexer;

#[cfg(test)]
mod tests;

use std::collections::HashSet;

use serde::Deserialize;

use super::inline::Piece;
use crate::theme;

/// `code.json` 的样子。
#[derive(Debug, Clone, Deserialize)]
pub struct Languages {
    /// 一种语言一条。
    pub languages: Vec<Language>,
}

/// 一种语言认什么。没写的都是不认。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Language {
    /// 名字和别名，照代码块开头写的那个认，不分大小写。
    pub names: Vec<String>,
    /// 关键字。
    #[serde(default)]
    pub keywords: Vec<String>,
    /// 关键字不分大小写（SQL、Dockerfile）。
    #[serde(default)]
    pub ignore_case: bool,
    /// 内建类型（`i32`、`str`、`int`）。
    #[serde(default)]
    pub types: Vec<String>,
    /// 常量（`true`、`None`、`nil`）。
    #[serde(default)]
    pub constants: Vec<String>,
    /// 大写开头、带小写的名字当类型（`Vec`、`String`）。
    #[serde(default)]
    pub capital_types: bool,
    /// 行注释的记号；空的是没有。
    #[serde(default)]
    pub comment: String,
    /// 块注释的头和尾（`/*`、`*/`），能跨行。
    #[serde(default)]
    pub block_comment: Option<[String; 2]>,
    /// 单引号括起来的也是字符串。
    #[serde(default)]
    pub single_quote_strings: bool,
    /// 反引号括起来的也是字符串。
    #[serde(default)]
    pub backtick_strings: bool,
    /// 三引号字符串（Python），能跨行。
    #[serde(default)]
    pub triple_quote_strings: bool,
    /// `名字!` 是宏（Rust）。
    #[serde(default)]
    pub macros: bool,
    /// `#[…]` 是属性（Rust）。
    #[serde(default)]
    pub attributes: bool,
    /// 一行打头的 `#名字` 是预处理指令（C/C++ 的 `#include`、`#define`）。
    #[serde(default)]
    pub preprocessor: bool,
    /// `@名字` 是装饰器、注解。
    #[serde(default)]
    pub decorators: bool,
    /// `$名字`、`${…}`、`$(…)` 是变量（shell、PHP、Makefile）。
    #[serde(default)]
    pub variables: bool,
    /// `-v`、`--force` 是命令的参数（shell）。
    #[serde(default)]
    pub flags: bool,
    /// 键和值之间的记号（`:`、`=`）：一行打头（或 `{`、`,`、`-` 后面）紧跟这个记号的名字、字符串是键。空的是不认键。
    #[serde(default)]
    pub keys: String,
    /// 一行只有 `[段]` 的是段名（TOML、INI）。
    #[serde(default)]
    pub sections: bool,
    /// `<标签 属性="…">`（HTML、XML）。
    #[serde(default)]
    pub tags: bool,
    /// diff：`+` 开头的行是加的、`-` 开头的是删的。
    #[serde(default)]
    pub diff: bool,
}

impl Languages {
    /// 照代码块开头写的语言找；没登记的是 `None`。
    pub fn find(&self, name: &str) -> Option<&Language> {
        let name = name.trim().to_lowercase();
        self.languages
            .iter()
            .find(|l| l.names.iter().any(|n| n.to_lowercase() == name))
    }
}

/// 走到哪了：上一行留下的块注释、三引号字符串接到这一行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// 平常。
    Code,
    /// 在块注释里。
    Comment,
    /// 在三引号字符串里，引号是哪种。
    Triple(char),
}

/// 一个代码块的着色：一行一行喂，记着上一行走到哪了。
pub struct Highlighter<'a> {
    language: Option<&'a Language>,
    keywords: HashSet<String>,
    types: HashSet<&'a str>,
    constants: HashSet<&'a str>,
    state: State,
}

impl<'a> Highlighter<'a> {
    /// 照这种语言着色；没登记的（`None`）不上色。
    pub fn new(language: Option<&'a Language>) -> Self {
        let fold = |w: &str| {
            if language.is_some_and(|l| l.ignore_case) {
                w.to_lowercase()
            } else {
                w.to_string()
            }
        };
        Self {
            language,
            keywords: language.map_or_else(HashSet::new, |l| {
                l.keywords.iter().map(|k| fold(k)).collect()
            }),
            types: language.map_or_else(HashSet::new, |l| {
                l.types.iter().map(String::as_str).collect()
            }),
            constants: language.map_or_else(HashSet::new, |l| {
                l.constants.iter().map(String::as_str).collect()
            }),
            state: State::Code,
        }
    }

    /// 一行着好色的片段，拼起来就是这一行。
    pub fn line(&mut self, line: &str) -> Vec<Piece> {
        // 没写语言、没登记的语言不上色（`tui.md`「代码着色」第 3 条）。
        if self.language.is_none() {
            return plain(line);
        }
        if self.language.is_some_and(|l| l.diff) {
            return diff_line(line);
        }
        self.scan(line)
    }

    /// 这个名字是不是关键字（照语言分不分大小写）。
    fn is_keyword(&self, word: &str) -> bool {
        if self.language.is_some_and(|l| l.ignore_case) {
            self.keywords.contains(&word.to_lowercase())
        } else {
            self.keywords.contains(word)
        }
    }
}

/// 不上色的一行：照原色。
fn plain(line: &str) -> Vec<Piece> {
    if line.is_empty() {
        return Vec::new();
    }
    vec![Piece {
        text: line.to_string(),
        style: ratatui::style::Style::new(),
        link: None,
    }]
}

/// diff 的一行：文件头和 `@@` 宏的颜色，`+` 加的颜色，`-` 删的颜色，别的原色。
fn diff_line(line: &str) -> Vec<Piece> {
    if line.is_empty() {
        return Vec::new();
    }
    let style = if line.starts_with("+++") || line.starts_with("---") || line.starts_with("@@") {
        theme::code_macro()
    } else if line.starts_with('+') {
        theme::added()
    } else if line.starts_with('-') {
        theme::removed()
    } else {
        ratatui::style::Style::new()
    };
    vec![Piece::new(line, style)]
}
