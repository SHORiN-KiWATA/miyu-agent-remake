//! 引用的东西没有（`docs/blueprint/config.md`「报错」的 `bad_reference`，`models.md`「怎么走」第三条第 2 条，施工 8-8）：
//! 引用（`models.chat`、挡位的值）、模型的列表（池的成员）指的供应商、池，在合出来的最终值里没有。
//!
//! 这是跨项的查：一项写在个人设置里，指的供应商可以配在系统配置里，所以照最终值查，不照这一份文件查。和引用的密钥、
//! 环境变量取不到（[`crate::secret::missing`]）一样，读进来以后另查一遍、只报不丢：值照样用，路由当场照它说 `no_model`
//! 和为什么（`models.md`「怎么走」第一条第 7 条）。写法不对的（`bad_format`）解析时已经丢了，这里碰不到。
//!
//! 什么算「有」由用它的一方说（`provider`、`pool` 两个闭包）：这一层只认引用的写法，不认识模型那一块的键。

use crate::item::{Item, Kind, Layer};
use crate::item::{Pointed, pointed};
use crate::parse::{Entry, Parsed};
use crate::problem::{Code, Problem};
use crate::value::Value;

/// 一层配置 `parsed`（当成 `layer` 读的）里，引用、模型的列表指的供应商、池没有的：一处一条，供应商的是 `NoProvider`、
/// 池的是 `NoPool`（协议上都是 `bad_reference`，错误），`name` 是指的那个名字。只看这一层算数的项，列表里每一个各查
/// 各的。`provider` 说一家供应商配没配，`pool` 说一个池配没配。
pub fn dangling(
    items: &[Item],
    parsed: &Parsed,
    layer: Layer,
    provider: &dyn Fn(&str) -> bool,
    pool: &dyn Fn(&str) -> bool,
) -> Vec<Problem> {
    let mut found = Vec::new();
    for (key, entry) in parsed.entries.iter().filter(|(_, entry)| entry.counts) {
        let Some(item) = items.iter().find(|item| item.key == entry.item) else {
            continue;
        };
        let texts: Vec<&str> = match (item.kind, &entry.value) {
            // `model_or` 列出的那几个字照引用读不出指的是什么，下面自然跳过（施工 R-5 补）。
            (Kind::Reference | Kind::Model | Kind::ModelOr(_), Value::Text(text)) => {
                vec![text.as_ref()]
            }
            (Kind::List(Kind::Reference | Kind::Model), Value::List(values)) => values
                .iter()
                .filter_map(|value| match value {
                    Value::Text(text) => Some(text.as_ref()),
                    _ => None,
                })
                .collect(),
            _ => continue,
        };
        for text in texts {
            let (code, name) = match pointed(text) {
                Some(Pointed::Provider(name)) if !provider(name) => (Code::NoProvider, name),
                Some(Pointed::Pool(name)) if !pool(name) => (Code::NoPool, name),
                _ => continue,
            };
            found.push(problem(code, layer, key, entry, name));
        }
    }
    found
}

/// 一条：在这一项的值那里，`name` 是指的那个。
fn problem(code: Code, layer: Layer, key: &str, entry: &Entry, name: &str) -> Problem {
    let mut problem = Problem::item(code, layer, key, entry.at, &entry.raw);
    problem.name = Some(name.to_string());
    problem
}

#[cfg(test)]
mod tests;
