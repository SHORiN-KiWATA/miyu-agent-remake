//! 改一项、删一项（`docs/blueprint/config.md`「怎么走」第五条第 2、3 条，G5，施工 8-3）：只动那一项，别的字节一个
//! 不变：注释、空行、顺序、不认识的键都照原样（`07-存储.md` S9）。
//!
//! 做法是在原来的字上按位置换：`toml_edit` 读的时候记下了每一格在第几个字节（[`toml_edit::Document`]），改哪一项就
//! 只换那一段字。不把整份读成可改的文档再写回去：写回去的样子由它定，前后字节比不住。
//!
//! - 已经有的：原地换值，行尾注释留着。
//! - 没有的：放进 `[<键的前几段>]` 那张表，接在它最后一个键那一行后面，表里还没有键的接在表头后面；那张表写成点号连着
//!   的键、行内表的，照它原来的写法接上；没有这张表的，在文件末尾新开，前面空一行。
//! - 删掉的：连同它那一行删掉；表空了、里面也没有注释的，表头一起删（在文件末尾的，连表头前面那一行空行）。
//! - 换行照这份文件原来的：第一个换行是 `\r\n` 的用 `\r\n`，没有换行的用 `\n`。
//!
//! 改完的字再读一遍，读不懂的（例如行内表后面又开了它的子表）当放不进去（[`Blocked`]），一个字节都不交出去。

use std::borrow::Cow;
use std::ops::Range;

use toml_edit::{Document, InlineTable, Item as Node, Table, Value as TomlValue};

use crate::item::Kind;
use crate::secret::Reference;
use crate::value::{Number, Value};

/// 改一项还是删一项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change<'a> {
    /// 键改成这个值；没有的加上。
    Set(&'a str, &'a Value),
    /// 删掉这个键；本来就没写的，字照原样。
    Unset(&'a str),
}

impl Change<'_> {
    /// 改的是哪一项。
    pub fn key(&self) -> &str {
        match self {
            Change::Set(key, _) | Change::Unset(key) => key,
        }
    }
}

/// 放不进去：这份字读不懂，或者这一项所在的那一组写成了别的东西（`ui = "zh"`、`[[ui]]`、键本身是一张表），带着键。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blocked(pub String);

/// 新建的文件开头那一行：`#:schema` 和 Schema 的相对路径（第五条第 2 条第 6 款）。只在核心新建配置文件时用，已经有的
/// 文件不加（G10）。
pub fn new_file(schema: &str) -> String {
    format!("#:schema {schema}\n")
}

/// 在字 `text` 上照 `change` 改一项，交回改好的字。
///
/// # Errors
///
/// 字读不懂；这一项放不进去（[`Blocked`]）。
pub fn apply(text: &str, change: Change<'_>) -> Result<String, Blocked> {
    let key = change.key();
    let blocked = || Blocked(key.to_string());
    let document = Document::parse(text).map_err(|_| blocked())?;
    // 真的键照 TOML 的写法拆：人起的名字那一段可能带引号（施工 8-6）。
    let path = crate::key::split(key).ok_or_else(blocked)?;
    let path: Vec<&str> = path.iter().map(String::as_str).collect();
    // 没有组的键（只有一段）放在最上面那张表里：密钥文件平铺，一行一个（施工 8-5）。
    let Some((last, group)) = path.split_last() else {
        return Err(blocked());
    };
    let section = section(&document, group).ok_or_else(blocked)?;
    let edited = match change {
        Change::Set(_, value) => set(text, &section, group, last, value),
        Change::Unset(_) => unset(text, &section, group, last),
    }
    .ok_or_else(blocked)?;
    match Document::parse(edited.as_str()) {
        Ok(_) => Ok(edited),
        Err(_) => Err(blocked()),
    }
}

