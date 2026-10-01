//! 值（`docs/blueprint/config.md`「配置清单」）：一项的值 [`Value`]，写成 TOML、写成协议上的 JSON；一份最终值
//! [`Values`]。
//!
//! 现在有字（选项、网址、名字、引用写成字）、开关（施工 8-2）、密钥的引用（施工 8-5）、整数和列表（施工 8-6）：别的写法随
//! 用到它的那一步加。设置类型的字段怎么从值变过来：[`Setting`]。

use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::item::Item;
use crate::secret::Reference;

/// 一项的值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// 字：选项写成它。清单里的默认值借着写在代码里的字，读进来的自己拿着。
    Text(Cow<'static, str>),
    /// 开关（施工 8-2）。
    Bool(bool),
    /// 密钥的引用（施工 8-5）：只有名字，不是密钥本身。
    Secret(Reference),
    /// 整数（施工 8-6）。
    Int(i64),
    /// 列表（施工 8-6）：每一个照元素的类型。
    List(Vec<Value>),
}

impl Value {
    /// 写成 TOML：字写成双引号的字符串，照 TOML 转义（引号、反斜杠、控制字符）。
    pub fn toml(&self) -> String {
        match self {
            Value::Text(text) => quoted(text),
            Value::Bool(on) => on.to_string(),
            Value::Secret(reference) => reference.toml(),
            Value::Int(number) => number.to_string(),
            Value::List(values) => {
                let values: Vec<String> = values.iter().map(Value::toml).collect();
                format!("[{}]", values.join(", "))
            }
        }
    }

    /// 写成协议上的 JSON。
    pub fn json(&self) -> serde_json::Value {
        match self {
            Value::Text(text) => serde_json::Value::String(text.to_string()),
            Value::Bool(on) => serde_json::Value::Bool(*on),
            Value::Secret(reference) => reference.json(),
            Value::Int(number) => serde_json::Value::from(*number),
            Value::List(values) => {
                serde_json::Value::Array(values.iter().map(Value::json).collect())
            }
        }
    }
}

/// 选项的设置类型是字：照原样拿出来。最终值都校验过，开关、引用变不成字，不会走到那一支（写成 TOML 的样子）。
impl From<&Value> for String {
    fn from(value: &Value) -> String {
        match value {
            Value::Text(text) => text.to_string(),
            other => other.toml(),
        }
    }
}

/// 开关的设置类型是 `bool`。最终值都校验过，字变不成开关，不会走到那一支（当成关着）。
impl From<&Value> for bool {
    fn from(value: &Value) -> bool {
        matches!(value, Value::Bool(true))
    }
}

/// 设置类型的一个字段怎么从最终值里的值变过来（[`settings!`](crate::settings) 生成的 `at` 用它）：`value` 是最终值里的
/// 那一项，没有的照默认值，默认值也没有的是空的。最终值都校验过，类型对不上的情形只在手写的值里有，照「没有」读。
pub trait Setting: Sized {
    /// 照值读。
    fn read(value: Option<&Value>) -> Self;
}

/// 选项：照原样拿出来（[`From<&Value>`]）。没有的是空字。
impl Setting for String {
    fn read(value: Option<&Value>) -> String {
        value.map(String::from).unwrap_or_default()
    }
}

/// 开关。没有的是关着。
impl Setting for bool {
    fn read(value: Option<&Value>) -> bool {
        value.is_some_and(bool::from)
    }
}

/// 没有默认值的字（网址、名字、引用、没有默认值的选项）：没有的是空的（施工 8-6）。
impl Setting for Option<String> {
    fn read(value: Option<&Value>) -> Option<String> {
        match value {
            Some(Value::Text(text)) => Some(text.to_string()),
            _ => None,
        }
    }
}

/// 没有默认值的整数（施工 8-6）。
impl Setting for Option<i64> {
    fn read(value: Option<&Value>) -> Option<i64> {
        match value {
            Some(Value::Int(number)) => Some(*number),
            _ => None,
        }
    }
}

/// 密钥的列表（施工 8-6）：照写的先后，不是引用的跳过。
impl Setting for Vec<Reference> {
    fn read(value: Option<&Value>) -> Vec<Reference> {
        match value {
            Some(Value::List(values)) => values
                .iter()
                .filter_map(|value| match value {
                    Value::Secret(reference) => Some(reference.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// TOML 的基本字符串：两头双引号，引号、反斜杠转义，控制字符写成转义（TOML 1.0「String」：基本字符串里除了制表，
/// 控制字符都不许照原样写）。
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 一份最终值：键到值。设置类型从它变过来（[`settings!`](crate::settings)），没有的项照默认值。分层合并交出它
/// （[`crate::merge`]，施工 8-2）；全是默认值的一份是 [`Values::defaults`]。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Values {
    map: BTreeMap<String, Value>,
}

impl Values {
    /// 清单 `items` 里每一项都照默认值：没有默认值的、键里有人起的名字那一段的不在里面。
    pub fn defaults(items: &[Item]) -> Values {
        Values {
            map: items
                .iter()
                .filter(|item| !item.is_pattern())
                .filter_map(|item| Some((item.key.to_string(), item.default.clone()?)))
                .collect(),
        }
    }

    /// 全部真的键，照字节排（施工 8-6：找人起的名字用，[`crate::key::names`]）。
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(String::as_str)
    }

    /// 键 `key` 的值；没有的是空的。
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.map.get(key)
    }

    /// 键 `key` 的值换成 `value`。
    pub fn set(&mut self, key: &str, value: Value) {
        self.map.insert(key.to_string(), value);
    }
}

#[cfg(test)]
mod tests;
