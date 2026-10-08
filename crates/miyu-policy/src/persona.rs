//! 人格目录里的两份字怎么读（施工 P-1 上，`docs/blueprint/personas.md`）：`persona.toml` 的名字、说明，`prompts/examples.md`
//! 的示范对话。纯逻辑：进来的是文件里的字，出去的是读好的样子，或者写明哪个文件第几行错在哪。找哪几层、读盘由存储做。

use std::fmt;

use miyu_config::phrases::{self, Label, PhraseError};
use miyu_kernel::block::{Block, Text};
use miyu_kernel::request::Message;
use serde::{Deserialize, Serialize};
use toml_edit::{Document, Item};

use crate::memory::MemoryScope;

/// `persona.toml` 在人格目录里的名字。
pub const TOML: &str = "persona.toml";
/// 示范对话在人格目录里的位置。
pub const EXAMPLES: &str = "prompts/examples.md";

/// P-3 上写进 `[persona]`、`[preset]` 的「以谁为底」那一格的键（施工 P-3 补撤掉了）：读的时候当没写，写的时候去掉（施工 P-3
/// 再补）。
pub const BASE: &str = "base";
/// 认得的语言：以前写成语言表的名字、说明，各写这几种里的几种。
pub use miyu_config::phrases::{LANGUAGES, Phrases};

/// `persona.toml` 读好的样子。每一格都可以没有。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonaFile {
    /// 名字：一句字（施工 P-3 补），以前写成语言表的照样认。
    pub name: Option<Label>,
    /// 一句说明，写法同名字。
    pub summary: Option<Label>,
    /// 记忆的默认范围（`[memory] scope`，施工 R-3 下）：只能是 `persona`、`session`；没写的是没有，照 `persona` 算。
    pub memory: Option<MemoryScope>,
}

impl PersonaFile {
    /// 叠在 `lower` 上面（同名覆盖，16 第四节）：逐项盖，这一层写了的整格换掉，没写的沿用下面的。
    #[must_use]
    pub fn over(self, mut lower: PersonaFile) -> PersonaFile {
        lower.name = self.name.or(lower.name);
        lower.summary = self.summary.or(lower.summary);
        lower.memory = self.memory.or(lower.memory);
        lower
    }
}

/// 一轮示范对话：人说的一句，她答的一句。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Demo {
    /// 人说的。
    pub user: String,
    /// 她答的。
    pub assistant: String,
}

impl Demo {
    /// 进请求的两条消息（施工 P-1 上）：人说的一条 user，她答的一条 assistant，各是一块文字。
    pub fn messages(&self) -> [Message; 2] {
        let text = |text: &str| {
            vec![Block::Text(Text {
                text: text.to_string(),
            })]
        };
        [
            Message::User {
                blocks: text(&self.user),
            },
            Message::Assistant {
                blocks: text(&self.assistant),
            },
        ]
    }
}

/// 人格的文件写错了：哪个文件、第几行（从 1 数，说不出的没有）、哪一种错、错的那一处，和一句英文短句（日志、协议的
/// `data.problem` 用）。给人看的那一句照 `code` 和 `detail` 用 `core/human/<语言>.json` 的 `persona-problems/<code>` 写
/// （施工 8-30）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// 人格目录里的相对位置，例如 `persona.toml`。
    pub file: &'static str,
    /// 第几行。
    pub line: Option<usize>,
    /// 哪一种错。
    pub code: Code,
    /// 错的那一处：表名、键（`persona.<键>`、`memory.<键>`）、`persona.<格>.<语言>`、`memory.scope`，读不成 TOML 的是它的
    /// 原话；示范对话的是空的。
    pub detail: String,
    /// 错在哪，英文短句。
    pub message: String,
}

/// 人格的文件错在哪一种（施工 8-30）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    /// 读不成 TOML。
    Syntax,
    /// 多了 `[persona]` 以外的表。
    UnknownTable,
    /// `persona` 不是表。
    NotATable,
    /// `[persona]` 里多了 `name`、`summary` 以外的键。
    UnknownKey,
    /// `name`、`summary` 不是一句字，也不是语言到一句话的表（以前的写法）。
    NotPhrases,
    /// 语言不是 `zh`、`en`、`ja`。
    UnknownLanguage,
    /// 一句话是空的、不是字。
    EmptyPhrase,
    /// `[memory]` 的 `scope` 不是 `persona`、`session`（施工 R-3 下加的读法）。
    BadMemoryScope,
    /// 示范对话第一行不是人说的。
    FirstLine,
    /// 示范对话没有一问一答交替。
    TakeTurns,
    /// 示范对话最后一句不是她答的。
    LastLine,
    /// 示范对话有一句是空的。
    EmptyLine,
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
            Code::BadMemoryScope => "bad_memory_scope",
            Code::FirstLine => "first_line",
            Code::TakeTurns => "take_turns",
            Code::LastLine => "last_line",
            Code::EmptyLine => "empty_line",
        }
    }

    /// 全部，照先后。
    pub const ALL: [Code; 12] = [
        Code::Syntax,
        Code::UnknownTable,
        Code::NotATable,
        Code::UnknownKey,
        Code::NotPhrases,
        Code::UnknownLanguage,
        Code::EmptyPhrase,
        Code::BadMemoryScope,
        Code::FirstLine,
        Code::TakeTurns,
        Code::LastLine,
        Code::EmptyLine,
    ];
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "{}:{line}: {}", self.file, self.message),
            None => write!(f, "{}: {}", self.file, self.message),
        }
    }
}

