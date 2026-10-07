//! 键（`docs/blueprint/config.md`「配置清单」，施工 8-6）：清单里的键可以有一段是人起的名字，例如
//! `providers.<id>.base_url`、`providers.<id>.models.<model>.window`（`models.md`「对外的样子」）。
//!
//! - 清单里写的是「样子」：每一段用 `.` 隔开，人起的名字那一段写成占位 [`ID`] 或 [`MODEL`]。
//! - 文件里、协议上、最终值里的是「真的键」：照 TOML 点号连着的键的写法，裸着能写的一段（只有字母、数字、`-`、`_`）
//!   照写，别的写成带双引号的一段：`providers.dev.models."deepseek-v4.1-flash".window`。[`split`]、[`join`] 互为来回。
//! - 一个真的键对不对得上一个样子：[`fit`]。占位照它的写法查：[`ID`] 是「路径里的名字」（`kernel/ids.md`），[`MODEL`] 是
//!   「短名字」（1 到 128 字节，没有控制字符）。

use crate::value::Value;

/// 占位：一家供应商、一个池这类「路径里的名字」：小写字母开头，只有小写字母、数字、`-`、`_`，最长 32 个字符，不是
/// Windows 的保留名（它要当 `state/` 下的文件名，`models.md`）。
pub const ID: &str = "<id>";

/// 占位：模型名，照供应商那边的叫法：1 到 128 字节，没有控制字符。
pub const MODEL: &str = "<model>";

/// 占位：通讯平台上的身份（施工 O-3，主人对应表 `external.bindings.<external>`），照短名字的写法：1 到 128 字节，没有控制字符，
/// 和 [`MODEL`] 一个规矩（`kernel/ids.md` 的 `ExternalId`）。
pub const EXTERNAL: &str = "<external>";

/// Windows 上这些名字建不了同名的文件（`kernel/ids.md`）。
const WINDOWS_RESERVED: [&str; 22] = [
    "con", "nul", "aux", "prn", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// 一段是不是占位。
pub fn is_placeholder(segment: &str) -> bool {
    segment == ID || segment == MODEL || segment == EXTERNAL
}

/// 样子 `pattern` 里有没有占位：没有的是写死的键，一项只有一个值。
pub fn is_pattern(pattern: &str) -> bool {
    pattern.split('.').any(is_placeholder)
}

/// 名字 `name` 合不合占位 `placeholder` 的写法。不是占位的不合。
pub fn valid(placeholder: &str, name: &str) -> bool {
    match placeholder {
        ID => {
            name.starts_with(|c: char| c.is_ascii_lowercase())
                && name.len() <= 32
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
                && !WINDOWS_RESERVED.contains(&name)
        }
        MODEL | EXTERNAL => {
            !name.is_empty() && name.len() <= 128 && !name.chars().any(char::is_control)
        }
        _ => false,
    }
}

/// 一个真的键和一个样子对得上的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fit {
    /// 对上了：占位那几段依次填的名字。
    Yes(Vec<String>),
    /// 段数、写死的几段都对上了，可第 `at` 段（从 0 数）填的名字不合那个占位的写法。
    BadName {
        /// 第几段。
        at: usize,
        /// 那个占位。
        placeholder: &'static str,
    },
    /// 对不上。
    No,
}

/// 一段段的真的键 `segments` 和样子 `pattern` 对不对得上（段数一样才算）。
pub fn fit(pattern: &'static str, segments: &[String]) -> Fit {
    let parts: Vec<&'static str> = pattern.split('.').collect();
    if parts.len() != segments.len() {
        return Fit::No;
    }
    fit_prefix(&parts, segments)
}

/// 同 [`fit`]，只比前面几段：`segments` 是样子开头那几段（一组键），比样子短也算。
pub fn fit_start(pattern: &'static str, segments: &[String]) -> Fit {
    let parts: Vec<&'static str> = pattern.split('.').collect();
    if parts.len() <= segments.len() {
        return Fit::No;
    }
    fit_prefix(&parts[..segments.len()], segments)
}

