//! 生成 JSON Schema（`docs/blueprint/config.md`「怎么走」第一条第 9 条，G10）：手改配置文件时编辑器照它补全、
//! 查错，配置文件第一行的 `#:schema` 指向它。
//!
//! draft-07。键照 `.` 分段，前几段是一层层的表（`type: object`），最后一段是这一项的属性：`title` 是名字，
//! `description` 是说明加上能写什么、能放在哪几层、什么时候生效，`type`、`enum`、`default` 照清单。不写
//! `additionalProperties: false`：不认识的键只是警告（G8），编辑器也不该标成错。格照名字的字母先后排，两格缩进，
//! 最后一个换行。

use serde_json::{Map, Value as Json, json};

use crate::item::{Item, Kind, Layer};
use crate::words::{self, Missing, Words};

/// 用的是哪一版 JSON Schema。
const DRAFT: &str = "http://json-schema.org/draft-07/schema#";

/// 能放进 `layer` 那一层的项的 JSON Schema，字照 `words` 那一种语言。
///
/// # Errors
///
/// 要用的字缺了（资源目录装得不全）。
pub fn render(items: &[Item], layer: Layer, words: &dyn Words) -> Result<String, Missing> {
    let mut properties = Map::new();
    for item in items.iter().filter(|item| item.layers.contains(&layer)) {
        let segments: Vec<&str> = item.key.split('.').collect();
        insert(&mut properties, &segments, property(item, words)?);
    }
    let schema = json!({
        "$schema": DRAFT,
        "properties": properties,
        "type": "object",
    });
    Ok(format!("{schema:#}\n"))
}

/// 照一段段的键往下找表（没有的建上），把最后一段放进去。键互不为前缀由 [`crate::list::check`] 守着：这里撞上
/// 一个不是表的，就放不进去。
fn insert(properties: &mut Map<String, Json>, segments: &[&str], leaf: Json) {
    match segments {
        [] => {}
        [last] => {
            properties.insert((*last).to_string(), leaf);
        }
        [table, rest @ ..] => {
            let node = properties
                .entry((*table).to_string())
                .or_insert_with(|| json!({"properties": {}, "type": "object"}));
            if let Some(inner) = node.get_mut("properties").and_then(Json::as_object_mut) {
                insert(inner, rest, leaf);
            }
        }
    }
}

/// 一项的属性。
fn property(item: &Item, words: &dyn Words) -> Result<Json, Missing> {
    let said = words::item(words, item.key)?;
    let facts = words::facts(words, item)?;
    let description = words::sentence(
        words,
        "config/schema-description",
        &[("description", &said.description), ("facts", &facts)],
    )?;
    let mut property = Map::new();
    property.insert("title".to_string(), json!(said.name));
    property.insert("description".to_string(), json!(description));
    property.insert("default".to_string(), item.default.json());
    match item.kind {
        Kind::Option(options) => {
            property.insert("type".to_string(), json!("string"));
            property.insert("enum".to_string(), json!(options));
        }
        Kind::Bool => {
            property.insert("type".to_string(), json!("boolean"));
        }
    }
    Ok(Json::Object(property))
}

#[cfg(test)]
mod tests;
