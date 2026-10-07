//! 场所规则（`docs/blueprint/chat.md` 第一条，`docs/designs/18-通讯平台.md` 第四节，施工 O-1）：读 `venues.d` 里的规则文件，
//! 照文件名排好先后；写错的照配置的原因码说清在哪、错在哪，其余照收。套到一个场所上在 [`resolve`] 里。
//!
//! 读的规矩（「怎么走」第 1 到 5、8 条）：
//!
//! - 出厂的和系统的合在一起，照文件名按字节排；同名的系统那一份替换出厂那一份，出厂的整个不读；同一个来源里重名的，后交进来
//!   的盖前面的。别的情形交进来的先后不影响结果。
//! - 一份文件：开头的 BOM 去掉再读；TOML 写法不对报一条 `syntax`，整份不用。
//! - 最上面只认 `rule`，别的键 `unknown_key`（警告）；`rule` 不是表的数组 `wrong_type`，这份文件没有规则。
//! - `match` 写错（不是表、不认识的键、值不对）这条规则整条不用：拼错的匹配条件不能让规则匹配得更宽（施工时定的第 2 条）。
//!   这条规则的属性照样查、照样报，改的人一次看全。
//! - 属性写错只丢那一项（`docs/designs/14-配置.md` G8）；不认识的键 `unknown_key`，给离得最近的名字。
//! - 问题照文件的先后，同一个文件里照行、列。
//!
//! 问题用自己的类型 [`Problem`]，原因码用配置的（施工时定的第 4 条）：配置的问题带一层 `Layer`，规则文件不是一层。

mod attrs;
mod forms;
mod ids;
mod resolve;

pub use ids::{parse_person, person};
pub use resolve::{Entry, Origin, Resolved, Venue, VenueKind};

use std::collections::BTreeMap;
use std::ops::Range;

use miyu_config::Value;
use miyu_config::parse::why;
use miyu_config::problem::{At, Code, got, nearest};
use toml_edit::{Document, Item as Node, Key, TableLike};

use resolve::{CONDITIONS, Match};

/// 开头的 UTF-8 BOM。
const BOM: char = '\u{FEFF}';

/// 最上面唯一认的键：规则的数组。
const RULE: &str = "rule";

/// 一条规则里放匹配条件的键。
const MATCH: &str = "match";

/// 一份规则文件来自哪。照先后排：同名的时候后面的替换前面的。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// 出厂：软件包资源目录里的 `venues.d/`，就是软件包的默认值。
    Factory,
    /// 系统：`system/venues.d/`，管理员写。同名的替换出厂那一份。
    System,
}

/// 一份规则文件：读文件的一方读好交进来（大小、是不是 UTF-8 由它管，施工时定的第 5 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// 来自哪。
    pub source: Source,
    /// 文件名，例如 `50-defaults.toml`：先后照它按字节排，同名的照来源替换。不带目录。
    pub name: String,
    /// 文件的字。
    pub text: String,
}

/// 读好的一组规则文件。
#[derive(Debug, Clone, Default)]
pub struct Read {
    /// 用得上的规则，照先后排好。
    pub rules: Rules,
    /// 发现的问题，照文件的先后，同一个文件里照行、列。
    pub problems: Vec<Problem>,
}

/// 一处问题，还没说成话（给人看的话随用到它的那一步，照 `miyu_config::problem::tell` 的做法）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// 原因码，照配置的；严重程度跟着它（[`Code::severity`]）。
    pub code: Code,
    /// 哪一份文件的来源。
    pub source: Source,
    /// 哪一份文件。
    pub file: String,
    /// 第几条规则，从 1 数。整份文件的问题、最上面的键没有。
    pub rule: Option<usize>,
    /// 哪一项的键，原样：属性写名字（`rate`），匹配条件带上 `match.`（`match.group`），最上面的写它自己（`rule`）。写法不对
    /// （`syntax`）的没有。
    pub key: Option<String>,
    /// 在哪：值写错的指到值，不认识的键指到键。写法不对的照 `toml_edit` 报的位置，它没给的没有。
    pub at: Option<At>,
    /// 收到了什么：原文照抄，最多 80 个字符（`miyu_config::problem::got`）；写成表头的是键。写法不对的没有。
    pub got: Option<String>,
    /// 不认识的键：离得最近的那一个，写法和 `key` 一样；太远的没有。
    pub suggest: Option<String>,
    /// 为什么：只有写法不对的有，是 `toml_edit` 说的最后一行，不带它印出来的原文。
    pub why: Option<String>,
}

