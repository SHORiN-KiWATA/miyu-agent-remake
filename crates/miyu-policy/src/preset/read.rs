//! 预设文件怎么读（施工 P-2 上；施工 F-3 上从 `preset.rs` 挪出来：那边放不下了）：`[preset]`、`[features]`、`[software]`、
//! `[tools]` 四张表，写错的报第一处，带代码、第几行。

use miyu_config::phrases::{self, Label, PhraseError};
use miyu_config::secret::valid_name;
use std::fmt;
use toml_edit::{Document, Item, TableLike};

use super::{PresetFile, TOOL_CHARS, Unlisted};

/// 预设文件写错了：第几行（从 1 数，说不出的没有）、哪一种错、错的那一处，和一句英文短句（日志、协议的 `data.problem` 用）。
/// 给人看的那一句照 `code` 和 `detail` 用 `core/human/<语言>.json` 的 `preset-problems/<code>` 写。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// 第几行。
    pub line: Option<usize>,
    /// 哪一种错。
    pub code: Code,
    /// 错的那一处：表名、`<表>.<键>`、`preset.<格>.<语言>`；读不成 TOML 的是它的原话。
    pub detail: String,
    /// 错在哪，英文短句。
    pub message: String,
}

/// 撤掉了的默认人格那一格的键（施工 P-4 上，2026-10-08 项目主人：只去掉预设的「默认人格」）：读的时候当没写，写的时候去掉，
/// `preset.set` 写它是参数不对。
pub const DEFAULT_PERSONA: &str = "default_persona";

/// 预设文件错在哪一种。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    /// 读不成 TOML。
    Syntax,
    /// 多了 `[preset]`、`[features]`、`[software]`、`[tools]` 以外的表。
    UnknownTable,
    /// 这四样有一样不是表。
    NotATable,
    /// `[preset]` 里多了别的键。
    UnknownKey,
    /// `name`、`summary` 不是一句字，也不是语言到一句话的表（以前的写法）。
    NotPhrases,
    /// 语言不是 `zh`、`en`、`ja`。
    UnknownLanguage,
    /// 一句话是空的、不是字。
    EmptyPhrase,
    /// `unlisted` 不是 `on`、`off`。
    BadUnlisted,
    /// `[software]` 的键不是合写法的软件包编号。
    BadSoftware,
    /// `[features]`、`[software]` 的值不是开关。
    NotBool,
    /// `[tools]` 的键不是工具名的写法。
    BadTool,
    /// `[tools]` 的值不是 `false`：单件打开某个包里的一件先不做。
    NotFalse,
    /// `[features]` 的键不是合写法的功能编号（施工 F-3 上）。
    BadFeature,
    /// `icon` 不是 Lucide 图标名的写法（施工 P-5）。
    BadIcon,
}

impl Code {
    /// 稳定的写法：协议、给人看的字的键用它。
    pub fn as_str(self) -> &'static str {
        match self {
            Code::Syntax => "syntax",
            Code::UnknownTable => "unknown_table",
            Code::NotATable => "not_a_table",
            Code::UnknownKey => "unknown_key",
            Code::NotPhrases => "not_phrases",
            Code::UnknownLanguage => "unknown_language",
            Code::EmptyPhrase => "empty_phrase",
            Code::BadUnlisted => "bad_unlisted",
            Code::BadSoftware => "bad_software",
            Code::NotBool => "not_bool",
            Code::BadTool => "bad_tool",
            Code::NotFalse => "not_false",
            Code::BadFeature => "bad_feature",
            Code::BadIcon => "bad_icon",
        }
    }

    /// 全部，照先后：给人看的字的门禁照它查三种语言都有。
    pub const ALL: [Code; 14] = [
        Code::Syntax,
        Code::UnknownTable,
        Code::NotATable,
        Code::UnknownKey,
        Code::NotPhrases,
        Code::UnknownLanguage,
        Code::EmptyPhrase,
        Code::BadUnlisted,
        Code::BadSoftware,
        Code::NotBool,
        Code::BadTool,
        Code::NotFalse,
        Code::BadFeature,
        Code::BadIcon,
    ];
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "{line}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

/// 读一份预设文件：`[preset]`、`[features]`、`[software]`、`[tools]` 四张表，都可以没有。
///
/// # Errors
///
/// 读不成 TOML、多了别的表或者键、哪一格的值不合写法，报第一处。
pub fn read(text: &str) -> Result<PresetFile, Problem> {
    let document = Document::parse(text).map_err(|error| Problem {
        line: error.span().map(|span| line_of(text, span.start)),
        code: Code::Syntax,
        detail: error.message().trim().to_string(),
        message: error.message().trim().to_string(),
    })?;
    let reader = Reader { text };
    let mut file = PresetFile::default();
    for (key, item) in document.as_table().iter() {
        match key {
            "preset" => reader.preset(reader.table(key, item)?, &mut file)?,
            "features" => {
                let switches = reader.switches(key, item, Code::BadFeature, "a feature id")?;
                file.features.extend(switches);
            }
            "software" => {
                let switches = reader.switches(key, item, Code::BadSoftware, "a package id")?;
                file.software.extend(switches);
            }
            "tools" => {
                for (name, item) in reader.table(key, item)?.iter() {
                    if !tool_name(name) {
                        return Err(reader.problem(
                            item,
                            Code::BadTool,
                            &format!("tools.{name}"),
                            format!("tools.{name}: a tool name uses only letters, digits, - and _"),
                        ));
                    }
                    if item.as_bool() != Some(false) {
                        return Err(reader.problem(
                            item,
                            Code::NotFalse,
                            &format!("tools.{name}"),
                            format!("tools.{name} can only be false"),
                        ));
                    }
                    file.tools_off.insert(name.to_string());
                }
            }
            other => {
                return Err(reader.problem(
                    item,
                    Code::UnknownTable,
                    other,
                    format!("unknown table [{other}]"),
                ));
            }
        }
    }
    Ok(file)
}

