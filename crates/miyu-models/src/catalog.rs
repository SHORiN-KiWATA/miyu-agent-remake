//! models.dev 的目录（`docs/blueprint/models.md`「怎么走」第二条第 1、2 条，施工 8-7）：安装包带的、缓存里拉的都是原样的
//! `api.json`，读的时候只读用得上的格，不认识的格不理。
//!
//! - 供应商：`id`、`name`、`env`、`npm`、`api`、`doc`、`models`；
//! - 模型：`id`、`name`、`family`、`tool_call`、`modalities.input`、`limit`（`context`、`input`、`output`）、`cost`、
//!   `reasoning_options`、`status`、`release_date`（施工 8-11，推荐模型用）、`provider.npm`、`interleaved`（施工 8-14：这个模型
//!   走哪种驱动、交错思考写在哪个字段）、`temperature`（施工 8-22：能不能调温度）。窗口取 `limit.context` 和 `limit.input` 里小的那个。
//!
//! 一个模型的格坏了（类型不对、数是负的），跳过它，交回它的名字由读的一方记一行；一家供应商自己的格坏了，整家跳过。
//! 整份不是 JSON 对象的，算读不了。读好以后照名字建两份索引：一模一样的名字、规整以后的名字（[`crate::matching`]）。
//!
//! 存法要紧凑（施工 V-2 下）：出厂的快照有八千多个模型，原来每个名字、家族、日期各存一份、索引里再各抄一份，读进来占约
//! 11.5 MB、十三万多次分配，核心空闲时的内存一大半是它。现在同样的字只存一份（[`Text`]：模型名既是表的键也是索引里的那一
//! 份，家族、日期这些照字共用），认得的几种输入是几个开关（[`Inputs`]）。

mod inputs;
mod price;

pub use inputs::Inputs;
pub use price::{Price, Rates, Tier, USD};

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::value::RawValue;

use crate::matching::normalize;

/// 目录里共用的一段字：同样的字只存一份，抄一份只是多一个引用（施工 V-2 下）。
pub type Text = Arc<str>;

/// 读好的目录。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalog {
    providers: BTreeMap<Text, CatalogProvider>,
    /// 一模一样的模型名 → 列了它的（供应商，模型），照字节排。
    named: BTreeMap<Text, Vec<Entry>>,
    /// 规整以后的模型名 → 同上。
    normalized: BTreeMap<Text, Vec<Entry>>,
}

/// 目录里的一个条目：供应商的编号、模型名，和表里的键是同一份。
pub type Entry = (Text, Text);

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
    pub models: BTreeMap<Text, CatalogModel>,
}

/// 目录里的一个模型：只有用得上的格。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CatalogModel {
    /// 显示名。
    pub name: Option<Text>,
    /// 家族：认原厂用（[`crate::matching`] 第 6 条）。
    pub family: Option<Text>,
    /// 能不能调工具。
    pub tools: Option<bool>,
    /// 能收的输入里认得的几种：`text`、`image`、`pdf`、`audio`、`video`。没写的没有。
    pub inputs: Option<Inputs>,
    /// 窗口：`limit.context` 和 `limit.input` 里小的那个。
    pub window: Option<u64>,
    /// 最大输出：`limit.output`。
    pub max_output: Option<u64>,
    /// 价格，美元。
    pub price: Option<Price>,
    /// 思考强度：`effort` 的几档（规整过）、有没有开关（施工 8-18）。都没有的没有。
    pub reasoning: Option<Reasoning>,
    /// 能不能调温度（`temperature`，施工 8-22）。没写的没有：当能调。
    pub temperature: Option<bool>,
    /// `deprecated`、`beta` 这类。
    pub status: Option<Text>,
    /// 发布日期，原样（`2026-09-10`）：照字比新旧（施工 8-11，第一次接入推荐模型用）。
    pub release_date: Option<Text>,
    /// 这个模型自己的 AI SDK 包名（`provider.npm`，施工 8-14）：和这一家的不一样的才写，照档案的 `[npm]` 认驱动。
    pub npm: Option<Text>,
    /// 交错思考写回哪个字段（`interleaved` 写成 `{"field": …}` 的那个字段名，施工 8-14）：`true` 这类说不出字段的没有。认不
    /// 认这个字段名归合资料的一方（[`crate::provider::Provider::for_model`]）。
    pub interleaved: Option<Text>,
}

/// 目录里一个模型的思考强度（施工 8-18，`models.md`「模型的资料」）：开关算不算、能不能关，合资料时照这一家的档案定
/// （[`crate::effort::offered`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reasoning {
    /// `effort` 的几档：`none`、`disabled` 读成 `off`，重复的只留第一个（[`crate::effort::levels`]）。没有的是空的。
    pub levels: Vec<String>,
    /// 有 `toggle`：能开关思考。
    pub toggle: bool,
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

