//! 温度（`docs/blueprint/models.md`「怎么走」第十四条，施工 8-22）：每个模型的一项配置，系统配置兜底、个人设置压在上面，
//! 和思考强度（[`crate::effort`]）一个规矩。
//!
//! - 这个模型用不用得了（[`usable`]）：目录说它不收温度的（`temperature: false`）不能；超过它走的驱动的上限
//!   （[`ceiling`]：`anthropic` 是 1，别的是 2）的不能。目录没写这一格的、没对上目录的当能调。
//! - 一次请求带哪一个：照配置的最终值（[`crate::facts::Facts::temperature`]，已经照 [`usable`] 查过），没有就不带。
//! - 配置里写的用不了的（[`unusable`]）：报 `unusable_temperature`，只报不丢，请求照没写发。这个模型能不能调、走哪种驱动由
//!   用它的一方交（要档案、目录）。

use miyu_config::key;
use miyu_config::parse::Parsed;
use miyu_config::problem::{Code, Problem};
use miyu_config::{Layer, Value};

use crate::provider::Driver;

/// 配置里模型默认的温度那一项（[`crate::settings::ModelSettings`] 的 `temperature`）。
pub const ITEM: &str = "providers.<id>.models.<model>.temperature";

/// 驱动收的温度的上限：Anthropic 的消息接口只收 0 到 1，别的两种收 0 到 2（清单本身的上限）。
pub fn ceiling(driver: Driver) -> f64 {
    match driver {
        Driver::Anthropic => 1.0,
        Driver::OpenAiChat | Driver::OpenAiResponses => 2.0,
    }
}

/// 查一个模型时要知道的：目录说的能不能调（没写的是 `None`），它真走的驱动。
pub type Takes = (Option<bool>, Driver);

/// 为什么用不了。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Unusable {
    /// 目录说这个模型不收温度。
    Unsupported,
    /// 超过驱动的上限，带着上限。
    TooHigh(f64),
}

/// 写的 `value` 这个模型用不用得了：`takes` 是目录说的能不能调（没写的是 `None`，当能调），`driver` 是它真走的驱动。
///
/// # Errors
///
/// 用不了的，交回为什么（[`Unusable`]）。
pub fn usable(value: f64, takes: Option<bool>, driver: Driver) -> Result<f64, Unusable> {
    if takes == Some(false) {
        return Err(Unusable::Unsupported);
    }
    let max = ceiling(driver);
    if value > max {
        return Err(Unusable::TooHigh(max));
    }
    Ok(value)
}

/// 一层配置 `parsed`（当成 `layer` 读的）里，模型默认的温度这个模型用不了的（「怎么走」第十四条第 2、3 条）：一处一条
/// `unusable_temperature`（错误），指到值那里；超过上限的 `name` 是上限。只看这一层算数的项。`model` 说供应商 `<id>`
/// 的模型 `<model>` 能不能调、走哪种驱动；说不出来的（那一家用不了）不查。
pub fn unusable(
    parsed: &Parsed,
    layer: Layer,
    model: &dyn Fn(&str, &str) -> Option<Takes>,
) -> Vec<Problem> {
    let mut found = Vec::new();
    for (written, entry) in &parsed.entries {
        if entry.item != ITEM || !entry.counts {
            continue;
        }
        let (Value::Float(value), Some(segments)) = (&entry.value, key::split(written)) else {
            continue;
        };
        let [_, provider, _, name, _] = segments.as_slice() else {
            continue;
        };
        let Some((takes, driver)) = model(provider, name) else {
            continue;
        };
        let Err(why) = usable(value.get(), takes, driver) else {
            continue;
        };
        let mut problem = Problem::item(
            Code::UnusableTemperature,
            layer,
            written,
            entry.at,
            &entry.raw,
        );
        if let Unusable::TooHigh(max) = why {
            problem.name = Some(max.to_string());
        }
        found.push(problem);
    }
    found
}

#[cfg(test)]
mod tests;