/// 读的时候带着原文，好说第几行。
struct Reader<'a> {
    text: &'a str,
}

impl Reader<'_> {
    /// `[features]`、`[software]`（叫 `key`）：编号照包编号的写法、不合的报 `bad`，值是开关（施工 F-3 上从 `[software]` 那一段
    /// 抽出来，两张表一个读法）。`what` 是英文那一句里怎么称呼编号。
    fn switches(
        &self,
        key: &str,
        item: &Item,
        bad: Code,
        what: &str,
    ) -> Result<Vec<(String, bool)>, Problem> {
        let mut switches = Vec::new();
        for (name, item) in self.table(key, item)?.iter() {
            if !valid_name(name) {
                return Err(self.problem(
                    item,
                    bad,
                    &format!("{key}.{name}"),
                    format!("{key}.{name}: {what} starts with a lowercase letter and uses only lowercase letters, digits, - and _"),
                ));
            }
            let on = item.as_bool().ok_or_else(|| {
                self.problem(
                    item,
                    Code::NotBool,
                    &format!("{key}.{name}"),
                    format!("{key}.{name} must be true or false"),
                )
            })?;
            switches.push((name.to_string(), on));
        }
        Ok(switches)
    }

    /// `[preset]` 那一张表。
    fn preset(&self, table: &dyn TableLike, file: &mut PresetFile) -> Result<(), Problem> {
        for (key, item) in table.iter() {
            match key {
                "name" => file.name = Some(self.label(key, item)?),
                "summary" => file.summary = Some(self.label(key, item)?),
                "icon" => {
                    file.icon = Some(
                        item.as_str()
                            .filter(|icon| icon_name(icon))
                            .map(str::to_string)
                            .ok_or_else(|| {
                                self.problem(
                                    item,
                                    Code::BadIcon,
                                    "preset.icon",
                                    "preset.icon must be a Lucide icon name".to_string(),
                                )
                            })?,
                    );
                }
                // P-3 上那几个小时里写进去的「以谁为底」（施工 P-3 再补）、P-4 上撤掉的默认人格：认出来就当没写，下一次写这份
                // 文件时去掉。
                crate::persona::BASE | DEFAULT_PERSONA => {}
                "unlisted" => {
                    file.unlisted = Some(match item.as_str() {
                        Some("on") => Unlisted::On,
                        Some("off") => Unlisted::Off,
                        _ => {
                            return Err(self.problem(
                                item,
                                Code::BadUnlisted,
                                "preset.unlisted",
                                "preset.unlisted must be on or off".to_string(),
                            ));
                        }
                    });
                }
                other => {
                    return Err(self.problem(
                        item,
                        Code::UnknownKey,
                        &format!("preset.{other}"),
                        format!("unknown key preset.{other}"),
                    ));
                }
            }
        }
        Ok(())
    }

    /// 该是表的：不是的报 `not_a_table`。
    fn table<'i>(&self, key: &str, item: &'i Item) -> Result<&'i dyn TableLike, Problem> {
        item.as_table_like().ok_or_else(|| {
            self.problem(item, Code::NotATable, key, format!("{key} must be a table"))
        })
    }

    /// 名字、说明：一句字，或者以前的语言表。
    fn label(&self, field: &str, item: &Item) -> Result<Label, Problem> {
        let line =
            |span: Option<std::ops::Range<usize>>| span.map(|span| line_of(self.text, span.start));
        // 说明可以是空的字（施工 P-3 再补）：没有说明，盖住下面那一层的。
        let read = match field {
            "summary" => phrases::read_summary,
            _ => phrases::read_label,
        };
        read(item).map_err(|error| match error {
            PhraseError::NotPhrases(span) => Problem {
                line: line(span),
                code: Code::NotPhrases,
                detail: format!("preset.{field}"),
                message: format!("preset.{field} must be text"),
            },
            PhraseError::Empty(language, span) if language.is_empty() => Problem {
                line: line(span),
                code: Code::EmptyPhrase,
                detail: format!("preset.{field}"),
                message: format!("preset.{field} must be non-empty text"),
            },
            PhraseError::UnknownLanguage(language, span) => Problem {
                line: line(span),
                code: Code::UnknownLanguage,
                detail: format!("preset.{field}.{language}"),
                message: format!("preset.{field}.{language}: language must be zh, en or ja"),
            },
            PhraseError::Empty(language, span) => Problem {
                line: line(span),
                code: Code::EmptyPhrase,
                detail: format!("preset.{field}.{language}"),
                message: format!("preset.{field}.{language} must be non-empty text"),
            },
        })
    }

    fn problem(&self, item: &Item, code: Code, detail: &str, message: String) -> Problem {
        Problem {
            line: item.span().map(|span| line_of(self.text, span.start)),
            code,
            detail: detail.to_string(),
            message,
        }
    }
}

/// 工具名：字母、数字、`-`、`_`，1 到 64 个。
fn tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= TOOL_CHARS
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// 字节位置 `offset` 在第几行。
fn line_of(text: &str, offset: usize) -> usize {
    text.as_bytes()
        .iter()
        .take(offset)
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}

/// Lucide 图标名的写法（施工 P-5，同软件包清单的 `icon`）：小写字母开头，只有小写字母、数字、`-`，最多 64 个。核心只查写法，
/// 不查 Lucide 里有没有。
fn icon_name(name: &str) -> bool {
    name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