/// 人敲的字照这一项的类型读（第五条第 3 条）：开关只认 `true`、`false`；选项、名字、引用照原样，两头带着双引号、
/// 是一个 TOML 字符串的去掉引号再用（照 TOML 转义读）；密钥照 TOML 的行内表读（`{ secret = "deepseek" }`，施工 8-5）；
/// 整数照 TOML 的整数读，列表照 TOML 的数组读（施工 8-6）；文字、时长同选项，小数照 TOML 的小数、整数读（施工 8-7）；
/// 网址先试引用（同密钥的读法），不是引用形状的照字读（施工 8-6b）。读不成的是空的（`wrong_type`）；读成了、不合这种
/// 类型的（不在选项里、不在范围里、写法不对、网址是 `{ secret = … }`）由调用的一方查。
pub fn input(kind: Kind, text: &str) -> Option<Value> {
    match kind {
        Kind::Bool => match text {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        Kind::Option(_)
        | Kind::Name
        | Kind::Reference
        | Kind::Text { .. }
        | Kind::Duration { .. } => Some(Value::Text(Cow::Owned(unquoted(text)))),
        Kind::Url => Reference::from_input(text)
            .map(Value::Secret)
            .or_else(|| Some(Value::Text(Cow::Owned(unquoted(text))))),
        Kind::Secret => Reference::from_input(text).map(Value::Secret),
        Kind::Int { .. } | Kind::Float { .. } | Kind::List(_) => {
            let document = Document::parse(format!("v = {text}")).ok()?;
            let value = document.get("v")?.as_value()?;
            crate::parse::read(kind, value)
        }
    }
}

/// 协议上 JSON 的值照这一项的类型读：选项、名字、引用要字，开关要布尔，密钥要 `{"secret": …}` 或 `{"env": …}`，
/// 整数要整数，列表要数组、每一个照元素的类型（施工 8-6）；文字、时长要字，小数要数（施工 8-7）；网址要字（写死的）或者
/// `{"env": …}`（引用，施工 8-6b，`{"secret": …}` 读得出来但 `Kind::check` 会挡）。别的是空的（`wrong_type`）。
pub fn from_json(kind: Kind, value: &serde_json::Value) -> Option<Value> {
    match (kind, value) {
        (
            Kind::Option(_)
            | Kind::Name
            | Kind::Reference
            | Kind::Text { .. }
            | Kind::Duration { .. },
            serde_json::Value::String(text),
        ) => Some(Value::Text(Cow::Owned(text.clone()))),
        (Kind::Url, serde_json::Value::String(text)) => Some(Value::Text(Cow::Owned(text.clone()))),
        (Kind::Float { .. }, serde_json::Value::Number(number)) => number
            .as_f64()
            .map(|number| Value::Float(Number::new(number))),
        (Kind::Bool, serde_json::Value::Bool(on)) => Some(Value::Bool(*on)),
        (Kind::Secret | Kind::Url, value) => Reference::from_json(value).map(Value::Secret),
        (Kind::Int { .. }, serde_json::Value::Number(number)) => number.as_i64().map(Value::Int),
        (Kind::List(inner), serde_json::Value::Array(values)) => values
            .iter()
            .map(|value| from_json(*inner, value))
            .collect::<Option<Vec<_>>>()
            .map(Value::List),
        _ => None,
    }
}

/// 两头带着双引号、正好是一个 TOML 字符串的：读出里面的字。别的照原样。
fn unquoted(text: &str) -> String {
    let quoted = text.len() >= 2 && text.starts_with('"') && text.ends_with('"');
    let read = quoted
        .then(|| Document::parse(format!("v = {text}")).ok())
        .flatten()
        .filter(|document| document.as_table().len() == 1)
        .and_then(|document| document.get("v").and_then(Node::as_str).map(str::to_string));
    read.unwrap_or_else(|| text.to_string())
}

/// 一组键在文件里是怎么写的。
enum Section<'d> {
    /// 有表头的表 `[ui]`。
    Header(&'d Table),
    /// 点号连着写的键 `ui.language = …`：写在最上面或者一张有表头的表里，新的键从第几段写起。
    Dotted(&'d Table, usize),
    /// 行内表 `ui = { … }`，和它整个在哪。
    Inline(&'d InlineTable, Range<usize>),
    /// 还没有：只因为子表才有的也算，没有表头。
    Missing,
}

/// 照 `group` 那几段找这一组；写成了值、数组的是空的（放不进去）。
fn section<'d, S>(document: &'d Document<S>, group: &[&str]) -> Option<Section<'d>> {
    let mut node = document.as_item();
    let mut anchor = 0;
    for (depth, part) in group.iter().enumerate() {
        let next = match node {
            Node::Table(table) => table.get(part),
            Node::Value(TomlValue::InlineTable(table)) => {
                table.get_key_value(part).map(|(_, node)| node)
            }
            _ => return None,
        };
        let Some(next) = next else {
            return Some(Section::Missing);
        };
        if matches!(next, Node::Table(table) if !table.is_dotted()) {
            anchor = depth + 1;
        }
        node = next;
    }
    match node {
        Node::Table(table) if table.is_dotted() => Some(Section::Dotted(table, anchor)),
        Node::Table(table) if table.is_implicit() => Some(Section::Missing),
        Node::Table(table) => Some(Section::Header(table)),
        Node::Value(TomlValue::InlineTable(table)) => {
            node.span().map(|span| Section::Inline(table, span))
        }
        _ => None,
    }
}

/// 这一组里名字是 `last` 的那一格；没有的是空的。
fn entry<'d>(section: &Section<'d>, last: &str) -> Option<&'d Node> {
    match section {
        Section::Header(table) | Section::Dotted(table, _) => table.get(last),
        Section::Inline(table, _) => table.get_key_value(last).map(|(_, node)| node),
        Section::Missing => None,
    }
}

