//! 给人看的字（`docs/blueprint/config.md`「给人看的字」）：每一项的名字、说明、选项名，页和组的名字，和生成
//! 文件要的几句。
//!
//! 字跟着界面语言，住在资源目录的 `core/human/<语言>.json` 里：前几样在 `config` 那一格（[`ConfigWords`]），几句话在
//! `said` 里，编号 `config/…`。这里不碰磁盘，由读资源的那一层读好，照 [`Words`] 交进来。标点、连词也是字：「a、b
//! 或 c」这样的连法照 `config/list`、`config/or`、`config/or-values` 几句接（`cli/main.md`「参数写错时」），不写在代码里。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Deserialize;

use crate::item::{Item, Kind};

/// 一种语言的给人看的字，由读资源的那一层实现（`miyu-store` 的 `Human`）。
pub trait Words {
    /// 键 `key` 这一项的名字、说明、选项名；没有的是空的。
    fn item(&self, key: &str) -> Option<&ItemWords>;

    /// `said` 里编号 `key` 的那一句（例如 `config/or`），照 `fields` 换好；没有这一句、少了字段的是空的。
    fn sentence(&self, key: &str, fields: &[(&str, &str)]) -> Option<String>;
}

/// 资源里 `config` 那一格：一种语言的名字和说明。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigWords {
    /// 每一项：键到它的名字、说明、选项名。
    #[serde(default)]
    pub items: BTreeMap<String, ItemWords>,
    /// 设置页每一页的名字：编号到名字。
    #[serde(default)]
    pub pages: BTreeMap<String, String>,
    /// 设置页每一组的名字：编号到名字。
    #[serde(default)]
    pub groups: BTreeMap<String, String>,
}

/// 一项给人看的字。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemWords {
    /// 名字，例如「界面语言」。
    pub name: String,
    /// 说明：一句或几句，带句末的标点。
    pub description: String,
    /// 选项名：选项到给人看的名字。别的类型没有。
    #[serde(default)]
    pub options: BTreeMap<String, String>,
}

/// 生成文件要的字缺了（资源目录装得不全、旧了）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing(pub String);

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "no words for {}", self.0)
    }
}

impl std::error::Error for Missing {}

/// 查资源里的字和清单对不对得上（G10、`14-配置.md` 第十节），交回每一处不对，一处一句：每一项都有名字、说明，
/// 选项都有名字，用到的页和组都有名字；资源里没有多出来的项、选项、页、组。
pub fn check(items: &[Item], words: &ConfigWords) -> Vec<String> {
    let mut problems = Vec::new();
    for item in items {
        let key = item.key;
        let Some(said) = words.items.get(key) else {
            problems.push(format!("{key}：没有名字和说明"));
            continue;
        };
        if said.name.trim().is_empty() {
            problems.push(format!("{key}：名字是空的"));
        }
        if said.description.trim().is_empty() {
            problems.push(format!("{key}：说明是空的"));
        }
        let options = match item.kind {
            Kind::Option(options) => options,
            _ => &[],
        };
        for option in options {
            if said
                .options
                .get(*option)
                .is_none_or(|name| name.trim().is_empty())
            {
                problems.push(format!("{key}：选项 {option} 没有名字"));
            }
        }
        for option in said.options.keys() {
            if !options.contains(&option.as_str()) {
                problems.push(format!("{key}：资源里多了选项 {option}"));
            }
        }
    }
    let keys: BTreeSet<&str> = items.iter().map(|item| item.key).collect();
    for key in words.items.keys() {
        if !keys.contains(key.as_str()) {
            problems.push(format!("{key}：资源里多了这一项，清单里没有"));
        }
    }
    let pages: BTreeSet<&str> = items.iter().map(|item| item.ui.page).collect();
    let groups: BTreeSet<&str> = items.iter().map(|item| item.ui.group).collect();
    problems.extend(named("页", &pages, &words.pages));
    problems.extend(named("组", &groups, &words.groups));
    problems
}