/// 读好的规则，照先后排好；套到场所上用 [`Rules::resolve`]。
#[derive(Debug, Clone, Default)]
pub struct Rules {
    /// 用得上的规则：`match` 写错的不在里面。
    rules: Vec<Rule>,
}

/// 一条用得上的规则。
#[derive(Debug, Clone)]
struct Rule {
    /// 来自哪一份。
    source: Source,
    /// 文件名。
    file: String,
    /// 文件里第几条，从 1 数。
    number: usize,
    /// 匹配条件。
    matcher: Match,
    /// 写对了的属性：名字到值和键所在的行。
    attrs: BTreeMap<&'static str, (Value, usize)>,
}

impl Rules {
    /// 读一组文件（「怎么走」第 1 到 5、8 条）。不会失败：坏的文件、坏的规则、坏的一项都变成问题，其余照收。
    pub fn parse(files: &[File]) -> Read {
        // 照文件名按字节排（`str` 的先后就是字节的先后）；同名的，来源排在后面的（系统）替换前面的，一样的后来的替换。
        let mut chosen: BTreeMap<&str, &File> = BTreeMap::new();
        for file in files {
            if chosen
                .get(file.name.as_str())
                .is_none_or(|kept| kept.source <= file.source)
            {
                chosen.insert(&file.name, file);
            }
        }
        let mut read = Read::default();
        for file in chosen.into_values() {
            let mut reader = Reader {
                file,
                text: file.text.strip_prefix(BOM).unwrap_or(&file.text),
                rules: Vec::new(),
                problems: Vec::new(),
            };
            reader.document();
            reader
                .problems
                .sort_by_key(|problem| problem.at.map(|at| (at.line, at.column)));
            read.rules.rules.append(&mut reader.rules);
            read.problems.append(&mut reader.problems);
        }
        read
    }
}

/// 读一份文件时手里的几样。
struct Reader<'a> {
    /// 读的哪一份。
    file: &'a File,
    /// 去掉 BOM 的字：位置照它算。
    text: &'a str,
    /// 读出来的、用得上的规则。
    rules: Vec<Rule>,
    /// 这份文件的问题，读完照行、列排。
    problems: Vec<Problem>,
}

