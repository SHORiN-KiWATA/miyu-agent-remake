//! 模型的资料（`docs/blueprint/models.md`「模型的资料」、「怎么走」第二条第 8 到 12 条，施工 8-7）：每一格一个值、一个来源，
//! 从上往下查，各查各的，对上就停：手写的、用出来的、供应商的列表、models.dev、驱动的保守默认。
//!
//! - 第 1、2 层对上的目录条目全借；第 3、4 层能力、窗口、最大输出、名字、状态都借，价格照 [`crate::matching`] 挑的那家借
//!   不借（`Matched::price`）。
//! - 价格是一整格：手写了一项就整份用手写的；本机的服务只认手写的，没写的是 0，来源 `local`。
//! - 倍率：模型手写的，再是供应商手写的，都没有是 1。
//! - 思考强度（施工 8-18，[`crate::effort`]）：几档照手写的、目录的，规整过；目录的开关只在这一家的档案写了开关时才算。默认的
//!   那一档只认配置写的、在这时的档位里的。
//!
//! 驱动、缓存类别这两格随用到它们的那一步（「施工时定的」8-7）。

mod source;

pub use source::Source;

use miyu_config::merge::{Origin, Resolved};
use miyu_config::{Value, key};
use miyu_drivers::Inputs;
use serde_json::{Map, Value as Json, json};

use crate::catalog::{CatalogModel, Price, Rates, USD};
use crate::effort;
use crate::knowledge::Knowledge;
use crate::matching::{Found, Matched, find};
use crate::provider::Provider;

/// 一格的值和它的来源。
#[derive(Debug, Clone, PartialEq)]
pub struct Fact<T> {
    /// 值。
    pub value: T,
    /// 从哪来的。
    pub source: Source,
}

/// 一个模型的资料：每一格各有来源。没有的值是 `None`，来源照样有（驱动的保守默认）。
#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
    /// 上下文窗口。没有：不主动压。
    pub window: Fact<Option<u64>>,
    /// 最大输出。没有：输出预留照策略的上限。
    pub max_output: Fact<Option<u64>>,
    /// 能收什么：`text`、`image`、`pdf` 里的几种。默认只有文字。
    pub inputs: Fact<Vec<String>>,
    /// 能不能调工具。没有：不知道，照样带工具面。
    pub tools: Fact<Option<bool>>,
    /// 思考强度有哪几档（施工 8-18 起规整过：`none`、`disabled` 读成 `off`，目录的开关照档案加 `off`、`on`）。
    pub reasoning: Fact<Option<Vec<String>>>,
    /// 默认的思考强度（施工 8-18）：配置写的、在这时的档位里的那一档；没写的、不在档位里的没有（请求照没写发）。
    pub effort: Fact<Option<String>>,
    /// 价格。没有：不算金额。
    pub price: Fact<Option<Price>>,
    /// 倍率。
    pub multiplier: Fact<f64>,
    /// 显示名。默认是模型名。
    pub name: Fact<String>,
    /// `deprecated`、`beta` 这类。
    pub status: Fact<Option<String>>,
}

impl Facts {
    /// 思考强度这时有哪几档（施工 8-18）：没有的是空的。
    pub fn levels(&self) -> &[String] {
        self.reasoning.value.as_deref().unwrap_or_default()
    }

    /// 能收哪些输入，写成驱动认的样子（`Call.inputs`）。
    pub fn driver_inputs(&self) -> Inputs {
        let has = |name: &str| self.inputs.value.iter().any(|input| input == name);
        Inputs {
            images: has("image"),
            pdf: has("pdf"),
        }
    }

    /// 写成 `model.list` 的 `facts`：每一格 `{"value":…,"from":…, 另带的}`。手写的来源要写成哪份文件，由 `file` 照层给。
    pub fn json(&self, file: &dyn Fn(miyu_config::Layer) -> String) -> Json {
        let mut map = Map::new();
        let mut put = |name: &str, value: Json, source: &Source| {
            let mut fact = source.json(file);
            fact.insert("value".to_string(), value);
            map.insert(name.to_string(), Json::Object(fact));
        };
        put("window", json!(self.window.value), &self.window.source);
        put(
            "max_output",
            json!(self.max_output.value),
            &self.max_output.source,
        );
        put("inputs", json!(self.inputs.value), &self.inputs.source);
        put("tools", json!(self.tools.value), &self.tools.source);
        put(
            "reasoning",
            json!(self.reasoning.value),
            &self.reasoning.source,
        );
        put("effort", json!(self.effort.value), &self.effort.source);
        let price = self.price.value.as_ref().map_or(Json::Null, Price::json);
        put("price", price, &self.price.source);
        put(
            "multiplier",
            json!(self.multiplier.value),
            &self.multiplier.source,
        );
        put("name", json!(self.name.value), &self.name.source);
        put("status", json!(self.status.value), &self.status.source);
        Json::Object(map)
    }
}

