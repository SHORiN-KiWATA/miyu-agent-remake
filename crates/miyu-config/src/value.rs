//! 值（`docs/blueprint/config.md`「配置清单」）：一项的值 [`Value`]，写成 TOML、写成协议上的 JSON；一份最终值
//! [`Values`]。
//!
//! 现在只有字（选项写成字）：别的写法随用到它的那一步加。

use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::item::Item;

/// 一项的值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// 字：选项写成它。清单里的默认值借着写在代码里的字，读进来的自己拿着。
    Text(Cow<'static, str>),
}

impl Value {
    /// 写成 TOML：字写成双引号的字符串，照 TOML 转义（引号、反斜杠、控制字符）。
    pub fn toml(&self) -> String {
        match self {
            Value::Text(text) => quoted(text),
        }
    }

    /// 写成协议上的 JSON。
    pub fn json(&self) -> serde_json::Value {
        match self {
            Value::Text(text) => serde_json::Value::String(text.to_string()),
        }
    }
}

/// 选项的设置类型是字：照原样拿出来。
impl From<&Value> for String {
    fn from(value: &Value) -> String {
        match value {
            Value::Text(text) => text.to_string(),
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

/// 一份最终值：键到值。设置类型从它变过来（[`settings!`](crate::settings)），没有的项照默认值。8-2 的分层合并
/// 交出它；现在只有默认值（[`Values::defaults`]）。
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
}

#[cfg(test)]
mod tests;