/// 读 `persona.toml`：`[persona]` 一张表，里面只有 `name`、`summary`，各是一张语言到一句话的表（`zh`、`en`、`ja`），话不能是
/// 空的；`[memory]` 一张表，里面只有 `scope`（`persona` 或 `session`，施工 R-3 下）。整个文件、这两张表、每一格都可以没有。
///
/// # Errors
///
/// 读不成 TOML、多了别的表或者键、语言不认识、值不是一句话。
pub fn read_toml(text: &str) -> Result<PersonaFile, Problem> {
    let document = Document::parse(text).map_err(|error| Problem {
        file: TOML,
        line: error.span().map(|span| line_of(text, span.start)),
        code: Code::Syntax,
        detail: error.message().trim().to_string(),
        message: error.message().trim().to_string(),
    })?;
    let at = |item: &Item| item.span().map(|span| line_of(text, span.start));
    let mut file = PersonaFile::default();
    for (key, item) in document.as_table().iter() {
        if key == "memory" {
            file.memory = read_memory(item, &at)?;
            continue;
        }
        if key != "persona" {
            return Err(problem(
                at(item),
                Code::UnknownTable,
                key,
                format!("unknown table [{key}]"),
            ));
        }
        let Some(table) = item.as_table_like() else {
            return Err(problem(
                at(item),
                Code::NotATable,
                "persona",
                "persona must be a table".to_string(),
            ));
        };
        for (key, item) in table.iter() {
            let label = match key {
                "name" => &mut file.name,
                "summary" => &mut file.summary,
                // P-3 上那几个小时里写进去的「以谁为底」：认出来就当没写（施工 P-3 再补），下一次写这份文件时去掉。
                BASE => continue,
                other => {
                    return Err(problem(
                        at(item),
                        Code::UnknownKey,
                        &format!("persona.{other}"),
                        format!("unknown key persona.{other}"),
                    ));
                }
            };
            *label = Some(read_label(key, item, text)?);
        }
    }
    Ok(file)
}

/// `[memory]`：只有 `scope`，`persona` 或 `session`。`off` 不在这里：不开记忆是开会话时、预设的事。
fn read_memory(
    item: &Item,
    at: &dyn Fn(&Item) -> Option<usize>,
) -> Result<Option<MemoryScope>, Problem> {
    let Some(table) = item.as_table_like() else {
        return Err(problem(
            at(item),
            Code::NotATable,
            "memory",
            "memory must be a table".to_string(),
        ));
    };
    let mut scope = None;
    for (key, item) in table.iter() {
        if key != "scope" {
            return Err(problem(
                at(item),
                Code::UnknownKey,
                &format!("memory.{key}"),
                format!("unknown key memory.{key}"),
            ));
        }
        scope = match item.as_str().and_then(MemoryScope::parse) {
            Some(MemoryScope::Off) | None => {
                return Err(problem(
                    at(item),
                    Code::BadMemoryScope,
                    "memory.scope",
                    "memory.scope must be persona or session".to_string(),
                ));
            }
            found => found,
        };
    }
    Ok(scope)
}

/// 名字、说明：一句字，或者以前的语言表。
fn read_label(field: &str, item: &Item, text: &str) -> Result<Label, Problem> {
    let line_at = |offset: usize| line_of(text, offset);
    // 说明可以是空的字（施工 P-3 再补）：没有说明，盖住下面那一层的。
    let read = match field {
        "summary" => phrases::read_summary,
        _ => phrases::read_label,
    };
    read(item).map_err(|error| match error {
        PhraseError::NotPhrases(span) => problem(
            span.map(|span| line_at(span.start)),
            Code::NotPhrases,
            &format!("persona.{field}"),
            format!("persona.{field} must be text"),
        ),
        PhraseError::Empty(language, span) if language.is_empty() => problem(
            span.map(|span| line_at(span.start)),
            Code::EmptyPhrase,
            &format!("persona.{field}"),
            format!("persona.{field} must be non-empty text"),
        ),
        PhraseError::UnknownLanguage(language, span) => problem(
            span.map(|span| line_at(span.start)),
            Code::UnknownLanguage,
            &format!("persona.{field}.{language}"),
            format!("persona.{field}.{language}: language must be zh, en or ja"),
        ),
        PhraseError::Empty(language, span) => problem(
            span.map(|span| line_at(span.start)),
            Code::EmptyPhrase,
            &format!("persona.{field}.{language}"),
            format!("persona.{field}.{language} must be non-empty text"),
        ),
    })
}

