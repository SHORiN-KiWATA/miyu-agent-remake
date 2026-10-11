//! 价格（`docs/blueprint/models.md`「模型的资料」`price` 那一行，施工 8-7）：每一百万 token 的价，四项、币种，目录里的另带
//! 按上下文分的档（`tiers`）、超过 20 万的价（`context_over_200k`）、思考的价。价格是一整格：从哪一层来，四项和币种就都
//! 照那一层的。怎么照它算金额在 [`crate::price`]（施工 8-15）。

use std::borrow::Cow;

use serde::Deserialize;
use serde_json::{Map, Value, json};

/// 四项的价：每一百万 token，没写的没有。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rates {
    /// 输入（不算缓存的）。
    pub input: Option<f64>,
    /// 输出。
    pub output: Option<f64>,
    /// 读缓存。
    pub cache_read: Option<f64>,
    /// 写缓存。
    pub cache_write: Option<f64>,
}

/// 按上下文分的一档：输入超过 `size` 的照这一档的价（施工 8-15，[`crate::price`]）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tier {
    /// 门槛：超过多少 token。
    pub size: u64,
    /// 这一档的价。
    pub rates: Rates,
}

/// 一个模型的价格。
#[derive(Debug, Clone, PartialEq)]
pub struct Price {
    /// 四项。
    pub rates: Rates,
    /// 单写的思考价。
    pub reasoning: Option<f64>,
    /// 币种：ISO 4217 的三个大写字母。目录的是 `USD`，不另分配（施工 V-2 下：八千多个模型各存一份 `USD`）。
    pub currency: Cow<'static, str>,
    /// 按上下文分的档，照写的先后。
    pub tiers: Vec<Tier>,
    /// 超过 20 万 token 的价。
    pub over_200k: Option<Rates>,
}

/// 目录里的币种。
pub const USD: &str = "USD";

impl Price {
    /// 只有四项的价格。
    pub fn of(rates: Rates, currency: &str) -> Price {
        Price {
            rates,
            reasoning: None,
            currency: match currency {
                USD => Cow::Borrowed(USD),
                other => Cow::Owned(other.to_string()),
            },
            tiers: Vec::new(),
            over_200k: None,
        }
    }

    /// 本机的服务：四项都是 0（第二条第 12 条）。
    pub fn free() -> Price {
        let zero = Some(0.0);
        Price::of(
            Rates {
                input: zero,
                output: zero,
                cache_read: zero,
                cache_write: zero,
            },
            USD,
        )
    }

    /// 写成 `model.list` 的 JSON：四项里写了的、币种，有的话再带思考价、分档、超过 20 万的价。
    pub fn json(&self) -> Value {
        let mut map = rates_json(&self.rates);
        if let Some(reasoning) = self.reasoning {
            map.insert("reasoning".to_string(), json!(reasoning));
        }
        map.insert("currency".to_string(), json!(self.currency));
        if !self.tiers.is_empty() {
            let tiers: Vec<Value> = self
                .tiers
                .iter()
                .map(|tier| {
                    let mut map = rates_json(&tier.rates);
                    map.insert("size".to_string(), json!(tier.size));
                    Value::Object(map)
                })
                .collect();
            map.insert("tiers".to_string(), Value::Array(tiers));
        }
        if let Some(over) = &self.over_200k {
            map.insert(
                "context_over_200k".to_string(),
                Value::Object(rates_json(over)),
            );
        }
        Value::Object(map)
    }
}

/// 四项里写了的。
fn rates_json(rates: &Rates) -> Map<String, Value> {
    let mut map = Map::new();
    for (name, value) in [
        ("input", rates.input),
        ("output", rates.output),
        ("cache_read", rates.cache_read),
        ("cache_write", rates.cache_write),
    ] {
        if let Some(value) = value {
            map.insert(name.to_string(), json!(value));
        }
    }
    map
}

/// 目录里 `cost` 的原文。
#[derive(Deserialize)]
pub(super) struct RawCost {
    #[serde(flatten)]
    rates: RawRates,
    #[serde(default)]
    reasoning: Option<f64>,
    #[serde(default)]
    tiers: Vec<RawTier>,
    #[serde(default)]
    context_over_200k: Option<RawRates>,
}

#[derive(Deserialize)]
struct RawRates {
    #[serde(default)]
    input: Option<f64>,
    #[serde(default)]
    output: Option<f64>,
    #[serde(default)]
    cache_read: Option<f64>,
    #[serde(default)]
    cache_write: Option<f64>,
}

#[derive(Deserialize)]
struct RawTier {
    #[serde(flatten)]
    rates: RawRates,
    tier: TierKind,
}

/// 一档按什么分：只认 `context`（按上下文），别的档不理。
#[derive(Deserialize)]
struct TierKind {
    #[serde(rename = "type")]
    kind: String,
    size: u64,
}

impl RawRates {
    fn rates(self) -> Rates {
        Rates {
            input: self.input,
            output: self.output,
            cache_read: self.cache_read,
            cache_write: self.cache_write,
        }
    }
}

impl RawCost {
    /// 目录的价格，美元。
    pub(super) fn price(self) -> Price {
        Price {
            rates: self.rates.rates(),
            reasoning: self.reasoning,
            currency: Cow::Borrowed(USD),
            tiers: self
                .tiers
                .into_iter()
                .filter(|tier| tier.tier.kind == "context")
                .map(|tier| Tier {
                    size: tier.tier.size,
                    rates: tier.rates.rates(),
                })
                .collect(),
            over_200k: self.context_over_200k.map(RawRates::rates),
        }
    }
}
