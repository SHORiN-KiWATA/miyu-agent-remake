//! 一次请求的金额（施工 8-15，`docs/blueprint/models.md`「事件」`model.called` 的 `cost`、「怎么走」第九条第 3 条）：执行器
//! 照价格、倍率算好，随「说完了」交给内核，内核原样记进 `model.called`。以后目录更新、人改倍率，已经记下的不重算。内核不碰
//! 价格，只记。

use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 一次请求花了多少（`model.called` 的 `cost`）。算不出的不写这一格。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cost {
    /// 这一次花了多少，币种照 `currency`，不取整。
    pub amount: Real,
    /// 币种：那一份价格的币种，目录的是 `USD`，手写的照写的。
    pub currency: String,
    /// 实际用的那一档价格：每一百万 token，币种同上，只写有的几项。
    pub price: Prices,
    /// 乘的倍率。
    pub multiplier: Real,
    /// 价格从哪来：`catalog:<目录里的供应商>/<模型>`、`config:<文件>:<行>`，或者 `local`（本机的服务）。
    pub source: String,
    /// 用了按上下文分档的价格：是超过多少 token 的那一档。没分档的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub above: Option<u64>,
}

/// 价格的四项，每一百万 token；没有的不写。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prices {
    /// 输入（不算缓存的）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Real>,
    /// 输出。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<Real>,
    /// 读缓存。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<Real>,
    /// 写缓存。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<Real>,
}

/// 一个小数（施工 8-15）：照位比相等，所以带着它的事件、输入照样能比（`0.0` 和 `-0.0` 不一样）。JSON 里读不出 `nan`、无穷。
///
/// 写出去时整数值写成整数（`1` 不写 `1.0`，绝对值在 2^53 以内的），别的照 `serde_json` 最短能读回原值的写法：样本里的
/// 倍率写 `1`（`models.md`「样子」）。
#[derive(Debug, Clone, Copy)]
pub struct Real(f64);

/// 写成整数的上限：2^53，再大的双精度不一定是准的整数。
const EXACT: f64 = 9_007_199_254_740_992.0;

impl Real {
    /// 包一个小数。
    pub fn new(number: f64) -> Real {
        Real(number)
    }

    /// 里面的小数。
    pub fn get(self) -> f64 {
        self.0
    }
}

impl PartialEq for Real {
    fn eq(&self, other: &Real) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for Real {}

impl Serialize for Real {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let number = self.0;
        if number.fract() == 0.0 && number.abs() < EXACT {
            // 在 2^53 以内的整数值，转成整数不丢、不截。
            return s.serialize_i64(number as i64);
        }
        s.serialize_f64(number)
    }
}

impl<'de> Deserialize<'de> for Real {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_f64(RealVisitor)
    }
}

/// 认 JSON 里的数：整数、小数都行。
struct RealVisitor;

impl Visitor<'_> for RealVisitor {
    type Value = Real;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a number")
    }

    fn visit_f64<E: de::Error>(self, number: f64) -> Result<Real, E> {
        Ok(Real(number))
    }

    // 金额、价格、倍率读成双精度：大过 2^53 的整数本来也不是准数。
    fn visit_i64<E: de::Error>(self, number: i64) -> Result<Real, E> {
        Ok(Real(number as f64))
    }

    fn visit_u64<E: de::Error>(self, number: u64) -> Result<Real, E> {
        Ok(Real(number as f64))
    }
}

#[cfg(test)]
mod tests;
