//! models.dev 的目录（`docs/blueprint/models.md`「怎么走」第二条第 1、2 条，施工 8-7）：安装包带的、缓存里拉的都是原样的
//! `api.json`，读的时候只读用得上的格，不认识的格不理。
//!
//! - 供应商：`id`、`name`、`env`、`npm`、`api`、`doc`、`models`；
//! - 模型：`id`、`name`、`family`、`tool_call`、`modalities.input`、`limit`（`context`、`input`、`output`）、`cost`、
//!   `reasoning_options`、`status`。窗口取 `limit.context` 和 `limit.input` 里小的那个。
//!
//! 一个模型的格坏了（类型不对、数是负的），跳过它，交回它的名字由读的一方记一行；一家供应商自己的格坏了，整家跳过。
//! 整份不是 JSON 对象的，算读不了。读好以后照名字建两份索引：一模一样的名字、规整以后的名字（[`crate::matching`]）。

mod price;

pub use price::{Price, Rates, Tier, USD};

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::value::RawValue;

use crate::matching::normalize;

/// 读好的目录。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalog {
    providers: BTreeMap<String, CatalogProvider>,
    /// 一模一样的模型名 → 列了它的（供应商，模型），照字节排。
    named: BTreeMap<String, Vec<Entry>>,
    /// 规整以后的模型名 → 同上。
    normalized: BTreeMap<String, Vec<Entry>>,
}

/// 目录里的一个条目：供应商的编号、模型名。
pub type Entry = (String, String);

/// 目录里的一家供应商。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CatalogProvider {
    /// 编号。
    pub id: String,
    /// 显示名。
    pub name: Option<String>,
    /// 找 key 的环境变量。
    pub env: Vec<String>,
    /// AI SDK 的包名：照档案的 `[npm]` 认驱动。
    pub npm: Option<String>,
    /// 地址。
    pub api: Option<String>,
    /// 文档。
    pub doc: Option<String>,
    /// 模型：名字 → 资料。
    pub models: BTreeMap<String, CatalogModel>,
}

/// 目录里的一个模型：只有用得上的格。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CatalogModel {
    /// 显示名。
    pub name: Option<String>,
    /// 家族：认原厂用（[`crate::matching`] 第 6 条）。
    pub family: Option<String>,
    /// 能不能调工具。
    pub tools: Option<bool>,
    /// 能收的输入里认得的几种：`text`、`image`、`pdf`。没写的没有。
    pub inputs: Option<Vec<String>>,
    /// 窗口：`limit.context` 和 `limit.input` 里小的那个。
    pub window: Option<u64>,
    /// 最大输出：`limit.output`。
    pub max_output: Option<u64>,
    /// 价格，美元。
    pub price: Option<Price>,
    /// 思考强度：`effort` 的几级；只有开关的是 `on`。
    pub reasoning: Option<Vec<String>>,
    /// `deprecated`、`beta` 这类。
    pub status: Option<String>,
}

/// 在用的目录：读好的，和它是哪一份、什么时候拉的（`model.list` 的 `catalog`，来源的 `fetched`）。
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    /// 目录。
    pub catalog: Catalog,
    /// 哪一份。
    pub source: CatalogSource,
    /// 什么时候拉的：`meta` 里的原样。
    pub fetched: String,
}

/// 在用的是哪一份目录。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogSource {
    /// 安装包带的快照。
    Snapshot,
    /// 缓存目录里后台拉的。
    Cache,
}

impl CatalogSource {
    /// 协议、运行日志上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            CatalogSource::Snapshot => "snapshot",
            CatalogSource::Cache => "cache",
        }
    }
}

/// 读的结果：目录，和跳过的（`<供应商>` 或 `<供应商>/<模型>`）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Read {
    /// 目录。
    pub catalog: Catalog,
    /// 坏了跳过的。
    pub skipped: Vec<String>,
}

/// 认得的几种输入。
const INPUTS: [&str; 3] = ["text", "image", "pdf"];

/// 一家供应商的原文：模型先不读，一个个读，坏一个不连累别的。
#[derive(Deserialize)]
struct RawProvider<'a> {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    env: Vec<String>,
    #[serde(default)]
    npm: Option<String>,
    #[serde(default)]
    api: Option<String>,
    #[serde(default)]
    doc: Option<String>,
    #[serde(default, borrow)]
    models: BTreeMap<String, &'a RawValue>,
}

/// 一个模型的原文里用得上的格。
#[derive(Deserialize)]
struct RawModel {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    tool_call: Option<bool>,
    #[serde(default)]
    modalities: Option<Modalities>,
    #[serde(default)]
    limit: Option<Limit>,
    #[serde(default)]
    cost: Option<price::RawCost>,
    #[serde(default)]
    reasoning_options: Option<Vec<ReasoningOption>>,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Deserialize)]