/// 一个模型的原文里用得上的格：字能借原文的就借，不另分配（施工 V-2 下）。
#[derive(Deserialize)]
struct RawModel<'a> {
    #[serde(default, borrow)]
    name: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    family: Option<Cow<'a, str>>,
    #[serde(default)]
    tool_call: Option<bool>,
    #[serde(default, borrow)]
    modalities: Option<Modalities<'a>>,
    #[serde(default)]
    limit: Option<Limit>,
    #[serde(default)]
    cost: Option<price::RawCost>,
    #[serde(default)]
    reasoning_options: Option<Vec<ReasoningOption>>,
    #[serde(default)]
    temperature: Option<bool>,
    #[serde(default, borrow)]
    status: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    release_date: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    provider: Option<ModelProvider<'a>>,
    /// 写法不一（`true`、`{"field": …}`），原样收下再看，不为它跳过整个模型。
    #[serde(default)]
    interleaved: Option<serde_json::Value>,
}

/// 模型上的 `provider`：只用 `npm`。
#[derive(Deserialize)]
struct ModelProvider<'a> {
    #[serde(default, borrow)]
    npm: Option<Cow<'a, str>>,
}

#[derive(Deserialize)]
struct Modalities<'a> {
    #[serde(default, borrow)]
    input: Option<Vec<Cow<'a, str>>>,
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
        let mut texts = Texts::default();
        for (id, provider) in raw {
            let Ok(provider) = serde_json::from_str::<RawProvider<'_>>(provider.get()) else {
                read.skipped.push(id);
                continue;
            };
            let mut models = BTreeMap::new();
            for (name, model) in provider.models {
                match serde_json::from_str::<RawModel<'_>>(model.get()) {
                    Ok(model) => {
                        let model = CatalogModel::of(model, &mut texts);
                        models.insert(texts.get(&name), model);
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
            read.catalog.providers.insert(texts.get(&id), entry);
        }
        read.catalog.index(&mut texts);
        Ok(read)
    }

    /// 照名字建索引：条目、键都是表里那一份的引用，规整以后的名字照字共用。
    fn index(&mut self, texts: &mut Texts) {
        for (provider, entry) in &self.providers {
            for model in entry.models.keys() {
                let at = (Arc::clone(provider), Arc::clone(model));
                self.named
                    .entry(Arc::clone(model))
                    .or_default()
                    .push(at.clone());
                self.normalized
                    .entry(texts.get(&normalize(model)))
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

impl CatalogModel {
    /// 照原文里用得上的格造，字照 `texts` 共用。
    fn of(raw: RawModel<'_>, texts: &mut Texts) -> CatalogModel {
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
            .map(|inputs| Inputs::of(inputs.iter().map(|input| &**input)));
        let mut text = |value: Option<Cow<'_, str>>| value.map(|value| texts.get(&value));
        CatalogModel {
            name: text(raw.name),
            family: text(raw.family),
            tools: raw.tool_call,
            inputs,
            window: window.filter(|window| *window > 0),
            max_output: limit.output.filter(|output| *output > 0),
            price: raw.cost.map(price::RawCost::price),
            reasoning: raw.reasoning_options.and_then(reasoning),
            temperature: raw.temperature,
            status: text(raw.status),
            release_date: text(raw.release_date),
            npm: text(raw.provider.and_then(|provider| provider.npm)),
            interleaved: text(
                raw.interleaved
                    .as_ref()
                    .and_then(|interleaved| interleaved.get("field")?.as_str())
                    .map(Cow::Borrowed),
            ),
        }
    }
}

/// 读一份目录时同样的字只存一份（施工 V-2 下）。
#[derive(Default)]
struct Texts(BTreeSet<Text>);

impl Texts {
    /// `text` 的那一份：见过的交回原来那一份，没见过的存下来。
    fn get(&mut self, text: &str) -> Text {
        if let Some(seen) = self.0.get(text) {
            return Arc::clone(seen);
        }
        let fresh: Text = Arc::from(text);
        self.0.insert(Arc::clone(&fresh));
        fresh
    }
}

/// 思考强度：第一个不空的 `effort` 的几档（规整过），有没有 `toggle`；两样都没有的（只有 `budget_tokens` 的也是）没有
/// （施工 8-18）。
fn reasoning(options: Vec<ReasoningOption>) -> Option<Reasoning> {
    let levels = options
        .iter()
        .filter(|option| option.kind == "effort")
        .map(|option| option.values.iter().flatten().cloned().collect::<Vec<_>>())
        .find(|values| !values.is_empty())
        .map(|values| crate::effort::levels(&values))
        .unwrap_or_default();
    let toggle = options.iter().any(|option| option.kind == "toggle");
    (toggle || !levels.is_empty()).then_some(Reasoning { levels, toggle })
}

#[cfg(test)]
mod tests;