/// 用到的 `used` 都有名字，`names` 里没有多出来的。
fn named(what: &str, used: &BTreeSet<&str>, names: &BTreeMap<String, String>) -> Vec<String> {
    let mut problems = Vec::new();
    for id in used {
        if names.get(*id).is_none_or(|name| name.trim().is_empty()) {
            problems.push(format!("{what} {id}：没有名字"));
        }
    }
    for id in names.keys() {
        if !used.contains(id.as_str()) {
            problems.push(format!("{what} {id}：资源里多了，清单里没有用到"));
        }
    }
    problems
}

/// 编号 `key` 的那一句，缺了的报缺了。
pub(crate) fn sentence(
    words: &dyn Words,
    key: &str,
    fields: &[(&str, &str)],
) -> Result<String, Missing> {
    words
        .sentence(key, fields)
        .ok_or_else(|| Missing(key.to_string()))
}

/// 键 `key` 这一项的字，缺了的报缺了。
pub(crate) fn item<'a>(words: &'a dyn Words, key: &str) -> Result<&'a ItemWords, Missing> {
    words
        .item(key)
        .ok_or_else(|| Missing(format!("config.items.{key}")))
}

/// 几个里的一个：「a、b 或 c」。一个的就是它，两个用 `or` 那一句接，三个以上前面的用 `config/list` 接。`or` 是
/// `config/or-values`（写成代码的值：中文「或」两边空一格，「trace 或 off」）或者 `config/or`（字：「系统配置或个人设置」）。
pub(crate) fn one_of(words: &dyn Words, parts: &[&str], or: &str) -> Result<String, Missing> {
    let Some((last, rest)) = parts.split_last() else {
        return Ok(String::new());
    };
    let Some((first, middle)) = rest.split_first() else {
        return Ok((*last).to_string());
    };
    let mut listed = (*first).to_string();
    for next in middle {
        listed = sentence(words, "config/list", &[("rest", &listed), ("next", next)])?;
    }
    sentence(words, or, &[("rest", &listed), ("last", last)])
}

/// 能写的几个值，照写法：选项是列出的几个，开关是 `true`、`false`，密钥是两种引用（施工 8-5）。数不完的几种（整数、网址、
/// 名字、引用、列表，施工 8-6）是空的：说成一句话，见 [`expected`]。
pub(crate) fn allowed(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Option(options) => options,
        Kind::Bool => &["true", "false"],
        Kind::Secret => &[r#"{ secret = "…" }"#, r#"{ env = "…" }"#],
        _ => &[],
    }
}

/// 能写什么，说成给人看的话：数得完的几种照 [`allowed`] 连成「a、b 或 c」；整数说范围，网址、名字、引用各一句，列表说
/// 「元素的列表」（施工 8-6，`config/expected/…`）。
pub(crate) fn expected(words: &dyn Words, kind: Kind) -> Result<String, Missing> {
    match kind {
        Kind::Option(_) | Kind::Bool | Kind::Secret => {
            one_of(words, allowed(kind), "config/or-values")
        }
        Kind::Int { min, max } => sentence(
            words,
            "config/expected/int",
            &[("min", &min.to_string()), ("max", &max.to_string())],
        ),
        Kind::Url => sentence(words, "config/expected/url", &[]),
        Kind::Name => sentence(words, "config/expected/name", &[]),
        Kind::Reference => sentence(words, "config/expected/reference", &[]),
        Kind::List(inner) => {
            let item = expected(words, *inner)?;
            sentence(words, "config/expected/list", &[("item", &item)])
        }
    }
}

/// 一项说明后面那几句（`config/facts`）：能写什么、能放在哪几层、什么时候生效。参考文件里是每一项的第二行，
/// JSON Schema 里接在说明后面。
pub(crate) fn facts(words: &dyn Words, item: &Item) -> Result<String, Missing> {
    let values = expected(words, item.kind)?;
    let mut layers = Vec::new();
    for layer in item.layers {
        layers.push(sentence(
            words,
            &format!("config/layer/{}", layer.as_str()),
            &[],
        )?);
    }
    let layers = one_of(
        words,
        &layers.iter().map(String::as_str).collect::<Vec<_>>(),
        "config/or",
    )?;
    let applies = sentence(
        words,
        &format!("config/applies/{}", item.applies.as_str()),
        &[],
    )?;
    sentence(
        words,
        "config/facts",
        &[
            ("values", &values),
            ("layers", &layers),
            ("applies", &applies),
        ],
    )
}

#[cfg(test)]
mod tests;