/// 改一项：有的原地换值，没有的照这一组的写法加上。放不进去的是空的。
fn set(
    text: &str,
    section: &Section<'_>,
    group: &[&str],
    last: &str,
    value: &Value,
) -> Option<String> {
    let written = value.toml();
    if let Some(node) = entry(section, last) {
        let span = node.as_value()?.span()?;
        return Some(splice(text, span, &written));
    }
    let nl = newline(text);
    match section {
        Section::Header(table) if group.is_empty() => Some(top_line(
            text,
            table,
            &format!("{} = {written}", crate::key::join(&[last])),
            nl,
        )),
        Section::Header(table) => {
            let after = last_value_end(table).or_else(|| table.span().map(|span| span.end))?;
            let line = format!("{} = {written}", crate::key::join(&[last]));
            Some(insert_line(text, after, &line, nl))
        }
        Section::Dotted(table, anchor) => {
            let after = last_value_end(table)?;
            let key = crate::key::join(&[&group[*anchor..], &[last]].concat());
            Some(insert_line(text, after, &format!("{key} = {written}"), nl))
        }
        Section::Inline(table, span) => {
            let last = crate::key::join(&[last]);
            match inline_last_end(table) {
                Some(after) => Some(splice(text, after..after, &format!(", {last} = {written}"))),
                None => Some(splice(
                    text,
                    span.clone(),
                    &format!("{{ {last} = {written} }}"),
                )),
            }
        }
        Section::Missing => Some(append_table(
            text,
            &crate::key::join(group),
            &format!("{} = {written}", crate::key::join(&[last])),
            nl,
        )),
    }
}

/// 删一项：连同它那一行删掉，表空了连表头删；行内表里的删掉这一格和它旁边的逗号。本来就没写的照原样。
fn unset(text: &str, section: &Section<'_>, group: &[&str], last: &str) -> Option<String> {
    let Some(node) = entry(section, last) else {
        return Some(text.to_string());
    };
    let span = node.as_value()?.span()?;
    match section {
        Section::Inline(table, _) => {
            let (key, _) = table.get_key_value(last)?;
            let start = key.span()?.start;
            Some(splice(text, around_comma(text, start..span.end), ""))
        }
        // 最上面那张表没有表头可删（施工 8-5：密钥文件平铺）。
        Section::Header(_) if !group.is_empty() => {
            let removed = splice(
                text,
                line_start(text, span.start)..line_end(text, span.end),
                "",
            );
            Some(without_empty_header(&removed, group))
        }
        _ => Some(splice(
            text,
            line_start(text, span.start)..line_end(text, span.end),
            "",
        )),
    }
}

/// 删掉一格以后，那张有表头的表空了、里面也没有注释的：表头一起删；它在文件末尾的，连表头前面那一行空行。
fn without_empty_header(text: &str, group: &[&str]) -> String {
    let Ok(document) = Document::parse(text) else {
        return text.to_string();
    };
    let Some(Section::Header(table)) = section(&document, group) else {
        return text.to_string();
    };
    let Some(header) = table.span() else {
        return text.to_string();
    };
    let body_start = line_end(text, header.end);
    let next = headers(document.as_table())
        .into_iter()
        .filter(|start| *start >= body_start)
        .min()
        .map_or(text.len(), |start| line_start(text, start));
    if !text[body_start..next].trim().is_empty() {
        return text.to_string();
    }
    let mut start = line_start(text, header.start);
    if next == text.len() && start > 0 {
        let before = line_start(text, start - 1);
        if text[before..start].trim().is_empty() {
            start = before;
        }
    }
    splice(text, start..next, "")
}

