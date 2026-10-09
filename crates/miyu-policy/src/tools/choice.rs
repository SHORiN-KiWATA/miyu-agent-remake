//! 工具面上一件的一个参数，照会话开局时的配置填上能选的几个（施工 8-8 补，`docs/blueprint/models.md`「工具」、
//! `tools/subagent.md`「会话开局时拼 `pool`」）：现在只有 `subagent` 的 `pool`，执行器造会话时调。
//!
//! 填好的进快照，整个会话照它发；读回能选的几个也照快照（[`ToolEntry::offered`]），载入不重拼。
//!
//! 参数格式是原样的 JSON（[`RawJson`]）：照原样一格格搬，只变要填的那一格。经 `serde_json::Value` 转一道不行：它的对象照字母
//! 排键，别的参数、别的格的字节都会变（`models.md`「施工时定的」8-8 补）。

use std::fmt;

use miyu_kernel::raw::RawJson;
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, Serializer};
use serde_json::value::RawValue;

use super::ToolEntry;

/// 能选的一个：名字（填进 `enum`），给模型看的一句（没写的没有）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// 名字：她写进这个参数的就是它。
    pub name: String,
    /// 给模型看的一句：参数说明后面一行 `<名字>: <它>`。没写的那一行只有名字。
    pub description: Option<String>,
}

impl ToolEntry {
    /// 照 `choices` 填参数 `parameter`：一个都没有的，参数格式里拿掉它；有的，它的 `type` 后面插 `enum`（照交进来的先后），
    /// `description` 后面每个接一行 `\n<名字>: <说明>`，没写说明的只接 `\n<名字>`（没有 `description` 的照这几行写一格，
    /// 接在最后）。别的格、别的参数一个字节不动。参数格式不是这个样子的（不是对象、没有这个参数、这个参数不是对象）照原样。
    pub fn offer(&mut self, parameter: &str, choices: &[Choice]) {
        if let Some(filled) = offer(&self.parameters, parameter, choices) {
            self.parameters = filled;
        }
    }

    /// 参数格式里拿掉参数 `parameter`（施工 T-1 上：后台运行关着的 `shell` 没有 `run_in_background`）：同一个都不给选的
    /// [`ToolEntry::offer`]，别的字节不动。
    pub fn without(&mut self, parameter: &str) {
        self.offer(parameter, &[]);
    }

    /// 参数 `parameter` 能选的几个：它的 `enum` 里的字，照写的先后。快照里没有这一格、写的不是字的，是空的。
    pub fn offered(&self, parameter: &str) -> Vec<String> {
        let found = || -> Option<Vec<String>> {
            let top = Ordered::read(self.parameters.get())?;
            let properties = Ordered::read(top.get("properties")?.get())?;
            let one = Ordered::read(properties.get(parameter)?.get())?;
            serde_json::from_str(one.get("enum")?.get()).ok()
        };
        found().unwrap_or_default()
    }
}

/// 填好的参数格式；不是能填的样子的没有。
fn offer(parameters: &RawJson, parameter: &str, choices: &[Choice]) -> Option<RawJson> {
    let mut top = Ordered::read(parameters.get())?;
    let mut properties = Ordered::read(top.get("properties")?.get())?;
    let at = properties.position(parameter)?;
    if choices.is_empty() {
        properties.0.remove(at);
    } else {
        let one = Ordered::read(properties.0[at].1.get())?;
        properties.0[at].1 = raw(&filled(one, choices)?)?;
    }
    top.set("properties", raw(&properties)?);
    serde_json::from_str(&serde_json::to_string(&top).ok()?).ok()
}

/// 一个参数填上 `choices`：`type` 后面插 `enum`（原来有的换掉），`description` 后面接几行。
fn filled(one: Ordered, choices: &[Choice]) -> Option<Ordered> {
    let names: Vec<&str> = choices.iter().map(|choice| choice.name.as_str()).collect();
    let lines: Vec<String> = choices
        .iter()
        .map(|choice| match &choice.description {
            Some(said) => format!("{}: {said}", choice.name),
            None => choice.name.clone(),
        })
        .collect();
    let mut out = Ordered(Vec::new());
    let mut described = false;
    for (key, value) in one.0 {
        match key.as_str() {
            "enum" => continue,
            "description" => {
                let said: String = serde_json::from_str(value.get()).ok()?;
                let text = std::iter::once(said).chain(lines.iter().cloned());
                out.0
                    .push((key, raw(&text.collect::<Vec<_>>().join("\n"))?));
                described = true;
            }
            "type" => {
                out.0.push((key, value));
                out.0.push(("enum".to_string(), raw(&names)?));
            }
            _ => out.0.push((key, value)),
        }
    }
    if out.position("enum").is_none() {
        out.0.insert(0, ("enum".to_string(), raw(&names)?));
    }
    if !described {
        out.0
            .push(("description".to_string(), raw(&lines.join("\n"))?));
    }
    Some(out)
}

/// 写成原样的一块 JSON。
fn raw<T: Serialize + ?Sized>(value: &T) -> Option<Box<RawValue>> {
    serde_json::value::to_raw_value(value).ok()
}

/// 一个 JSON 对象，照写的先后留着每一格，格的值原样。
struct Ordered(Vec<(String, Box<RawValue>)>);

impl Ordered {
    /// 读一个对象；不是对象的没有。
    fn read(text: &str) -> Option<Ordered> {
        serde_json::from_str(text).ok()
    }

    /// 叫 `key` 的那一格是第几个。
    fn position(&self, key: &str) -> Option<usize> {
        self.0.iter().position(|(name, _)| name == key)
    }

    /// 叫 `key` 的那一格的值。
    fn get(&self, key: &str) -> Option<&RawValue> {
        self.0
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| &**value)
    }

    /// 换掉叫 `key` 的那一格的值，位置不变。
    fn set(&mut self, key: &str, value: Box<RawValue>) {
        if let Some(at) = self.position(key) {
            self.0[at].1 = value;
        }
    }
}

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Ordered, D::Error> {
        deserializer.deserialize_map(OrderedVisitor)
    }
}

/// 照先后读一个对象的每一格。
struct OrderedVisitor;

impl<'de> Visitor<'de> for OrderedVisitor {
    type Value = Ordered;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
        let mut pairs = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, Box<RawValue>>()? {
            pairs.push((key, value));
        }
        Ok(Ordered(pairs))
    }
}

impl Serialize for Ordered {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests;
