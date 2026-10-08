//! 预设文件怎么读（施工 P-2 上，`docs/blueprint/presets.md`，`16-人格与预设.md` 第三节）：`[preset]` 的名字、说明、默认人格、
//! 没列出来的软件开不开，`[software]` 按软件包开关，`[tools]` 关掉单件工具。纯逻辑：进来的是文件里的字，出去的是读好的样子，
//! 或者写明第几行错在哪。找哪几层、读盘由存储做。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use miyu_config::phrases::{self, Label, PhraseError};
use miyu_config::secret::valid_name;
use miyu_kernel::id::ContentHash;
use serde::{Deserialize, Serialize};
use toml_edit::{Document, Item, TableLike};

/// 工具名最多几个字符。
const TOOL_CHARS: usize = 64;

/// 记忆这个软件（施工 P-2 中，`10-自带软件.md` 第四节）：三件工具和回合开始的召回。和 `miyu_memory::PACKAGE` 是同一个编号。
pub const MEMORY: &str = "memory";

/// 角色扮演这个软件（施工 P-2 中）：人格的角色扮演提示和风格锁（`16-人格与预设.md` 第八节：开发预设不开）。它没有工具。
pub const ROLEPLAY: &str = "roleplay";

/// 没列在 `[software]` 里的软件（包括以后新装的）开不开（Y7）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unlisted {
    /// 开：功能全开那种。
    On,
    /// 关：开发那种，只开列出来的。
    Off,
}

impl Unlisted {
    /// 文件、协议里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Unlisted::On => "on",
            Unlisted::Off => "off",
        }
    }
}

/// 一份预设文件读好的样子。每一格都可以没有：同名覆盖只写改了的。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PresetFile {
    /// 名字：一句字（施工 P-3 补），以前写成语言表的照样认。
    pub name: Option<Label>,
    /// 一句说明，写法同名字。
    pub summary: Option<Label>,
    /// 不指定人格时用哪个人格（人格的编号；在不在开会话时查）。
    pub default_persona: Option<String>,
    /// 没列出来的软件开不开；几层都没写的照 [`Unlisted::On`]（[`PresetFile::unlisted`]）。
    pub unlisted: Option<Unlisted>,
    /// 软件包的编号到开不开。
    pub software: BTreeMap<String, bool>,
    /// 关掉的单件工具（开着的包里的）。
    pub tools_off: BTreeSet<String>,
}

impl PresetFile {
    /// 叠在 `lower` 上面（同名覆盖，16 第四节）：逐格盖，名字、说明写了的整格换掉，`[software]` 逐个键盖，关掉的工具叠在一起。
    #[must_use]
    pub fn over(self, mut lower: PresetFile) -> PresetFile {
        lower.name = self.name.or(lower.name);
        lower.summary = self.summary.or(lower.summary);
        lower.default_persona = self.default_persona.or(lower.default_persona);
        lower.unlisted = self.unlisted.or(lower.unlisted);
        lower.software.extend(self.software);
        lower.tools_off.extend(self.tools_off);
        lower
    }

    /// 没列出来的软件开不开：几层都没写的是开。功能全开是默认，`[tools]` 里只关一两件的写法也是建在「其余都开」上的。
    pub fn unlisted(&self) -> Unlisted {
        self.unlisted.unwrap_or(Unlisted::On)
    }

    /// 软件 `software` 开不开（施工 P-2 中，Y7）：`[software]` 写了的照写的，没写的照 `unlisted`。
    pub fn opens(&self, software: &str) -> bool {
        self.software
            .get(software)
            .copied()
            .unwrap_or(self.unlisted() == Unlisted::On)
    }

    /// 包 `package` 里的工具 `tool` 留不留在工具面上：包开着，这一件也没被 `[tools]` 关掉（走查 C1）。
    pub fn keeps(&self, package: &str, tool: &str) -> bool {
        self.opens(package) && !self.tools_off.contains(tool)
    }

    /// 叠好的文件的指纹（施工 P-2 下）：记进快照，回合开始时执行器照它认出预设的文件改了。名字、说明照以前的写法算
    /// （施工 P-3 补：没写的是空表、语言表照原样，一句字的是那句字）：以前造的快照照旧对得上，开着的会话不白白换一次快照。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：几格总写得成 JSON。
    pub fn digest(&self) -> ContentHash {
        let fields = (
            label_json(self.name.as_ref()),
            label_json(self.summary.as_ref()),
            &self.default_persona,
            self.unlisted.map(Unlisted::as_str),
            &self.software,
            &self.tools_off,
        );
        ContentHash::of(&serde_json::to_vec(&fields).expect("预设的几格写得成 JSON"))
    }
}

/// 快照里记的预设（施工 P-2 中，`Snapshot::preset`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetPin {
    /// 预设的编号。
    pub id: String,
    /// 造会话时装了、这个预设没开的软件，照编号排。都开着的不写。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub off: Vec<String>,
    /// 叠好的文件的指纹（施工 P-2 下，[`PresetFile::digest`]）：回合开始时照它认出预设改了。P-2（中）造的没有：不换。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<ContentHash>,
}

/// 开会话时找好的预设（施工 P-2 中）：编号、叠好的文件，和这台机器上装了、这个预设没开的软件（照编号排；Y8 那一行照它写）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    /// 编号。
    pub id: String,
    /// 叠好的文件。
    pub file: PresetFile,
    /// 装了、没开的软件。
    pub off: Vec<String>,
    /// 文件的指纹（施工 P-2 下）：照找到的那一份算，[`Chosen::keeping_memory`] 改了记忆那一格也不变。
    pub digest: ContentHash,
}

