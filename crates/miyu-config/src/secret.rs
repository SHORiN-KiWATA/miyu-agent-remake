//! 密钥（`docs/blueprint/config.md`「怎么走」第九条，G9，施工 8-5）：密钥的名字、配置里引用密钥的写法
//! （`{ secret = "<名字>" }`、`{ env = "<变量>" }`），密钥文件 `system/secrets.toml` 的字怎么读、怎么改一行。
//!
//! 取出来的密钥是单独的类型 [`Secret`]：`Debug` 只印 `Secret(…)`，没有 `Display`，不能序列化，所以它不会顺手进了日志、
//! 事件、协议的回应、报错的话（`07-存储.md` 第九节）。密钥文件里的问题不带 `got`：收到的原文就是密钥。
//!
//! 纯逻辑：进来的是字，出去的是字。读写文件在 `miyu-store` 的 `secrets.rs`，`secret.*` 方法在端点。

use std::collections::BTreeMap;
use std::fmt;

use toml_edit::{Document, Item as Node, TableLike, Value as TomlValue};

use crate::edit::{self, Blocked, Change};
use crate::item::{Item, Layer};
use crate::parse::Parsed;
use crate::problem::{At, Code, Problem};
use crate::value::Value;

/// 名字最长几个字符（「类型」表的「名字」）。
pub const NAME_CHARS: usize = 64;

/// `secret.set` 收的密钥最长几个字节（「协议」`secret.set`）。
pub const VALUE_BYTES: usize = 16 * 1024;

/// 开头的 UTF-8 BOM。
const BOM: char = '\u{FEFF}';

/// 名字合不合写法：小写字母开头，只有小写字母、数字、`-`、`_`，最长 64 个字符。合写法的名字在 TOML 里是裸键，不用加引号。
pub fn valid_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_lowercase())
        && name.len() <= NAME_CHARS
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// 配置里引用一个密钥的写法（第九条第 5 条）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reference {
    /// `{ secret = "<名字>" }`：照名字到密钥文件里取。
    Secret(String),
    /// `{ env = "<变量>" }`：照核心起来时的环境取。
    Env(String),
}

impl Reference {
    /// 写成 TOML 的行内表：`{ secret = "deepseek" }`。
    pub fn toml(&self) -> String {
        let (key, name) = self.parts();
        format!(
            "{{ {key} = {} }}",
            Value::Text(name.to_string().into()).toml()
        )
    }

    /// 写成协议上的 JSON：`{"secret":"deepseek"}`。
    pub fn json(&self) -> serde_json::Value {
        let (key, name) = self.parts();
        serde_json::json!({ key: name })
    }

    /// 引用的名字：密钥的名字，或者环境变量的名字。
    pub fn name(&self) -> &str {
        self.parts().1
    }

    /// 哪一种、名字。
    fn parts(&self) -> (&'static str, &str) {
        match self {
            Reference::Secret(name) => ("secret", name),
            Reference::Env(name) => ("env", name),
        }
    }

    /// 照一种、一个名字造：`secret` 的名字要合写法；`env` 的名字不能是空的、不能有 `=` 和 NUL（系统的环境变量放不下）。
    fn of(kind: &str, name: &str) -> Option<Reference> {
        match kind {
            "secret" if valid_name(name) => Some(Reference::Secret(name.to_string())),
            "env" if !name.is_empty() && !name.contains(['=', '\0']) => {
                Some(Reference::Env(name.to_string()))
            }
            _ => None,
        }
    }

    /// 从 TOML 的一张表（行内表或者有表头的）读：正好一格，`secret` 或 `env`，值是字。别的写法读不成。
    pub(crate) fn read(table: &dyn TableLike) -> Option<Reference> {
        let mut entries = table.iter();
        let (kind, node) = entries.next()?;
        if entries.next().is_some() {
            return None;
        }
        Reference::of(kind, node.as_str()?)
    }

    /// 从协议上的 JSON 读：`{"secret": "…"}` 或 `{"env": "…"}`，正好一格。
    pub fn from_json(value: &serde_json::Value) -> Option<Reference> {
        let object = value.as_object().filter(|object| object.len() == 1)?;
        let (kind, name) = object.iter().next()?;
        Reference::of(kind, name.as_str()?)
    }

    /// 人敲的字：TOML 的行内表 `{ secret = "deepseek" }`。
    pub fn from_input(text: &str) -> Option<Reference> {
        let document = Document::parse(format!("v = {text}")).ok()?;
        let node = document.get("v")?;
        Reference::read(node.as_table_like()?)
    }
}

/// 一个密钥的值。只有 [`Secret::expose`] 拿得到字：`Debug` 只印 `Secret(…)`，没有 `Display`、不能序列化。
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(…)")
    }
}

/// `secret.set` 不收的密钥（都是 `bad_params`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// 去掉前后空白是空的。
    Empty,
    /// 有控制字符。
    Control,
    /// 超过 16 KiB。
    TooBig,
}

impl Secret {
    /// 经 `secret.set` 来的：去掉前后空白（粘贴时常带着换行），不能是空的、不能有控制字符、不超过 16 KiB。
    ///
    /// # Errors
    ///
    /// 不收的几种（[`Refused`]）。
    pub fn new(value: &str) -> Result<Secret, Refused> {
        let value = value.trim();
        if value.is_empty() {
            Err(Refused::Empty)
        } else if value.chars().any(char::is_control) {
            Err(Refused::Control)
        } else if value.len() > VALUE_BYTES {
            Err(Refused::TooBig)
        } else {
            Ok(Secret(value.to_string()))
        }
    }

