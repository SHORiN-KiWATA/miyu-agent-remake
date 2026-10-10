//! 预算照 `docs/designs/23-性能预算.md` 第二节那张表读（F1：预算是策略数据，实测后校准）：改了那张表，量尺跟着比，
//! 数字不抄进代码。
//!
//! 一行的第二格里找第一个「数 单位」，前面带 `p50`、`p95`、`p99` 的照那个分位比；只写了一个数的照 p50 比。单位认
//! `ms`、`MB`；一个数都没有的（「用完就退」）不算预算。

use std::collections::BTreeMap;

/// 第二节的标题，表从它下面开始。
const SECTION: &str = "### 二、预算";

/// 一条预算。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Budget {
    /// 照哪个分位比：50、95、99。
    pub quantile: u8,
    /// 数。
    pub value: f64,
    /// 单位。
    pub unit: Unit,
}

/// 预算的单位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// 毫秒。
    Ms,
    /// 兆字节（MiB，和 `smaps_rollup` 的 kB 一样按 1024 算）。
    Mb,
}

impl Budget {
    /// 写进表里的样子：`p95 5 ms`、`30 MB`。
    pub fn show(&self) -> String {
        let unit = match self.unit {
            Unit::Ms => "ms",
            Unit::Mb => "MB",
        };
        match self.quantile {
            50 => format!("{} {unit}", self.value),
            q => format!("p{q} {} {unit}", self.value),
        }
    }
}

/// 一个量到的数比预算：没量的「不量」，没超的「过」，超了的「超」。
pub fn verdict(measured: Option<f64>, budget: &Budget) -> &'static str {
    match measured {
        None => "不量",
        Some(value) if value <= budget.value => "过",
        Some(_) => "超",
    }
}

/// 读第二节的表：第一格（项目）对到预算。
///
/// # Errors
///
/// 找不到第二节、第二节里没有表。
pub fn parse(design: &str) -> Result<BTreeMap<String, Budget>, String> {
    let start = design
        .find(SECTION)
        .ok_or_else(|| format!("找不到「{SECTION}」"))?;
    let rows: Vec<&str> = design[start..]
        .lines()
        .skip(1)
        .skip_while(|line| !line.starts_with('|'))
        .take_while(|line| line.starts_with('|'))
        .collect();
    if rows.is_empty() {
        return Err(format!("「{SECTION}」下面没有表"));
    }
    Ok(rows
        .iter()
        .skip(2)
        .filter_map(|row| {
            let mut cells = row.split('|').map(str::trim).skip(1);
            let name = cells.next()?;
            Some((name.to_string(), budget(cells.next()?)?))
        })
        .collect())
}

/// 一格里的预算。
fn budget(cell: &str) -> Option<Budget> {
    let words: Vec<&str> = cell
        .split(|c: char| c.is_whitespace() || "（）(),，".contains(c))
        .filter(|word| !word.is_empty())
        .collect();
    words.windows(2).enumerate().find_map(|(at, pair)| {
        let value: f64 = pair[0].parse().ok()?;
        let unit = match pair[1] {
            "ms" => Unit::Ms,
            "MB" => Unit::Mb,
            _ => return None,
        };
        let quantile = at
            .checked_sub(1)
            .and_then(|before| words[before].strip_prefix('p')?.parse().ok())
            .unwrap_or(50);
        Some(Budget {
            quantile,
            value,
            unit,
        })
    })
}

#[cfg(test)]
mod tests;