struct Modalities {
    #[serde(default)]
    input: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct Limit {
    #[serde(default)]
    context: Option<u64>,
    #[serde(default)]
    input: Option<u64>,
    #[serde(default)]
    output: Option<u64>,
}

/// `reasoning_options` 里的一项：`{"type":"toggle"}` 或 `{"type":"effort","values":[…]}`，别的不理。几级里写了 `null` 的
/// 不算一级（真目录里 sarvam 的两个模型这么写，不为它跳过整个模型）。
#[derive(Deserialize)]
struct ReasoningOption {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    values: Vec<Option<String>>,
}

impl Catalog {
    /// 读 `api.json` 的原文。
    ///
    /// # Errors
    ///
    /// 不是 JSON，或者最外层不是一张「编号 → 供应商」的表：原话说是目录。
    pub fn parse(text: &str) -> Result<Read, String> {
        let raw: BTreeMap<String, &RawValue> =
            serde_json::from_str(text).map_err(|error| format!("catalog not readable: {error}"))?;
        let mut read = Read::default();
        for (id, provider) in raw {
            let Ok(provider) = serde_json::from_str::<RawProvider<'_>>(provider.get()) else {
                read.skipped.push(id);
                continue;
            };
            let mut models = BTreeMap::new();
            for (name, model) in provider.models {
                match serde_json::from_str::<RawModel>(model.get()) {
                    Ok(model) => {
                        models.insert(name, model.into());
                    }
                    Err(_) => read.skipped.push(format!("{id}/{name}")),
                }
            }
            let entry = CatalogProvider {
                id: id.clone(),
                name: provider.name,
                env: provider.env,
                npm: provider.npm,
                api: provider.api,
                doc: provider.doc,
                models,
            };
            read.catalog.providers.insert(id, entry);
        }
        read.catalog.index();
        Ok(read)
    }

    /// 照名字建索引。
    fn index(&mut self) {
        for (provider, entry) in &self.providers {
            for model in entry.models.keys() {
                let at = (provider.clone(), model.clone());
                self.named
                    .entry(model.clone())
                    .or_default()
                    .push(at.clone());
                self.normalized
                    .entry(normalize(model))
                    .or_default()
                    .push(at);
            }
        }
    }

    /// 编号是 `id` 的那一家。
    pub fn provider(&self, id: &str) -> Option<&CatalogProvider> {
        self.providers.get(id)
    }

    /// 全部供应商，照编号排。
    pub fn providers(&self) -> impl Iterator<Item = &CatalogProvider> {
        self.providers.values()
    }

    /// 那一家的那个模型。
    pub fn model(&self, provider: &str, model: &str) -> Option<&CatalogModel> {
        self.providers.get(provider)?.models.get(model)
    }

    /// 有几个模型。
    pub fn model_count(&self) -> usize {
        self.providers
            .values()
            .map(|entry| entry.models.len())
            .sum()
    }

    /// 名字和 `name` 一模一样的条目，照字节排。
    pub fn named(&self, name: &str) -> &[Entry] {
        self.named.get(name).map_or(&[], Vec::as_slice)
    }

    /// 规整以后是 `normalized` 的条目，照字节排。
    pub fn normalized(&self, normalized: &str) -> &[Entry] {
        self.normalized.get(normalized).map_or(&[], Vec::as_slice)
    }
}

impl From<RawModel> for CatalogModel {
    fn from(raw: RawModel) -> CatalogModel {
        let limit = raw.limit.unwrap_or(Limit {
            context: None,
            input: None,
            output: None,
        });
        let window = match (limit.context, limit.input) {
            (Some(context), Some(input)) => Some(context.min(input)),
            (context, input) => context.or(input),
        };
        let inputs = raw
            .modalities
            .and_then(|modalities| modalities.input)
            .map(|inputs| {
                INPUTS
                    .iter()
                    .filter(|known| inputs.iter().any(|input| input == *known))
                    .map(|known| (*known).to_string())
                    .collect()
            });
        CatalogModel {
            name: raw.name,
            family: raw.family,
            tools: raw.tool_call,
            inputs,
            window: window.filter(|window| *window > 0),
            max_output: limit.output.filter(|output| *output > 0),
            price: raw.cost.map(price::RawCost::price),
            reasoning: raw.reasoning_options.and_then(reasoning),
            status: raw.status,
        }
    }
}

/// 思考强度：有 `effort` 的照它的几级；只有开关的写 `on`；都没有的没有。
fn reasoning(options: Vec<ReasoningOption>) -> Option<Vec<String>> {
    let effort = options
        .iter()
        .filter(|option| option.kind == "effort")
        .map(|option| option.values.iter().flatten().cloned().collect::<Vec<_>>())
        .find(|values| !values.is_empty());
    match effort {
        Some(effort) => Some(effort),
        None => options
            .iter()
            .any(|option| option.kind == "toggle")
            .then(|| vec!["on".to_string()]),
    }
}

#[cfg(test)]
mod tests;
