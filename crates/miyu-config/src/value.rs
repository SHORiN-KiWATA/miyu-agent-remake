//! 值（`docs/blueprint/config.md`「配置清单」）：一项的值 [`Value`]，写成 TOML、写成协议上的 JSON；一份最终值
//! [`Values`]。
//!
//! 现在有字（选项写成字）、开关（施工 8-2）和密钥的引用（施工 8-5）：别的写法随用到它的那一步加。

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
}

impl Value {
    /// 写成 TOML：字写成双引号的字符串，照 TOML 转义（引号、反斜杠、控制字符）。
    pub fn toml(&self) -> String {
        match self {
            Value::Text(text) => quoted(text),
            Value::Bool(on) => on.to_string(),
            Value::Secret(reference) => reference.toml(),
        }
    }

    /// 写成协议上的 JSON。
    pub fn json(&self) -> serde_json::Value {
        match self {
            Value::Text(text) => serde_json::Value::String(text.to_string()),
            Value::Bool(on) => serde_json::Value::Bool(*on),
            Value::Secret(reference) => reference.json(),
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
    /// 清单 `items` 里每一项都照默认值。
    pub fn defaults(items: &[Item]) -> Values {
        Values {
            map: items
                .iter()
                .map(|item| (item.key.to_string(), item.default.clone()))
                .collect(),
        }
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
