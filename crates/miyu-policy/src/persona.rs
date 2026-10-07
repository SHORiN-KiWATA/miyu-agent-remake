//! 人格目录里的两份字怎么读（施工 P-1 上，`docs/blueprint/personas.md`）：`persona.toml` 的名字、说明，`prompts/examples.md`
//! 的示范对话。纯逻辑：进来的是文件里的字，出去的是读好的样子，或者写明哪个文件第几行错在哪。找哪几层、读盘由存储做。

use std::collections::BTreeMap;
use std::fmt;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::request::Message;
use serde::{Deserialize, Serialize};
use toml_edit::{Document, Item, TableLike};

use crate::memory::MemoryScope;

/// `persona.toml` 在人格目录里的名字。
pub const TOML: &str = "persona.toml";
/// 示范对话在人格目录里的位置。
pub const EXAMPLES: &str = "prompts/examples.md";
/// 认得的语言：名字、说明各写这几种里的几种。
pub const LANGUAGES: [&str; 3] = ["zh", "en", "ja"];

/// 一句话的几种语言：语言代码到那一句。
pub type Phrases = BTreeMap<String, String>;

/// `persona.toml` 读好的样子。每一格都可以没有。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonaFile {
    /// 名字，例如 `{"en": "Software Engineer", "zh": "软件工程师"}`。
    pub name: Phrases,
    /// 一句说明。
    pub summary: Phrases,
    /// 记忆的默认范围（`[memory] scope`，施工 R-3 下）：只能是 `persona`、`session`；没写的是没有，照 `persona` 算。
    pub memory: Option<MemoryScope>,
}

impl PersonaFile {
    /// 叠在 `lower` 上面（同名覆盖，16 第四节）：逐项盖，这一层写了的语言换掉，没写的沿用下面的。
    #[must_use]
    pub fn over(self, mut lower: PersonaFile) -> PersonaFile {
        lower.name.extend(self.name);
        lower.summary.extend(self.summary);
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

/// 人格的文件写错了：哪个文件、第几行（从 1 数，说不出的没有）、错在哪。错在哪是给人看的英文短句。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// 人格目录里的相对位置，例如 `persona.toml`。
    pub file: &'static str,
    /// 第几行。
    pub line: Option<usize>,
    /// 错在哪。
    pub message: String,
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
            return Err(problem(at(item), format!("unknown table [{key}]")));
        }
        let Some(table) = item.as_table_like() else {
            return Err(problem(at(item), "persona must be a table".to_string()));
        };
        for (key, item) in table.iter() {
            let phrases = match key {
                "name" => &mut file.name,
                "summary" => &mut file.summary,
                other => return Err(problem(at(item), format!("unknown key persona.{other}"))),
            };
            *phrases = read_phrases(key, item, &at)?;
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
        return Err(problem(at(item), "memory must be a table".to_string()));
    };
    let mut scope = None;
    for (key, item) in table.iter() {
        if key != "scope" {
            return Err(problem(at(item), format!("unknown key memory.{key}")));
        }
        scope = match item.as_str().and_then(MemoryScope::parse) {
            Some(MemoryScope::Off) | None => {
                return Err(problem(
                    at(item),
                    "memory.scope must be persona or session".to_string(),
                ));
            }
            found => found,
        };
    }
    Ok(scope)
}

/// 一张语言到一句话的表。
fn read_phrases(
    field: &str,
    item: &Item,
    at: &dyn Fn(&Item) -> Option<usize>,
) -> Result<Phrases, Problem> {
    let Some(table) = item.as_table_like() else {
        return Err(problem(
            at(item),
            format!("persona.{field} must map languages to text"),
        ));
    };
    let mut phrases = Phrases::new();
    for (language, value) in TableLike::iter(table) {
        if !LANGUAGES.contains(&language) {
            return Err(problem(
                at(value),
                format!("persona.{field}.{language}: language must be zh, en or ja"),
            ));
        }
        match value.as_str().map(str::trim) {
            Some(text) if !text.is_empty() => {
                phrases.insert(language.to_string(), text.to_string());
            }
            _ => {
                return Err(problem(
                    at(value),
                    format!("persona.{field}.{language} must be non-empty text"),
                ));
            }
        }
    }
    Ok(phrases)
}

fn problem(line: Option<usize>, message: String) -> Problem {
    Problem {
        file: TOML,
        line,
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
                    return Err(example(number, "user and assistant must take turns"));
                }
                if said.is_empty() && who == Speaker::Assistant {
                    return Err(example(number, "the first line must start with user:"));
                }
                said.push((who, number, rest.to_string()));
            }
            None if line.trim().is_empty() => {}
            None => match said.last_mut() {
                Some((_, _, text)) => {
                    text.push('\n');
                    text.push_str(line);
                }
                None => return Err(example(number, "the first line must start with user:")),
            },
        }
    }
    if let Some((Speaker::User, number, _)) = said.last() {
        return Err(example(*number, "the last line must be the assistant's"));
    }
    let mut demos = Vec::new();
    for pair in said.chunks(2) {
        let [(_, first, user), (_, second, assistant)] = pair else {
            continue;
        };
        for (number, text) in [(first, user), (second, assistant)] {
            if text.trim().is_empty() {
                return Err(example(*number, "a line must say something"));
            }
        }
        demos.push(Demo {
            user: user.trim().to_string(),
            assistant: assistant.trim().to_string(),
        });
    }
    Ok(demos)
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

fn example(line: usize, message: &str) -> Problem {
    Problem {
        file: EXAMPLES,
        line: Some(line),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests;
