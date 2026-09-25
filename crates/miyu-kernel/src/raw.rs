//! 原样的 JSON：一个字节都不改，写出去还是原样。
//!
//! 两处要用：驱动私有数据（`docs/designs/03-事件模型.md` 第九节），和读到不认识的种类时
//! 整块留着（第八节）。只能从 serde_json 读，这正是它能一字不差的原因。

use std::collections::BTreeMap;

use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;

#[derive(Debug, Clone)]
pub struct RawJson(Box<RawValue>);

impl RawJson {
    /// 原样的 JSON 文本。
    pub fn get(&self) -> &str {
        self.0.get()
    }
}

/// 两块原样的 JSON，文本一字不差才算相等。
impl PartialEq for RawJson {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl Eq for RawJson {}

impl Serialize for RawJson {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for RawJson {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Box::<RawValue>::deserialize(d).map(RawJson)
    }
}

/// 按 `field` 这个字段（`type` 或 `kind`）分派的读法：先把整块原样读下来，看这个字段的值；
/// `known` 认识这个值就照它读，返回 `None` 表示不认识，整块原样交给 `unknown`。
pub(crate) fn read_tagged<'de, D, T>(
    d: D,
    field: &'static str,
    known: fn(&str, &str) -> Option<serde_json::Result<T>>,
    unknown: fn(RawJson) -> T,
) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = RawJson::deserialize(d)?;
    let fields: BTreeMap<String, &RawValue> =
        serde_json::from_str(raw.get()).map_err(D::Error::custom)?;
    let value = fields
        .get(field)
        .ok_or_else(|| D::Error::custom(format!("缺了 {field} 字段")))?;
    let tag: String = serde_json::from_str(value.get()).map_err(D::Error::custom)?;
    match known(&tag, raw.get()) {
        Some(read) => read.map_err(D::Error::custom),
        None => Ok(unknown(raw)),
    }
}

/// 从一段 JSON 文本读出一种认识的写法。
pub(crate) fn parse<T: DeserializeOwned>(json: &str) -> serde_json::Result<T> {
    serde_json::from_str(json)
}