/// 这家 `provider` 的模型 `model` 的资料，照这一轮的配置 `resolved` 和手头的资料 `knowledge`。另交回对目录的结果：
/// `model.list` 照它标出手写指定的条目不存在。
pub fn facts(
    resolved: &Resolved,
    knowledge: &Knowledge<'_>,
    provider: &Provider,
    model: &str,
) -> (Facts, Found) {
    let written = Written {
        resolved,
        provider: &provider.id,
        model,
    };
    let found = match knowledge.catalog {
        Some(loaded) => find(
            &loaded.catalog,
            knowledge.vendors,
            provider
                .recognized
                .as_ref()
                .map(|recognized| recognized.provider.as_str()),
            model,
            written.text(&["catalog"]).map(|(text, _)| text).as_deref(),
        ),
        None => Found::Nothing,
    };
    let entry = match (&found, knowledge.catalog) {
        (Found::Matched(matched), Some(loaded)) => loaded
            .catalog
            .model(&matched.provider, &matched.model)
            .map(|entry| (entry, matched, loaded.fetched.as_str())),
        _ => None,
    };
    // 对上了目录的：每一格借的来源都是这一个条目（价格另看挑的那家借不借）。
    let model_data = entry.map(|(entry, _, _)| entry);
    let borrowed = entry.map(|(_, matched, fetched)| Source::catalog(matched, fetched));
    let from_catalog =
        |pick: &dyn Fn(&CatalogModel) -> Option<u64>| Some((pick(model_data?)?, borrowed.clone()?));
    let learned = knowledge.learned.window(&provider.id, model);
    let listed = knowledge.lists.get(&provider.id).and_then(|list| {
        let window = list.find(model)?.window?;
        Some((
            window,
            Source::Provider {
                fetched: list.fetched,
            },
        ))
    });
    let reasoning = fact(
        written
            .texts(&["reasoning"])
            .map(|(names, source)| (effort::levels(&names), source))
            .or_else(|| {
                let offered =
                    effort::offered(model_data?.reasoning.as_ref()?, provider.switchable())?;
                Some((offered, borrowed.clone()?))
            }),
    );
    let chosen = written.text(&["effort"]).and_then(|(level, source)| {
        let level = effort::normalize(&level).to_string();
        let known = reasoning.value.as_deref().unwrap_or_default();
        known.contains(&level).then_some((level, source))
    });
    let window = written
        .int(&["window"])
        .or_else(|| learned.map(|stamped| (stamped.value, Source::Learned { at: stamped.at })))
        .or(listed)
        .or_else(|| from_catalog(&|entry| entry.window));
    let facts = Facts {
        window: fact(window),
        max_output: fact(
            written
                .int(&["max_output"])
                .or_else(|| from_catalog(&|entry| entry.max_output)),
        ),
        inputs: or_default(
            written
                .texts(&["inputs"])
                .or_else(|| Some((model_data?.inputs.clone()?, borrowed.clone()?))),
            vec!["text".to_string()],
        ),
        tools: fact(
            written
                .bool(&["tools"])
                .or_else(|| Some((model_data?.tools?, borrowed.clone()?))),
        ),
        reasoning,
        effort: fact(chosen),
        price: price(&written, provider.local, entry),
        multiplier: or_default(
            written
                .float(&["price_multiplier"])
                .or_else(|| written.provider_float("price_multiplier")),
            1.0,
        ),
        name: or_default(
            model_data.and_then(|entry| Some((entry.name.clone()?, borrowed.clone()?))),
            model.to_string(),
        ),
        status: fact(model_data.and_then(|entry| Some((entry.status.clone()?, borrowed.clone()?)))),
    };
    (facts, found)
}

/// 查到的写成一格，没查到的是驱动的保守默认 `default`。
fn or_default<T>(found: Option<(T, Source)>, default: T) -> Fact<T> {
    match found {
        Some((value, source)) => Fact { value, source },
        None => Fact {
            value: default,
            source: Source::Default,
        },
    }
}

