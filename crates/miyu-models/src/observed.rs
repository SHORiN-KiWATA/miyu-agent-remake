//! 用出来的、供应商的列表（`docs/blueprint/models.md`「怎么走」第二条第 9、10 条，「文件」那张表，施工 8-7）：两种派生
//! 数据的样子和读写。文件在 `state/models/` 下，由执行器读写（先写临时文件再改名，坏了当没有）；这里只管字和合并。
//!
//! - `learned.json`：`{"<供应商>/<模型>":{"window":{"value":…,"at":…}}}`。请求报上下文超长、报了上限的，记下真窗口；
//!   只学窗口，一直留着，比现在的小才记。
//! - `providers/<编号>.json`：`{"fetched":…,"models":[{"id":…,"window":…}]}`。供应商的模型列表。

use std::collections::BTreeMap;

use miyu_kernel::time::Timestamp;
use serde::{Deserialize, Serialize};

/// 用出来的：`<供应商>/<模型>` → 这个模型学到的。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Learned(BTreeMap<String, LearnedModel>);

/// 一个模型学到的：现在只有窗口。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearnedModel {
    /// 窗口。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<Stamped>,
}

/// 一个学到的数，和什么时候学到的。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamped {
    /// 数。
    pub value: u64,
    /// 什么时候记下的。
    pub at: Timestamp,
}

impl Learned {
    /// 读 `learned.json`。
    ///
    /// # Errors
    ///
    /// 不是这个样子的 JSON：原话说是哪一份。
    pub fn parse(text: &str) -> Result<Learned, String> {
        serde_json::from_str(text).map_err(|error| format!("learned.json not readable: {error}"))
    }

    /// 写成 JSON，照键排。
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// 这家这个模型学到的窗口。
    pub fn window(&self, provider: &str, model: &str) -> Option<Stamped> {
        self.0.get(&key(provider, model))?.window
    }

    /// 记一个窗口 `window`（`at` 时学到的）：比已经学到的小、或者还没学到才记，交回记没记。
    pub fn learn(&mut self, provider: &str, model: &str, window: u64, at: Timestamp) -> bool {
        let entry = self.0.entry(key(provider, model)).or_default();
        if entry.window.is_some_and(|known| known.value <= window) {
            return false;
        }
        entry.window = Some(Stamped { value: window, at });
        true
    }
}

/// 键：`<供应商>/<模型>`。
fn key(provider: &str, model: &str) -> String {
    format!("{provider}/{model}")
}

/// 一家供应商的模型列表。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderList {
    /// 什么时候拉的。
    pub fetched: Timestamp,
    /// 列出来的模型，照它给的先后。
    pub models: Vec<ListedModel>,
}

/// 列表里的一个模型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedModel {
    /// 模型名。
    pub id: String,
    /// 报了的窗口。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<u64>,
}

impl ProviderList {
    /// 读 `providers/<编号>.json`。
    ///
    /// # Errors
    ///
    /// 不是这个样子的 JSON：原话说是哪一份。
    pub fn parse(text: &str) -> Result<ProviderList, String> {
        serde_json::from_str(text).map_err(|error| format!("provider list not readable: {error}"))
    }

    /// 写成 JSON。
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// 列表里的这个模型。
    pub fn find(&self, model: &str) -> Option<&ListedModel> {
        self.models.iter().find(|listed| listed.id == model)
    }
}

#[cfg(test)]
mod tests;