fn problem(line: Option<usize>, code: Code, detail: &str, message: String) -> Problem {
    Problem {
        file: TOML,
        line,
        code,
        detail: detail.to_string(),
        message,
    }
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

/// 读示范对话（照旧版 `miyu-dialogs.md` 的写法）：`user:` 或 `assistant:` 开头（不分大小写）起一条，
/// 后面不带开头的行接在这一条后面，空行不算；要从人说的开始、一问一答交替、她答的结束。每一句去掉前后空白，不能是空的。
///
/// # Errors
///
/// 第一行不是 `user:`、`assistant:` 开头；没有交替；最后是人说的；有一句是空的。写明第几行。
pub fn read_examples(text: &str) -> Result<Vec<Demo>, Problem> {
    let mut said: Vec<(Speaker, usize, String)> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        match speaker(line) {
            Some((who, rest)) => {
                if let Some((last, _, _)) = said.last()
                    && *last == who
                {
                    return Err(example(number, Code::TakeTurns));
                }
                if said.is_empty() && who == Speaker::Assistant {
                    return Err(example(number, Code::FirstLine));
                }
                said.push((who, number, rest.to_string()));
            }
            None if line.trim().is_empty() => {}
            None => match said.last_mut() {
                Some((_, _, text)) => {
                    text.push('\n');
                    text.push_str(line);
                }
                None => return Err(example(number, Code::FirstLine)),
            },
        }
    }
    if let Some((Speaker::User, number, _)) = said.last() {
        return Err(example(*number, Code::LastLine));
    }
    let mut demos = Vec::new();
    for pair in said.chunks(2) {
        let [(_, first, user), (_, second, assistant)] = pair else {
            continue;
        };
        for (number, text) in [(first, user), (second, assistant)] {
            if text.trim().is_empty() {
                return Err(example(*number, Code::EmptyLine));
            }
        }
        demos.push(Demo {
            user: user.trim().to_string(),
            assistant: assistant.trim().to_string(),
        });
    }
    Ok(demos)
}

/// 把示范对话写回 `prompts/examples.md` 的写法（施工 P-3 补：界面里一对一对地编，格式留在核心）：每一对 `user:`、
/// `assistant:` 开头，对与对之间空一行。一句里的空行写不进去（读的时候空行不算），去掉；每一句去掉前后空白。
///
/// # Errors
///
/// 写出来读不回原样的（一句里有一行看起来像 `user:`、`assistant:` 开头，读的时候会当成下一句），交回是第几对（从 1 数）。
pub fn write_examples(demos: &[Demo]) -> Result<String, usize> {
    let kept: Vec<Demo> = demos
        .iter()
        .map(|demo| Demo {
            user: solid(&demo.user),
            assistant: solid(&demo.assistant),
        })
        .collect();
    let text = written(&kept);
    if read_examples(&text).is_ok_and(|read| read == kept) {
        return Ok(text);
    }
    let bad = (1..=kept.len())
        .find(|&n| {
            let part = &kept[..n];
            !read_examples(&written(part)).is_ok_and(|read| read == part)
        })
        .unwrap_or(kept.len());
    Err(bad)
}

/// 一对对写成字。
fn written(demos: &[Demo]) -> String {
    let pairs: Vec<String> = demos
        .iter()
        .map(|demo| format!("user: {}\nassistant: {}\n", demo.user, demo.assistant))
        .collect();
    pairs.join("\n")
}

/// 去掉空行和前后空白。
fn solid(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    lines.join("\n").trim().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Speaker {
    User,
    Assistant,
}

/// 这一行谁说的：`user:`、`assistant:` 开头（不分大小写），交回冒号后面的字。
fn speaker(line: &str) -> Option<(Speaker, &str)> {
    let (head, rest) = line.split_once(':')?;
    let who = if head.eq_ignore_ascii_case("user") {
        Speaker::User
    } else if head.eq_ignore_ascii_case("assistant") {
        Speaker::Assistant
    } else {
        return None;
    };
    Some((who, rest))
}

fn example(line: usize, code: Code) -> Problem {
    let message = match code {
        Code::FirstLine => "the first line must start with user:",
        Code::TakeTurns => "user and assistant must take turns",
        Code::LastLine => "the last line must be the assistant's",
        _ => "a line must say something",
    };
    Problem {
        file: EXAMPLES,
        line: Some(line),
        code,
        detail: String::new(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests;