/// 查到的写成一格，没查到的是驱动的保守默认：没有。
fn fact<T>(found: Option<(T, Source)>) -> Fact<Option<T>> {
    match found {
        Some((value, source)) => Fact {
            value: Some(value),
            source,
        },
        None => Fact {
            value: None,
            source: Source::Default,
        },
    }
}

/// 价格：手写的整份；本机的服务没写是 0；目录的照挑的那家借不借；都没有的没有。
fn price(
    written: &Written<'_>,
    local: bool,
    entry: Option<(&CatalogModel, &Matched, &str)>,
) -> Fact<Option<Price>> {
    let item = |name: &str| written.float(&["price", name]);
    let rates = ["input", "output", "cache_read", "cache_write"].map(item);
    let currency = written.text(&["price", "currency"]);
    let first = rates
        .iter()
        .flatten()
        .map(|(_, source)| source.clone())
        .chain(currency.iter().map(|(_, source)| source.clone()))
        .next();
    if let Some(source) = first {
        let [input, output, cache_read, cache_write] =
            rates.map(|rate| rate.map(|(value, _)| value));
        let rates = Rates {
            input,
            output,
            cache_read,
            cache_write,
        };
        let currency = currency.map_or_else(|| USD.to_string(), |(text, _)| text);
        return Fact {
            value: Some(Price::of(rates, &currency)),
            source,
        };
    }
    if local {
        return Fact {
            value: Some(Price::free()),
            source: Source::Local,
        };
    }
    fact(entry.and_then(|(entry, matched, fetched)| {
        let price = entry.price.clone().filter(|_| matched.price)?;
        Some((price, Source::catalog(matched, fetched)))
    }))
}

/// 手写的那几格：`providers.<供应商>.models."<模型>".…` 的最终值和它在哪一行。
struct Written<'a> {
    resolved: &'a Resolved,
    provider: &'a str,
    model: &'a str,
}

impl Written<'_> {
    /// 模型这一块下面 `tail` 那一项的值和来源。
    fn get(&self, tail: &[&str]) -> Option<(&Value, Source)> {
        let mut segments = vec!["providers", self.provider, "models", self.model];
        segments.extend_from_slice(tail);
        self.at(&segments)
    }

    /// 供应商这一层的 `name` 那一项（倍率：模型上没写的看它）。
    fn provider_float(&self, name: &str) -> Option<(f64, Source)> {
        match self.at(&["providers", self.provider, name])? {
            (Value::Float(number), source) => Some((number.get(), source)),
            _ => None,
        }
    }

    /// 照段落接成真的键，取值和来源。来源只认写在文件里的（这几项没有默认值，也不由环境变量压过）。
    fn at(&self, segments: &[&str]) -> Option<(&Value, Source)> {
        let (value, origin) = self.resolved.get(&key::join(segments))?;
        match origin {
            Origin::File { layer, line } => Some((
                value,
                Source::Config {
                    layer: *layer,
                    line: *line,
                },
            )),
            Origin::Default | Origin::Env(_) => None,
        }
    }

    fn int(&self, tail: &[&str]) -> Option<(u64, Source)> {
        match self.get(tail)? {
            (Value::Int(number), source) => Some((u64::try_from(*number).ok()?, source)),
            _ => None,
        }
    }

    fn float(&self, tail: &[&str]) -> Option<(f64, Source)> {
        match self.get(tail)? {
            (Value::Float(number), source) => Some((number.get(), source)),
            _ => None,
        }
    }

    fn bool(&self, tail: &[&str]) -> Option<(bool, Source)> {
        match self.get(tail)? {
            (Value::Bool(on), source) => Some((*on, source)),
            _ => None,
        }
    }

    fn text(&self, tail: &[&str]) -> Option<(String, Source)> {
        match self.get(tail)? {
            (Value::Text(text), source) => Some((text.to_string(), source)),
            _ => None,
        }
    }

    fn texts(&self, tail: &[&str]) -> Option<(Vec<String>, Source)> {
        match self.get(tail)? {
            (Value::List(values), source) => Some((
                values
                    .iter()
                    .filter_map(|value| match value {
                        Value::Text(text) => Some(text.to_string()),
                        _ => None,
                    })
                    .collect(),
                source,
            )),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