impl Reader<'_> {
    /// 读整份：写法不对报 `syntax`，什么规则也不收。
    fn document(&mut self) {
        let document = match Document::parse(self.text) {
            Ok(document) => document,
            Err(error) => {
                let at = error.span().map(|span| At::of(self.text, span.start));
                self.problems.push(Problem {
                    at,
                    why: Some(why(error.message())),
                    ..self.problem(Code::Syntax, None, None)
                });
                return;
            }
        };
        let root = document.as_table();
        for (name, _) in root.iter() {
            let Some((key, node)) = root.get_key_value(name) else {
                continue;
            };
            if name == RULE {
                self.rules(key, node);
            } else {
                let mut problem = self.item(Code::UnknownKey, None, name, key, node, true);
                problem.suggest = nearest([RULE], name).map(String::from);
                self.problems.push(problem);
            }
        }
    }

    /// `rule`：表的数组，`[[rule]]` 或者写在一行里的 `rule = [{ … }]`。别的写法 `wrong_type`。
    fn rules(&mut self, key: &Key, node: &Node) {
        let tables: Option<Vec<&dyn TableLike>> = match node {
            Node::ArrayOfTables(array) => {
                Some(array.iter().map(|table| table as &dyn TableLike).collect())
            }
            Node::Value(toml_edit::Value::Array(array)) => array
                .iter()
                .map(|value| value.as_inline_table().map(|table| table as &dyn TableLike))
                .collect(),
            _ => None,
        };
        let Some(tables) = tables else {
            let problem = self.item(Code::WrongType, None, RULE, key, node, false);
            self.problems.push(problem);
            return;
        };
        for (index, table) in tables.into_iter().enumerate() {
            self.rule(index + 1, table);
        }
    }

    /// 第 `number` 条规则：`match` 写错的整条不用，属性写错的只丢那一项。
    fn rule(&mut self, number: usize, table: &dyn TableLike) {
        let mut matcher = Some(Match::default());
        let mut attrs = BTreeMap::new();
        for (name, _) in table.iter() {
            let Some((key, node)) = table.get_key_value(name) else {
                continue;
            };
            if name == MATCH {
                matcher = self.matcher(number, key, node);
                continue;
            }
            let Some((known, form)) = attrs::find(name) else {
                let mut problem = self.item(Code::UnknownKey, Some(number), name, key, node, true);
                let names = attrs::ATTRS.iter().map(|(known, _)| *known);
                problem.suggest = nearest(names, name).map(String::from);
                self.problems.push(problem);
                continue;
            };
            let read = node
                .as_value()
                .ok_or(Code::WrongType)
                .and_then(|value| form.read(value));
            match read {
                Ok(value) => {
                    attrs.insert(known, (value, self.at(key.span()).line));
                }
                Err(code) => {
                    let problem = self.item(code, Some(number), name, key, node, false);
                    self.problems.push(problem);
                }
            }
        }
        if let Some(matcher) = matcher {
            self.rules.push(Rule {
                source: self.file.source,
                file: self.file.name.clone(),
                number,
                matcher,
                attrs,
            });
        }
    }

    /// 第 `number` 条规则的 `match`：每一个写错的条件都报，有一个写错就是空的。
    fn matcher(&mut self, number: usize, key: &Key, node: &Node) -> Option<Match> {
        let Some(table) = node.as_table_like() else {
            let problem = self.item(Code::WrongType, Some(number), MATCH, key, node, false);
            self.problems.push(problem);
            return None;
        };
        let mut matcher = Match::default();
        let mut good = true;
        for (name, _) in table.iter() {
            let Some((key, node)) = table.get_key_value(name) else {
                continue;
            };
            // 写错一个以后接着查后面的，只为一次报全；这条规则反正不用了。
            if let Err(code) = matcher.set(name, node.as_value()) {
                let full = format!("{MATCH}.{name}");
                let unknown = code == Code::UnknownKey;
                let mut problem = self.item(code, Some(number), &full, key, node, unknown);
                if unknown {
                    problem.suggest = nearest(CONDITIONS.iter().copied(), name)
                        .map(|near| format!("{MATCH}.{near}"));
                }
                self.problems.push(problem);
                good = false;
            }
        }
        good.then_some(matcher)
    }

    /// 一项的问题：键 `name`，原文照 `node` 取；`at_key` 指到键（不认识的键），不然指到值。
    fn item(
        &self,
        code: Code,
        rule: Option<usize>,
        name: &str,
        key: &Key,
        node: &Node,
        at_key: bool,
    ) -> Problem {
        let key_at = self.at(key.span());
        let (at, raw) = match node {
            Node::Value(value) => {
                let at = match at_key {
                    true => key_at,
                    false => value
                        .span()
                        .map_or(key_at, |span| At::of(self.text, span.start)),
                };
                (at, self.slice(value.span()).unwrap_or_default())
            }
            // 表头这类：位置、原文都照键。
            _ => (key_at, self.slice(key.span()).unwrap_or_else(|| key.get())),
        };
        Problem {
            at: Some(at),
            got: Some(got(raw)),
            ..self.problem(code, rule, Some(name.to_string()))
        }
    }

    /// 这份文件的一条问题，位置、原文、最近的名字、为什么都空着，由调用的一方填。
    fn problem(&self, code: Code, rule: Option<usize>, key: Option<String>) -> Problem {
        Problem {
            code,
            source: self.file.source,
            file: self.file.name.clone(),
            rule,
            key,
            at: None,
            got: None,
            suggest: None,
            why: None,
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

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
