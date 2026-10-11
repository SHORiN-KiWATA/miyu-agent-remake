//! 闸门（施工 V-3，23 F4「回归闸门钉住一个大会话，每次改动都跑」）：有预算的几行，时间超过预算的 `factor` 倍、内存超过
//! 预算就算没过。CI 的机器比开发的机器慢、抖得多，时间的倍数照 CI 实测定（`docs/blueprint/perf.md`「闸门」）；内存不随
//! 机器快慢变，照预算拦。

use std::collections::BTreeMap;

use super::Results;
use super::rows::checks;
use crate::budget::{Budget, Unit};

/// 没过闸门的几行，一行一句：说的是什么、量到多少、预算、闸门。都过了的是空的。
///
/// # Errors
///
/// 预算表里缺了要比的一项。
pub fn over(
    results: &Results,
    budgets: &BTreeMap<String, Budget>,
    factor: f64,
) -> Result<Vec<String>, String> {
    Ok(checks(results, budgets)?
        .into_iter()
        .filter_map(|check| {
            let measured = check.measured?;
            let (unit, times) = match check.budget.unit {
                Unit::Ms => ("ms", factor),
                Unit::Mb => ("MB", 1.0),
            };
            let limit = check.budget.value * times;
            (measured > limit).then(|| {
                format!(
                    "{}：量到 {measured:.1} {unit}，预算 {}，闸门是它的 {times} 倍（{limit:.1} {unit}）",
                    check.label,
                    check.budget.show()
                )
            })
        })
        .collect())
}

/// 写了倍数的照闸门判（施工 V-3）：有一行没过就交回报错，一行一句；没写的、都过了的交回好。表在这之前已经写好了。
///
/// # Errors
///
/// 有一行没过闸门；预算表里缺了要比的一项。
pub fn enforce(
    results: &Results,
    budgets: &BTreeMap<String, Budget>,
    factor: Option<f64>,
) -> Result<(), String> {
    let Some(factor) = factor else {
        return Ok(());
    };
    let over = over(results, budgets, factor)?;
    if over.is_empty() {
        eprintln!("闸门：时间都在预算的 {factor} 倍以内，内存都在预算以内");
        return Ok(());
    }
    Err(format!("闸门没过：\n{}", over.join("\n")))
}