impl Chosen {
    /// 照装了的软件 `installed` 算好没开的那几个。
    pub fn new<'a>(
        id: String,
        file: PresetFile,
        installed: impl IntoIterator<Item = &'a str>,
    ) -> Chosen {
        let digest = file.digest();
        let off = off(&file, installed);
        Chosen {
            id,
            file,
            off,
            digest,
        }
    }

    /// 换预设时记忆照开会话时的（施工 P-2 下，L3）：`memory` 开不开改成 `open`，没开的那几个照 `installed` 重新算，指纹不变。
    #[must_use]
    pub fn keeping_memory<'a>(
        mut self,
        open: bool,
        installed: impl IntoIterator<Item = &'a str>,
    ) -> Chosen {
        self.file.software.insert(MEMORY.to_string(), open);
        self.off = off(&self.file, installed);
        self
    }

    /// 记进快照的那一份。
    pub fn pin(&self) -> PresetPin {
        PresetPin {
            id: self.id.clone(),
            off: self.off.clone(),
            digest: Some(self.digest.clone()),
        }
    }
}

/// 装了的 `installed` 里 `file` 没开的，照编号排、不重复。
fn off<'a>(file: &PresetFile, installed: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let off: BTreeSet<String> = installed
        .into_iter()
        .filter(|software| !file.opens(software))
        .map(str::to_string)
        .collect();
    off.into_iter().collect()
}

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

/// 预设文件错在哪一种。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    /// 读不成 TOML。
    Syntax,
    /// 多了 `[preset]`、`[software]`、`[tools]` 以外的表。
    UnknownTable,
    /// 这三样有一样不是表。
    NotATable,
    /// `[preset]` 里多了别的键。
    UnknownKey,
    /// `name`、`summary` 不是一句字，也不是语言到一句话的表（以前的写法）。
    NotPhrases,
    /// 语言不是 `zh`、`en`、`ja`。
    UnknownLanguage,
    /// 一句话是空的、不是字。
    EmptyPhrase,
    /// `default_persona` 不是合写法的人格编号。
    BadPersona,
    /// `unlisted` 不是 `on`、`off`。
    BadUnlisted,
    /// `[software]` 的键不是合写法的软件包编号。
    BadSoftware,
    /// `[software]` 的值不是开关。
    NotBool,
    /// `[tools]` 的键不是工具名的写法。
    BadTool,
    /// `[tools]` 的值不是 `false`：单件打开某个包里的一件先不做。
    NotFalse,
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
            Code::BadPersona => "bad_persona",
            Code::BadUnlisted => "bad_unlisted",
            Code::BadSoftware => "bad_software",
            Code::NotBool => "not_bool",
            Code::BadTool => "bad_tool",
            Code::NotFalse => "not_false",
        }
    }

    /// 全部，照先后：给人看的字的门禁照它查三种语言都有。
    pub const ALL: [Code; 13] = [
        Code::Syntax,
        Code::UnknownTable,
        Code::NotATable,
        Code::UnknownKey,
        Code::NotPhrases,
        Code::UnknownLanguage,
        Code::EmptyPhrase,
        Code::BadPersona,
        Code::BadUnlisted,
        Code::BadSoftware,
        Code::NotBool,
        Code::BadTool,
        Code::NotFalse,
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

/// 读一份预设文件：`[preset]`、`[software]`、`[tools]` 三张表，都可以没有。
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
            "software" => {
                for (name, item) in reader.table(key, item)?.iter() {
                    if !valid_name(name) {
                        return Err(reader.problem(
                            item,
                            Code::BadSoftware,
                            &format!("software.{name}"),
                            format!("software.{name}: a package id starts with a lowercase letter and uses only lowercase letters, digits, - and _"),
                        ));
                    }
                    let on = item.as_bool().ok_or_else(|| {
                        reader.problem(
                            item,
                            Code::NotBool,
                            &format!("software.{name}"),
                            format!("software.{name} must be true or false"),
                        )
                    })?;
                    file.software.insert(name.to_string(), on);
                }
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
    /// `[preset]` 那一张表。
    fn preset(&self, table: &dyn TableLike, file: &mut PresetFile) -> Result<(), Problem> {
        for (key, item) in table.iter() {
            match key {
                "name" => file.name = Some(self.label(key, item)?),
                "summary" => file.summary = Some(self.label(key, item)?),
                "default_persona" => {
                    let persona = item.as_str().filter(|id| valid_name(id)).ok_or_else(|| {
                        self.problem(
                            item,
                            Code::BadPersona,
                            "preset.default_persona",
                            "preset.default_persona must be a persona id".to_string(),
                        )
                    })?;
                    file.default_persona = Some(persona.to_string());
                }
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
        phrases::read_label(item).map_err(|error| match error {
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

/// 名字、说明照以前的写法写成 JSON（[`PresetFile::digest`]）：没写的是空表，语言表照原样，一句字的是那句字。
fn label_json(label: Option<&Label>) -> serde_json::Value {
    match label {
        None => serde_json::json!({}),
        Some(Label::Each(phrases)) => serde_json::json!(phrases),
        Some(Label::One(text)) => serde_json::json!(text),
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

#[cfg(test)]
mod tests;
