//! 金额（`docs/blueprint/models.md`「怎么走」第九条第 2 条，`15-模型与供应商.md` M9，施工 8-15）：照这一次真发给的模型的价格、
//! 倍率和这一次的用量，算出这一次花了多少，写成 `model.called` 的 `cost`。
//!
//! - 按上下文分档：输入（没命中、命中、写进缓存三项加起来）严格大于哪一档的门槛，用门槛最高的那一档；目录的
//!   `context_over_200k` 当作门槛 200000 的一档，排在 `tiers` 后面，门槛一样的取先写的。档里没写的项用底价的。
//! - 金额 = (没命中 × 输入价 + 命中 × 读缓存价 + 写进缓存 × 写缓存价 + 输出 × 输出价) ÷ 1000000 × 倍率，照这个先后，双精度。
//! - 用量不是 0 的哪一项没有价格，不算；单写了思考价、又和这一档的输出价不一样的，也不算（思考算在输出里，拆不开）。
//! - 币种照那一份价格的，不换算；本机的服务价格四项都是 0，出处 `local`。
//!
//! 几种币种的金额排先后（[`ordered`]）：`usage.currency`（[`crate::settings::UsageSettings`]）排最前，别的照币种代码的字母先后。

use miyu_config::Layer;
use miyu_kernel::event::{Cost, Prices, Real, Usage};

use crate::catalog::{Price, Rates};
use crate::facts::{Facts, Source};

/// 目录的 `context_over_200k` 当作这个门槛的一档。
const OVER_200K: u64 = 200_000;

/// 价格按每一百万 token 写。
const PER: f64 = 1_000_000.0;

/// 算金额要的：真发给的那个模型的价格、倍率，价格从哪来（写进 `cost.source`）。
#[derive(Debug, Clone, PartialEq)]
pub struct Tariff {
    /// 价格，一整份。
    pub price: Price,
    /// 倍率。
    pub multiplier: f64,
    /// 价格从哪来：`catalog:<供应商>/<模型>`、`config:<文件>:<行>`、`local`。
    pub source: String,
}

impl Tariff {
    /// 照模型的资料造：没有价格的没有。手写的出处写成哪份文件，由 `file` 照层给（数据根里的相对路径）。
    pub fn of(facts: &Facts, file: &dyn Fn(Layer) -> String) -> Option<Tariff> {
        let price = facts.price.value.clone()?;
        let source = match &facts.price.source {
            Source::Catalog { entry, .. } => format!("catalog:{entry}"),
            Source::Config { layer, line } => format!("config:{}:{line}", file(*layer)),
            Source::Local => "local".to_string(),
            // 价格只从手写的、目录、本机的来；别的来源说明是空的，不算。
            Source::Learned { .. } | Source::Provider { .. } | Source::Default => return None,
        };
        Some(Tariff {
            price,
            multiplier: facts.multiplier.value,
            source,
        })
    }

    /// 这一次的用量 `usage` 花了多少；算不出的没有（见模块的说明）。
    pub fn cost(&self, usage: &Usage) -> Option<Cost> {
        let input = usage.uncached + usage.cache_read + usage.cache_write;
        let (above, rates) = match tier(&self.price, input) {
            Some((size, rates)) => (Some(size), over(&self.price.rates, rates)),
            None => (None, self.price.rates),
        };
        if self
            .price
            .reasoning
            .is_some_and(|reasoning| Some(reasoning) != rates.output)
        {
            return None;
        }
        let mut sum = 0.0;
        for (tokens, rate) in [
            (usage.uncached, rates.input),
            (usage.cache_read, rates.cache_read),
            (usage.cache_write, rates.cache_write),
            (usage.output, rates.output),
        ] {
            if tokens == 0 {
                continue;
            }
            sum += tokens as f64 * rate?;
        }
        let real = |rate: Option<f64>| rate.map(Real::new);
        Some(Cost {
            amount: Real::new(sum / PER * self.multiplier),
            currency: self.price.currency.to_string(),
            price: Prices {
                input: real(rates.input),
                output: real(rates.output),
                cache_read: real(rates.cache_read),
                cache_write: real(rates.cache_write),
            },
            multiplier: Real::new(self.multiplier),
            source: self.source.clone(),
            above,
        })
    }
}

/// 输入 `input` 严格大于门槛的几档里门槛最高的那一档：`tiers` 照写的先后，`context_over_200k` 排最后；门槛一样的取先写的。
fn tier(price: &Price, input: u64) -> Option<(u64, &Rates)> {
    let over = price.over_200k.as_ref().map(|rates| (OVER_200K, rates));
    let tiers = price.tiers.iter().map(|tier| (tier.size, &tier.rates));
    let mut chosen: Option<(u64, &Rates)> = None;
    for (size, rates) in tiers.chain(over) {
        if input > size && chosen.is_none_or(|(highest, _)| size > highest) {
            chosen = Some((size, rates));
        }
    }
    chosen
}

/// 一档的价：档里写了的用档的，没写的用底价 `base` 的。
fn over(base: &Rates, tier: &Rates) -> Rates {
    Rates {
        input: tier.input.or(base.input),
        output: tier.output.or(base.output),
        cache_read: tier.cache_read.or(base.cache_read),
        cache_write: tier.cache_write.or(base.cache_write),
    }
}

/// 几种币种的金额排好先后（`models.md`「对外的样子」`usage.currency`）：`first` 排最前，别的照币种代码的字母先后。
pub fn ordered(amounts: &mut [(String, f64)], first: &str) {
    amounts.sort_by(|(a, _), (b, _)| (a != first, a).cmp(&(b != first, b)));
}

#[cfg(test)]
mod tests;