/// 这张表里（连同点号连着写在它里面的）最后一个值在第几个字节结束。
fn last_value_end(table: &Table) -> Option<usize> {
    table
        .iter()
        .filter_map(|(_, node)| match node {
            Node::Value(value) => value.span().map(|span| span.end),
            Node::Table(inner) if inner.is_dotted() => last_value_end(inner),
            _ => None,
        })
        .max()
}

/// 行内表里最后一个值在第几个字节结束。
fn inline_last_end(table: &InlineTable) -> Option<usize> {
    table
        .iter()
        .filter_map(|(_, value)| value.span().map(|span| span.end))
        .max()
}

/// 全部表头（`[a]`、`[[a]]`）开头在第几个字节：删表头时找下一张表从哪开始。
fn headers(table: &Table) -> Vec<usize> {
    let mut found = Vec::new();
    for (_, node) in table.iter() {
        match node {
            Node::Table(inner) => {
                if !inner.is_dotted()
                    && !inner.is_implicit()
                    && let Some(span) = inner.span()
                {
                    found.push(span.start);
                }
                found.extend(headers(inner));
            }
            Node::ArrayOfTables(array) => {
                for inner in array.iter() {
                    found.extend(inner.span().map(|span| span.start));
                    found.extend(headers(inner));
                }
            }
            _ => {}
        }
    }
    found
}

/// 行内表里的一格 `range` 连同它后面的逗号；是最后一格的，连同它前面的逗号。
fn around_comma(text: &str, range: Range<usize>) -> Range<usize> {
    let after = &text[range.end..];
    let gap = after.len() - after.trim_start_matches([' ', '\t']).len();
    if after[gap..].starts_with(',') {
        let rest = &after[gap + 1..];
        let space = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        return range.start..range.end + gap + 1 + space;
    }
    let before = text[..range.start].trim_end_matches([' ', '\t']);
    match before.strip_suffix(',') {
        Some(kept) => kept.len()..range.end,
        None => range,
    }
}

/// 最上面那张表里加一行 `line`（施工 8-5）：接在它最后一个值那一行后面；还没有值的，放在第一张表的表头前面（放在表头
/// 后面就进了那张表），一张表都没有的接在末尾。
fn top_line(text: &str, root: &Table, line: &str, nl: &str) -> String {
    if let Some(after) = last_value_end(root) {
        return insert_line(text, after, line, nl);
    }
    match headers(root).into_iter().min() {
        Some(first) => {
            let at = line_start(text, first);
            splice(text, at..at, &format!("{line}{nl}"))
        }
        None => {
            let lead = match text.is_empty() || text.ends_with('\n') {
                true => "",
                false => nl,
            };
            format!("{text}{lead}{line}{nl}")
        }
    }
}

/// 在第 `after` 个字节所在的那一行后面插一行 `line`。那一行是最后一行、没有换行的，先补上。
fn insert_line(text: &str, after: usize, line: &str, nl: &str) -> String {
    let end = line_end(text, after);
    let lead = match text[..end].ends_with('\n') {
        true => "",
        false => nl,
    };
    splice(text, end..end, &format!("{lead}{line}{nl}"))
}

/// 在文件末尾新开一张表 `[header]`，放一行 `line`：前面空一行（已经空着的不再空，空文件不空）。
fn append_table(text: &str, header: &str, line: &str, nl: &str) -> String {
    let mut out = text.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(nl);
    }
    let body = out
        .strip_suffix('\n')
        .map(|rest| rest.strip_suffix('\r').unwrap_or(rest));
    if body.is_some_and(|body| !body.is_empty() && !body.ends_with('\n')) {
        out.push_str(nl);
    }
    out.push_str(&format!("[{header}]{nl}{line}{nl}"));
    out
}

/// 第 `at` 个字节所在那一行从哪开始。
fn line_start(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map_or(0, |found| found + 1)
}

/// 第 `at` 个字节所在那一行到哪结束：连同换行；最后一行没有换行的到末尾。
fn line_end(text: &str, at: usize) -> usize {
    text[at..]
        .find('\n')
        .map_or(text.len(), |found| at + found + 1)
}

/// 把 `range` 那一段换成 `with`。
fn splice(text: &str, range: Range<usize>, with: &str) -> String {
    format!("{}{with}{}", &text[..range.start], &text[range.end..])
}

/// 这份字用的换行：第一个换行是 `\r\n` 的用它，别的用 `\n`。
pub fn newline(text: &str) -> &'static str {
    match text.find('\n') {
        Some(at) if text[..at].ends_with('\r') => "\r\n",
        _ => "\n",
    }
}

#[cfg(test)]
mod tests;
