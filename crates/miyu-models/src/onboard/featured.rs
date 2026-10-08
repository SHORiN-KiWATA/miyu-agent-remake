//! 常用的几家（施工 8-11 再补，2026-10-08 项目主人定，`docs/blueprint/models.md`「怎么走」第七条）：资源目录的
//! `models/featured.toml`，`miyu setup` 和头的第一次引导先列它们。核心读好原文交进来，这里只认写法。

use miyu_config::phrases::{Label, read_label};
use toml_edit::{Document, Item};

/// 常用的一家。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Featured {
    /// 目录、档案里的编号。
    pub catalog: String,
    /// 中文界面用的编号（国内的地址）；没写的照 `catalog`。
    pub catalog_zh: Option<String>,
    /// 给人看的名字。
    pub name: Label,
}

impl Featured {
    /// 照界面语言 `language` 挑编号：中文的有国内的用国内的。
    pub fn id(&self, language: &str) -> &str {
        match (&self.catalog_zh, language) {
            (Some(zh), "zh") => zh,
            _ => &self.catalog,
        }
    }
}

/// 读 `featured.toml` 的原文：`[[providers]]` 一家一项，照先后。
///
/// # Errors
///
/// 读不成 TOML；没有 `providers` 那一组；哪一家缺 `catalog`、`name`，写法不对：说是第几家。
pub fn featured(text: &str) -> Result<Vec<Featured>, String> {
    let document = Document::parse(text).map_err(|error| format!("featured.toml: {error}"))?;
    let providers = document
        .get("providers")
        .and_then(Item::as_array_of_tables)
        .ok_or_else(|| "featured.toml: no [[providers]]".to_string())?;
    providers
        .iter()
        .enumerate()
        .map(|(k, table)| {
            let at = |what: &str| format!("featured.toml: provider {}: {what}", k + 1);
            let text_of = |key: &str| table.get(key).and_then(Item::as_str).map(str::to_string);
            let catalog = text_of("catalog").ok_or_else(|| at("catalog must be text"))?;
            let name = table
                .get("name")
                .ok_or_else(|| at("name is missing"))
                .and_then(|item| {
                    read_label(item).map_err(|_| at("name must be text or a language table"))
                })?;
            Ok(Featured {
                catalog,
                catalog_zh: text_of("catalog_zh"),
                name,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
