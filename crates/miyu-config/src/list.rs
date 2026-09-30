//! 查清单写得对不对（`docs/blueprint/config.md`「怎么走」第一条第 2 到 4 条，G10）：两个键不指同一件事，键合
//! 写法，默认值过自己的校验。清单写在代码里，写错了是程序的错：核心的测试照登记的全部清单查一遍，不在起来时查。

use std::collections::BTreeSet;

use crate::item::{Item, Kind};

/// 留给扩展的第一段：内置的模块不许用（`14-配置.md` 第二节）。
const EXTENSIONS: &str = "ext";

/// 查一遍清单，交回每一处不对，一处一句。空的就是对的。
pub fn check(items: &[Item]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for item in items {
        let key = item.key;
        if !seen.insert(key) {
            problems.push(format!("{key}：键重复了"));
        }
        if let Some(why) = key_problem(key) {
            problems.push(format!("{key}：{why}"));
        }
        if item.layers.is_empty() {
            problems.push(format!("{key}：一层都不能放"));
        }
        if item.layers.iter().collect::<BTreeSet<_>>().len() != item.layers.len() {
            problems.push(format!("{key}：层写重了"));
        }
        problems.extend(kind_problems(item));
        if !item.kind.accepts(&item.default) {
            problems.push(format!(
                "{key}：默认值 {} 过不了自己的校验",
                item.default.toml()
            ));
        }
    }
    for item in items {
        for other in items {
            if other
                .key
                .strip_prefix(item.key)
                .is_some_and(|rest| rest.starts_with('.'))
            {
                problems.push(format!(
                    "{}：{} 是它的前缀，{} 那一格说不清是表还是值",
                    other.key, item.key, item.key
                ));
            }
        }
    }
    problems
}

/// 键的写法（第一条第 2 条）：至少两段；每一段小写字母开头，只有小写字母、数字、`_`；第一段不是 `ext`。
fn key_problem(key: &str) -> Option<&'static str> {
    let segments: Vec<&str> = key.split('.').collect();
    if segments.len() < 2 {
        return Some("至少两段：第一段是模块的编号");
    }
    let good = |segment: &str| {
        segment.starts_with(|c: char| c.is_ascii_lowercase())
            && segment
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    if !segments.iter().all(|segment| good(segment)) {
        return Some("每一段要小写字母开头，只有小写字母、数字、_");
    }
    if segments[0] == EXTENSIONS {
        return Some("第一段 ext 留给扩展");
    }
    None
}

/// 类型本身写得对不对：选项至少两个、不重复。
fn kind_problems(item: &Item) -> Vec<String> {
    match item.kind {
        Kind::Option(options) => {
            let mut problems = Vec::new();
            if options.len() < 2 {
                problems.push(format!("{}：选项至少两个", item.key));
            }
            if options.iter().collect::<BTreeSet<_>>().len() != options.len() {
                problems.push(format!("{}：选项写重了", item.key));
            }
            problems
        }
    }
}

#[cfg(test)]
mod tests;
