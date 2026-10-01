//! 读一份配置的字（`docs/blueprint/config.md`「怎么走」第二条第 3 条，施工 8-2）：用 `toml_edit` 解析，照清单一项项认，
//! 记下每一项在第几行；类型、层不对的那一项报问题、不收，别的照收（G8：只丢写错的那一项）。
//!
//! - TOML 写法不对：整份的问题 `syntax`，行、列照 `toml_edit` 报的位置，为什么只取它的原话，不带它印出来的那一行
//!   原文（原文可能很长，密钥文件里更不能印）。
//! - 不在清单里的键：`unknown_key`，警告，原样留在文件里，不进最终值；拼错的给出离得最近的键名。
//! - 一组键（`ui`）下面写成了一个值：`wrong_type`，期望一张表。
//! - 这一层不能写的：`wrong_layer`；值写得对的照样记下来（不算），`miyu config explain` 列得出它。
//! - 键里人起的名字那一段（施工 8-6）：照清单里的样子认（[`crate::key`]），写法不对的那一段报一条 `bad_format`，底下的
//!   都不收。真的键照 [`crate::key::join`] 的写法记。
//!
//! 开头的 UTF-8 BOM 去掉再解析（读文件的一方已经去掉了，这里再去一次不碍事），行、列照去掉以后的字算。

use std::collections::BTreeMap;
use std::ops::Range;

use toml_edit::{Document, Item as Node, Key, Value as TomlValue};

use crate::item::{Item, Kind, Layer};
use crate::key::{self, Fit};
use crate::problem::{At, Code, Problem, nearest};
use crate::value::{Number, Value};

/// 开头的 UTF-8 BOM。
const BOM: char = '\u{FEFF}';

/// 读好的一份：写对了的项，和发现的问题。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    /// 写对了的项：真的键到它写的值和位置。不能写在这一层的也在，`counts` 是 `false`。
    pub entries: BTreeMap<String, Entry>,
    /// 一项一项的问题，照在文件里的先后。
    pub problems: Vec<Problem>,
}

/// 一项写的值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 清单里的哪一项（它的键，可能是带占位的样子，施工 8-6）。
    pub item: &'static str,
    /// 写的值，过了类型的校验。
    pub value: Value,
    /// 键所在的那一行（来源的 `line`）。
    pub line: usize,
    /// 值在哪：这一项的问题（例如收紧）指到它。
    pub at: At,
    /// 值的原文。
    pub raw: String,
    /// 算不算：这一层能写的算；不能写的（`wrong_layer`）不算，只记着给人看。
    pub counts: bool,
}

/// 把字 `text` 当成 `layer` 那一层的文件读，照清单 `items` 认。
///
/// # Errors
///
/// TOML 写法不对：交回那一条 `syntax`，这份文件整份不用（装箱：一条问题的格不少，别让结果跟着变大）。
pub fn parse(items: &[Item], layer: Layer, text: &str) -> Result<Parsed, Box<Problem>> {
    let text = text.strip_prefix(BOM).unwrap_or(text);
    let document = Document::parse(text).map_err(|error| {
        let at = error.span().map(|span| At::of(text, span.start));
        Box::new(Problem {
            at,
            ..Problem::file(Code::Syntax, layer, Some(why(error.message())))
        })
    })?;
    let mut reader = Reader {
        items,
        layer,
        text,
        parsed: Parsed::default(),
    };
    if let Some(root) = document.as_item().as_table_like() {
        reader.table(root, &[], None);
    }
    reader
        .parsed
        .problems
        .sort_by_key(|problem| problem.at.map(|at| (at.line, at.column)));
    Ok(reader.parsed)
}

/// `toml_edit` 的原话只取最后一行（为什么），不带它印出来的原文。
pub(crate) fn why(message: &str) -> String {
    message
        .trim_end()
        .lines()
        .next_back()
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// 读的时候手里的几样。
struct Reader<'a> {
    items: &'a [Item],
    layer: Layer,
    text: &'a str,
    parsed: Parsed,
}

