//! 值（`docs/blueprint/config.md`「配置清单」）：一项的值 [`Value`]，写成 TOML、写成协议上的 JSON；一份最终值
//! [`Values`]。
//!
//! 现在有字（选项、网址、名字、引用、文字、时长写成字）、开关（施工 8-2）、密钥的引用（施工 8-5）、整数和列表（施工 8-6）、
//! 小数（施工 8-7）：别的写法随用到它的那一步加。设置类型的字段怎么从值变过来：[`Setting`]。网址可能是写死的，也可能是
//! 一个环境变量的引用：[`Address`]（施工 8-6b）。

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
    /// 小数（施工 8-7）。
    Float(Number),
}

/// 一个小数（施工 8-7）：照位比相等，所以值能比、能当键。`nan`、`inf` 读的时候就不收（[`crate::Kind::Float`]）。
#[derive(Debug, Clone, Copy)]
pub struct Number(f64);

impl Number {
    /// 包一个小数。
    pub fn new(number: f64) -> Number {
        Number(number)
    }

    /// 里面的小数。
    pub fn get(self) -> f64 {
        self.0
    }
}

/// 照位比：`0.0` 和 `-0.0` 不一样，读进来的值里不会有 `nan`。
impl PartialEq for Number {
    fn eq(&self, other: &Number) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for Number {}

impl Value {
    /// 写成 TOML：字写成双引号的字符串，照 TOML 转义（引号、反斜杠、控制字符）。
    pub fn toml(&self) -> String {
        match self {
            Value::Text(text) => quoted(text),
            Value::Bool(on) => on.to_string(),
            Value::Secret(reference) => reference.toml(),
            Value::Int(number) => number.to_string(),
            Value::Float(number) => float_toml(number.get()),
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
            Value::Float(number) => serde_json::Value::from(number.get()),
            Value::List(values) => {
                serde_json::Value::Array(values.iter().map(Value::json).collect())
            }
        }
    }
}

/// 选项的设置类型是字：照原样拿出来。最终值都校验过，开关变不成字，不会走到那一支（写成 TOML 的样子）。引用
/// （`{ env = … }`）读成空字，不写死的 TOML 字节，防着字段被悄悄填进一句读不出地址的乱码：网址类型整体认引用（施工
/// 8-6b），网址的字段一律用 [`Address`]（施工 8-8 把 `models.catalog.url` 也换了过来）。
impl From<&Value> for String {
    fn from(value: &Value) -> String {
        match value {
            Value::Text(text) => text.to_string(),
            Value::Secret(_) => String::new(),
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

/// 没有默认值的字（名字、引用、没有默认值的选项）：没有的是空的（施工 8-6）。网址用 [`Address`]，不用它：网址可能是
/// 一个引用，这里读不出环境变量（施工 8-6b）。
impl Setting for Option<String> {
    fn read(value: Option<&Value>) -> Option<String> {
        match value {
            Some(Value::Text(text)) => Some(text.to_string()),
            _ => None,
        }
    }
}

/// 一项网址类型的值（施工 8-6b，`config.md`「类型」网址那一行）：写死的地址，或者一个环境变量的引用。只有
/// `{ env = … }`，没有 `{ secret = … }`：地址不进密钥文件，和它一样只留在拉起核心的环境变量里（第九条第 5 条，
/// `config/environment.rs`）。[`Reference::Env`] 只存变量的名字，取出来的地址只在真要连供应商的那一刻读（`models.md`
/// 「怎么走」第一条），不会在这里被解出来：`config.get`、`model.list` 照写的样子交，不交地址。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Address {
    /// 写死的地址。
    Literal(String),
    /// 环境变量的名字。
    Env(String),
}

/// 没有默认值的网址（施工 8-6b）：写死的照字读；`{ env = … }` 照引用读，`{ secret = … }` 读不出来（[`crate::item::Kind`]
/// 的 `check` 挡在前面，正常不会走到这里）；没写的是空的。
impl Setting for Option<Address> {
    fn read(value: Option<&Value>) -> Option<Address> {
        match value {
            Some(Value::Text(text)) => Some(Address::Literal(text.to_string())),
            Some(Value::Secret(Reference::Env(name))) => Some(Address::Env(name.clone())),
            _ => None,
        }
    }
}

/// 有默认值的网址（施工 8-8，`models.catalog.url`）：同上；读不出来的（正常碰不到）是空的地址。
impl Setting for Address {
    fn read(value: Option<&Value>) -> Address {
        <Option<Address> as Setting>::read(value).unwrap_or_else(|| Address::Literal(String::new()))
    }
}

/// 有默认值的整数（施工 R-6 上：`memory.extract_turns`）：最终值都校验过、总有它；读不成的（只有手写的值里有）是 0。
impl Setting for i64 {
    fn read(value: Option<&Value>) -> i64 {
        match value {
            Some(Value::Int(number)) => *number,
            _ => 0,
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

/// 没有默认值的小数（施工 8-7）。
impl Setting for Option<Number> {
    fn read(value: Option<&Value>) -> Option<Number> {
        match value {
            Some(Value::Float(number)) => Some(*number),
            _ => None,
        }
    }
}

/// 没有默认值的开关（施工 8-7）：没写和写了 `false` 分得开。
impl Setting for Option<bool> {
    fn read(value: Option<&Value>) -> Option<bool> {
        match value {
            Some(Value::Bool(on)) => Some(*on),
            _ => None,
        }
    }
}

/// 时长（施工 8-7）：照 [`crate::item::duration`] 读。最终值都校验过；读不成的（只有手写的值里有）是 0。
impl Setting for std::time::Duration {
    fn read(value: Option<&Value>) -> std::time::Duration {
        match value {
            Some(Value::Text(text)) => crate::item::duration(text).unwrap_or_default(),
            _ => std::time::Duration::ZERO,
        }
    }
}

/// 没有默认值的字的列表（施工 8-7：选项、文字的列表）：没写的是空的，写了空列表的是空的列表；照写的先后，不是字的跳过。
impl Setting for Option<Vec<String>> {
    fn read(value: Option<&Value>) -> Option<Vec<String>> {
        match value {
            Some(Value::List(values)) => Some(
                values
                    .iter()
                    .filter_map(|value| match value {
                        Value::Text(text) => Some(text.to_string()),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        }
    }
}

/// 小数写成 TOML：整数也带上 `.0`，读回来还是小数（TOML 1.0「Float」）。
fn float_toml(number: f64) -> String {
    let text = number.to_string();
    match text.contains(['.', 'e', 'E']) {
        true => text,
        false => format!("{text}.0"),
    }
}

/// 单个密钥（施工 O-8，`onebot.token`）：写了的是它的引用，没写的、不是引用的是空的。
impl Setting for Option<Reference> {
    fn read(value: Option<&Value>) -> Option<Reference> {
        match value {
            Some(Value::Secret(reference)) => Some(reference.clone()),
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
