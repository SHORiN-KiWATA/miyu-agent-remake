//! 查清单写得对不对（`docs/blueprint/config.md`「怎么走」第一条第 2 到 4 条，G10）：两个键不指同一件事，键合
//! 写法，默认值过自己的校验，能放进项目配置的写了怎么收紧（「收紧」）。清单写在代码里，写错了是程序的错：核心的测试照登记的全部清单查一遍，不在起来时查。

use std::collections::BTreeSet;

use crate::item::{Item, Kind, Layer, Tighten};

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
        problems.extend(tighten_problem(item));
        if let Some(default) = &item.default
            && !item.kind.accepts(default)
        {
            problems.push(format!("{key}：默认值 {} 过不了自己的校验", default.toml()));
        }
        if item.is_pattern() && item.env.is_some() {
            problems.push(format!("{key}：键里有人起的名字，不能由环境变量压过"));
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

/// 键的写法（第一条第 2 条）：至少两段；每一段小写字母开头，只有小写字母、数字、`_`，或者是人起的名字的占位
/// `<id>`、`<model>`（施工 8-6，不能是第一段、不能是最后一段）；第一段不是 `ext`。
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
    let last = segments.len() - 1;
    if segments
        .iter()
        .enumerate()
        .any(|(at, segment)| crate::key::is_placeholder(segment) && (at == 0 || at == last))
    {
        return Some("人起的名字那一段不能是第一段、最后一段");
    }
    if !segments
        .iter()
        .all(|segment| good(segment) || crate::key::is_placeholder(segment))
    {
        return Some("每一段要小写字母开头，只有小写字母、数字、_");
    }
    if segments[0] == EXTENSIONS {
        return Some("第一段 ext 留给扩展");
    }
    None
}

/// 收紧写得对不对：能放进项目配置的必写，别的不写；「只能打开」只给开关。
fn tighten_problem(item: &Item) -> Option<String> {
    let key = item.key;
    let project = item.layers.contains(&Layer::Project);
    match (project, item.tighten, item.kind) {
        (true, None, _) => Some(format!("{key}：能放进项目配置，要写怎么收紧")),
        (false, Some(_), _) => Some(format!("{key}：不能放进项目配置，不写收紧")),
        (true, Some(Tighten::TrueOnly), kind) if kind != Kind::Bool => {
            Some(format!("{key}：只能打开只给开关"))
        }
        _ => None,
    }
}

/// 类型本身写得对不对：选项至少两个、不重复；整数、小数的最小不比最大大，时长的最短大于 0、不比最长长，文字至少一个字
/// （施工 8-7）；列表的元素不是列表。
fn kind_problems(item: &Item) -> Vec<String> {
    match item.kind {
        Kind::Int { min, max } | Kind::Float { min, max } if min > max => {
            vec![format!("{}：最小比最大大", item.key)]
        }
        Kind::Duration { min, max } if min > max || min == 0 => {
            vec![format!("{}：时长的最短要大于 0、不比最长长", item.key)]
        }
        Kind::Text { max: 0 } => vec![format!("{}：文字至少能写一个字", item.key)],
        Kind::List(Kind::List(_)) => vec![format!("{}：列表的元素不能是列表", item.key)],
        Kind::Bool
        | Kind::Secret
        | Kind::Int { .. }
        | Kind::Float { .. }
        | Kind::Text { .. }
        | Kind::Duration { .. }
        | Kind::Url
        | Kind::Name
        | Kind::Reference
        | Kind::List(_) => Vec::new(),
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