/// 照 `parts` 一段段比：写死的段要一样，占位的段先记下名字，写法不对的记下第一个。
fn fit_prefix(parts: &[&'static str], segments: &[String]) -> Fit {
    let mut names = Vec::new();
    let mut bad = None;
    for (at, (part, segment)) in parts.iter().zip(segments).enumerate() {
        if is_placeholder(part) {
            if bad.is_none() && !valid(part, segment) {
                bad = Some((at, *part));
            }
            names.push(segment.clone());
        } else if part != segment {
            return Fit::No;
        }
    }
    match bad {
        Some((at, placeholder)) => Fit::BadName { at, placeholder },
        None => Fit::Yes(names),
    }
}

/// 真的键 `key` 是清单 `items` 里的哪一项：写死的照原样比，键里有人起的名字的照样子对（名字的写法要对）。都对不上、
/// 读不成的是空的。
pub fn item_of<'a>(items: &'a [crate::Item], key: &str) -> Option<&'a crate::Item> {
    if let Some(item) = items.iter().find(|item| item.key == key) {
        return Some(item);
    }
    let segments = split(key)?;
    items
        .iter()
        .find(|item| matches!(fit(item.key, &segments), Fit::Yes(_)))
}

/// 照样子 `pattern` 填上名字 `names`，交回真的键：占位依次换成名字，要加引号的加上。名字不够的，剩下的占位照原样。
pub fn fill(pattern: &str, names: &[&str]) -> String {
    let mut names = names.iter();
    let segments: Vec<String> = pattern
        .split('.')
        .map(|part| match is_placeholder(part) {
            true => names.next().map_or(part, |name| name).to_string(),
            false => part.to_string(),
        })
        .collect();
    join(&segments)
}

/// 一段能不能裸着写：只有 ASCII 字母、数字、`-`、`_`，不是空的（TOML 的裸键）。
fn bare(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// 一段段接成真的键：能裸着写的照写，别的写成 TOML 的基本字符串。
pub fn join<S: AsRef<str>>(segments: &[S]) -> String {
    segments
        .iter()
        .map(|segment| {
            let segment = segment.as_ref();
            match bare(segment) {
                true => segment.to_string(),
                false => Value::Text(segment.to_string().into()).toml(),
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

/// 真的键拆成一段段：照 TOML 点号连着的键读（裸的、带双引号的、带单引号的，`.` 两边可以有空格）。读不成的是空的。
pub fn split(key: &str) -> Option<Vec<String>> {
    let document = toml_edit::Document::parse(format!("{key} = 0")).ok()?;
    let mut segments = Vec::new();
    let mut node = document.as_item();
    loop {
        let table = node.as_table_like()?;
        let mut entries = table.iter();
        let (name, next) = entries.next()?;
        if entries.next().is_some() {
            return None;
        }
        segments.push(name.to_string());
        if next.is_value() {
            return Some(segments);
        }
        node = next;
    }
}

/// 一份键里，对得上样子 `pattern` 前几段、前面几个占位依次是 `filled` 的，下一个占位填的名字有哪几个（照名字排、不重复）。
/// 例如 `names(keys, "providers.<id>", &[])` 是配了的每一家供应商。
pub fn names<'a>(
    keys: impl IntoIterator<Item = &'a str>,
    pattern: &str,
    filled: &[&str],
) -> Vec<String> {
    let parts: Vec<&str> = pattern.split('.').collect();
    let mut found = std::collections::BTreeSet::new();
    for key in keys {
        let Some(segments) = split(key) else {
            continue;
        };
        if segments.len() <= parts.len() {
            continue;
        }
        let mut filled = filled.iter();
        let mut last = None;
        let fits = parts.iter().zip(&segments).all(|(part, segment)| {
            if !is_placeholder(part) {
                return part == segment;
            }
            match filled.next() {
                Some(name) => name == segment,
                None => {
                    last = Some(segment.clone());
                    true
                }
            }
        });
        if fits && let Some(name) = last {
            found.insert(name);
        }
    }
    found.into_iter().collect()
}

#[cfg(test)]
mod tests;