    /// 密钥文件里手写的：去掉前后空白，不是空的就收（手改的不另查控制字符、长短，照原样用）。
    fn from_file(value: &str) -> Option<Secret> {
        let value = value.trim();
        (!value.is_empty()).then(|| Secret(value.to_string()))
    }

    /// 密钥本身。只在真要用它的地方拿（写回文件、连供应商）。
    pub fn expose(&self) -> &str {
        &self.0
    }
}

/// 读好的一份密钥文件：名字到密钥，和写错的那几行。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stored {
    /// 写对了的：名字到密钥。
    pub entries: BTreeMap<String, Secret>,
    /// 写错的：名字不合写法（`bad_format`）、值不是不空的字（`wrong_type`），照在文件里的先后。都不带 `got`。
    pub problems: Vec<Problem>,
}

/// 读密钥文件的字（第九条第 1 条）：一行一个 `名字 = "值"`，平铺，没有表。写错的那一行报问题、不收，别的照收（G8）。
/// 问题照系统配置那一层记：M8 只有系统的密钥文件。
///
/// # Errors
///
/// TOML 写法不对：交回那一条 `syntax`，为什么只取 `toml_edit` 的原话最后一行，不带它印的原文（原文就是密钥）。
pub fn parse_file(text: &str) -> Result<Stored, Box<Problem>> {
    let text = text.strip_prefix(BOM).unwrap_or(text);
    let document = Document::parse(text).map_err(|error| {
        let at = error.span().map(|span| At::of(text, span.start));
        let why = crate::parse::why(error.message());
        Box::new(Problem {
            at,
            ..Problem::file(Code::Syntax, Layer::System, Some(why))
        })
    })?;
    let mut stored = Stored::default();
    for (name, node) in document.as_table().iter() {
        let Some((key, _)) = document.as_table().get_key_value(name) else {
            continue;
        };
        let at = key
            .span()
            .map_or(At { line: 1, column: 1 }, |span| At::of(text, span.start));
        let code = match node.as_str().and_then(Secret::from_file) {
            Some(_) if !valid_name(name) => Code::SecretName,
            Some(secret) => {
                stored.entries.insert(name.to_string(), secret);
                continue;
            }
            None => Code::SecretValue,
        };
        stored.problems.push(Problem {
            at: Some(at),
            key: Some(name.to_string()),
            ..Problem::file(code, Layer::System, None)
        });
    }
    stored
        .problems
        .sort_by_key(|problem| problem.at.map(|at| (at.line, at.column)));
    Ok(stored)
}

/// 在密钥文件的字上写入或者换掉一个：有的只换那一个值，没有的加一行，注释、别的行一个字节不动（第九条第 4 条）。
///
/// # Errors
///
/// 字读不懂；这个名字在文件里写成了一张表（[`Blocked`]）。
pub fn set_in(text: &str, name: &str, secret: &Secret) -> Result<String, Blocked> {
    let value = Value::Text(secret.expose().to_string().into());
    edit::apply(text, Change::Set(name, &value))
}

/// 在密钥文件的字上删掉一个：连同它那一行。
///
/// # Errors
///
/// 字读不懂（[`Blocked`]）。
pub fn unset_in(text: &str, name: &str) -> Result<String, Blocked> {
    edit::apply(text, Change::Unset(name))
}

/// 一层配置里引用的密钥、环境变量取不取得到（第九条第 5 条）：引用的密钥没设的报 `unknown_secret`，环境变量没设的报
/// `env_not_set`，都是警告。只看这一层算数的、类型是密钥或者密钥的列表（施工 8-6）的项，列表里每一个各查各的。`secret`
/// 说一个名字的密钥设没设，`env` 说一个环境变量核心起来时设没设。
pub fn missing(
    items: &[Item],
    parsed: &Parsed,
    layer: Layer,
    secret: &dyn Fn(&str) -> bool,
    env: &dyn Fn(&str) -> bool,
) -> Vec<Problem> {
    let mut found = Vec::new();
    for (key, entry) in parsed.entries.iter().filter(|(_, entry)| entry.counts) {
        if !items.iter().any(|item| item.key == entry.item) {
            continue;
        }
        let references: Vec<&Reference> = match &entry.value {
            Value::Secret(reference) => vec![reference],
            Value::List(values) => values
                .iter()
                .filter_map(|value| match value {
                    Value::Secret(reference) => Some(reference),
                    _ => None,
                })
                .collect(),
            _ => continue,
        };
        for reference in references {
            let (code, name) = match reference {
                Reference::Secret(name) if !secret(name) => (Code::UnknownSecret, name),
                Reference::Env(name) if !env(name) => (Code::EnvNotSet, name),
                _ => continue,
            };
            let mut problem = Problem::item(code, layer, key, entry.at, &entry.raw);
            problem.name = Some(name.clone());
            found.push(problem);
        }
    }
    found
}

/// 一份最终值里引用了哪些密钥（`secret.list` 的 `used_by`，施工 8-6 起列表里的也算）：密钥的名字和引用它的真的键。
pub fn used(values: &crate::Values) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for key in values.keys() {
        let references: Vec<&Value> = match values.get(key) {
            Some(Value::List(values)) => values.iter().collect(),
            Some(value) => vec![value],
            None => continue,
        };
        for value in references {
            if let Value::Secret(Reference::Secret(name)) = value {
                found.push((name.clone(), key.to_string()));
            }
        }
    }
    found
}

/// 从一格 TOML 读引用：行内表、有表头的表都认。
pub(crate) fn read_node(node: &Node) -> Option<Reference> {
    match node {
        Node::Value(TomlValue::InlineTable(table)) => Reference::read(table),
        Node::Table(table) => Reference::read(table),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