impl Reader<'_> {
    /// 一张表 `table`（键的前几段是 `prefix`，最上面的是空的）里的每一格。点号连着写的键（`ui.language = …`）里的表，
    /// 位置照整个键的开头 `anchor` 算：报错指到这一行的键的第一个字。
    fn table(&mut self, table: &dyn toml_edit::TableLike, prefix: &[String], anchor: Option<At>) {
        for (name, _) in table.iter() {
            let Some((key, node)) = table.get_key_value(name) else {
                continue;
            };
            let path: Vec<String> = prefix
                .iter()
                .cloned()
                .chain(std::iter::once(name.to_string()))
                .collect();
            let key_at = anchor.unwrap_or_else(|| self.at(key.span()));
            self.node(&path, key, key_at, node);
        }
    }

    /// 一格：清单里的一项、一组键，或者不认识的。键是一段段的 `path`，在 `key_at`。
    fn node(&mut self, path: &[String], key: &Key, key_at: At, node: &Node) {
        let inner = |table: &dyn toml_edit::TableLike| table.is_dotted().then_some(key_at);
        let full = key::join(path);
        let last = path.len() - 1;
        let mut bad = None;
        let mut group = false;
        let mut found = None;
        // 占位不会是最后一段（`list::check`）：名字写错的那一段总在一组键上，在对一组的开头时查出来。
        for item in self.items {
            if matches!(key::fit(item.key, path), Fit::Yes(_)) {
                found = Some(item);
            }
            match key::fit_start(item.key, path) {
                Fit::Yes(_) => group = true,
                Fit::BadName { at, placeholder } if at == last => bad = Some(placeholder),
                _ => {}
            }
        }
        if let Some(item) = found {
            self.item(item, &full, key_at, node);
        } else if group {
            match node.as_table_like() {
                Some(table) => self.table(table, path, inner(table)),
                None => {
                    let at = self.value_at(node, key_at);
                    let raw = self.raw(node, key);
                    let problem = Problem::item(Code::WrongType, self.layer, &full, at, &raw);
                    self.parsed.problems.push(problem);
                }
            }
        } else if let Some(placeholder) = bad {
            // 人起的名字写法不对：报这一段，底下的都不收（施工 8-6）。
            let mut problem = Problem::item(Code::BadSegment, self.layer, &full, key_at, "");
            problem.got = None;
            problem.name = Some(path[last].clone());
            problem.why = Some(placeholder.to_string());
            self.parsed.problems.push(problem);
        } else if let Some(table) = node.as_table_like() {
            // 不认识的一张表：往里走，每一项照整个键报，离得最近的键名才找得准（`uii.language` 找得到 `ui.language`）。
            self.table(table, path, inner(table));
        } else {
            let raw = self.raw(node, key);
            let mut problem = Problem::item(Code::UnknownKey, self.layer, &full, key_at, &raw);
            problem.suggest = nearest(self.items, &full);
            self.parsed.problems.push(problem);
        }
    }

    /// 清单里的一项 `item`，真的键是 `full`，在 `key_at`。
    fn item(&mut self, item: &Item, full: &str, key_at: At, node: &Node) {
        let at = self.value_at(node, key_at);
        let raw = self.slice(node.span()).unwrap_or_default().to_string();
        let value = match item.kind {
            Kind::Secret => crate::secret::read_node(node).map(Value::Secret),
            kind => node.as_value().and_then(|value| read(kind, value)),
        };
        let counts = item.layers.contains(&self.layer);
        if !counts {
            self.parsed
                .problems
                .push(Problem::item(Code::WrongLayer, self.layer, full, at, &raw));
        }
        let checked = value.ok_or(Code::WrongType).and_then(|value| {
            item.kind.check(&value)?;
            Ok(value)
        });
        match checked {
            Ok(value) => {
                let entry = Entry {
                    item: item.key,
                    value,
                    line: key_at.line,
                    at,
                    raw,
                    counts,
                };
                self.parsed.entries.insert(full.to_string(), entry);
            }
            Err(_) if !counts => {}
            Err(code) => self
                .parsed
                .problems
                .push(Problem::item(code, self.layer, full, at, &raw)),
        }
    }

    /// 一格的值在哪；没有位置的（表头这类）照键的位置。
    fn value_at(&self, node: &Node, key_at: At) -> At {
        match node {
            Node::Value(value) => value
                .span()
                .map_or(key_at, |span| At::of(self.text, span.start)),
            _ => key_at,
        }
    }

    /// 一格的原文：值照它的位置取；表这类取不到的，写成键。
    fn raw(&self, node: &Node, key: &Key) -> String {
        match node {
            Node::Value(value) => self.slice(value.span()).unwrap_or_default().to_string(),
            _ => self
                .slice(key.span())
                .unwrap_or_else(|| key.get())
                .to_string(),
        }
    }

    /// 照位置取原文。
    fn slice(&self, span: Option<Range<usize>>) -> Option<&str> {
        span.and_then(|span| self.text.get(span))
    }

    /// 照位置算行列；没有位置的当第一行第一列。
    fn at(&self, span: Option<Range<usize>>) -> At {
        span.map_or(At { line: 1, column: 1 }, |span| {
            At::of(self.text, span.start)
        })
    }
}

/// 照类型读一个 TOML 的值：选项、网址、名字、引用、文字、时长要字，开关要布尔，整数要整数，小数要小数或整数，密钥要只有一格的行内表，列表要数组、
/// 每一个照元素的类型。别的写法读不成。有表头的密钥不是值，不走这里（[`crate::secret::read_node`]）。
pub(crate) fn read(kind: Kind, value: &TomlValue) -> Option<Value> {
    match (kind, value) {
        (
            Kind::Option(_)
            | Kind::Url
            | Kind::Name
            | Kind::Reference
            | Kind::Text { .. }
            | Kind::Duration { .. },
            TomlValue::String(text),
        ) => Some(Value::Text(std::borrow::Cow::Owned(text.value().clone()))),
        // 小数也收整数：`price_multiplier = 1` 是常见的写法（施工 8-7）。
        (Kind::Float { .. }, TomlValue::Float(number)) => {
            Some(Value::Float(Number::new(*number.value())))
        }
        (Kind::Float { .. }, TomlValue::Integer(number)) => {
            Some(Value::Float(Number::new(*number.value() as f64)))
        }
        (Kind::Bool, TomlValue::Boolean(on)) => Some(Value::Bool(*on.value())),
        (Kind::Int { .. }, TomlValue::Integer(number)) => Some(Value::Int(*number.value())),
        (Kind::Secret, TomlValue::InlineTable(table)) => {
            crate::secret::Reference::read(table).map(Value::Secret)
        }
        (Kind::List(inner), TomlValue::Array(array)) => array
            .iter()
            .map(|value| read(*inner, value))
            .collect::<Option<Vec<_>>>()
            .map(Value::List),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
