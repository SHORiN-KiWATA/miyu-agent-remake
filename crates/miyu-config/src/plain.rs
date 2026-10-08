//! 没有清单的 TOML 文件怎么读一项、怎么从协议上的 JSON 换成要写的值（施工 P-3 中）：人格、预设的文件不在配置清单里，键和值
//! 照 TOML 原样。改哪一项照旧用 [`crate::edit::apply`]，这里只补它没有的两样：
//!
//! - [`json_at`]：一份字里某个键现在的值，写成 JSON（`expect` 照它比）。键照 [`crate::key::split`] 拆，表、行内表、点号连着
//!   的键都认；没有这一项、字读不懂的是空的。
//! - [`from_json`]：协议上的一个值换成要写的值：字、开关、整数、小数；别的（`null`、数组、对象）是空的，由调用的一方当参数
//!   不对。

use std::borrow::Cow;

use toml_edit::{Document, Item, Value as TomlValue};

use crate::value::{Number, Value};

/// 字 `text` 里键 `key` 现在的值，写成 JSON。只认字、开关、整数、小数、它们的数组；没有这一项、是表、字读不懂的是空的。
pub fn json_at(text: &str, key: &str) -> Option<serde_json::Value> {
    let document = Document::parse(text).ok()?;
    let path = crate::key::split(key)?;
    let (last, group) = path.split_last()?;
    let mut table = document.as_table() as &dyn toml_edit::TableLike;
    for name in group {
        table = table.get(name)?.as_table_like()?;
    }
    match table.get(last)? {
        Item::Value(value) => json_of(value),
        _ => None,
    }
}

/// 协议上的一个值换成要写的值：字、开关、整数、小数。别的是空的。
pub fn from_json(value: &serde_json::Value) -> Option<Value> {
    match value {
        serde_json::Value::String(text) => Some(Value::Text(Cow::Owned(text.clone()))),
        serde_json::Value::Bool(on) => Some(Value::Bool(*on)),
        serde_json::Value::Number(number) => number.as_i64().map(Value::Int).or_else(|| {
            number
                .as_f64()
                .map(|number| Value::Float(Number::new(number)))
        }),
        _ => None,
    }
}

/// 一个 TOML 的值写成 JSON：字、开关、整数、小数、它们的数组。
fn json_of(value: &TomlValue) -> Option<serde_json::Value> {
    match value {
        TomlValue::String(text) => Some(serde_json::Value::String(text.value().clone())),
        TomlValue::Boolean(on) => Some(serde_json::Value::Bool(*on.value())),
        TomlValue::Integer(number) => Some(serde_json::Value::from(*number.value())),
        TomlValue::Float(number) => {
            serde_json::Number::from_f64(*number.value()).map(serde_json::Value::Number)
        }
        TomlValue::Array(array) => array
            .iter()
            .map(json_of)
            .collect::<Option<Vec<_>>>()
            .map(serde_json::Value::Array),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
