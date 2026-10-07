//! 按天的用量：照日期排好，算单日最高、连续几天、热度图（一周一列、周一在最上）和五档颜色。

use jiff::ToSpan;
use jiff::civil::Date;

use crate::core::UsageRow;

/// 一天用了多少：token，和 `currency` 那一种币的金额。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Day {
    /// 哪天。
    pub date: Date,
    /// 四项加起来的 token。
    pub tokens: u64,
    /// 那一种币的金额；这天一笔都没有的是 0。
    pub amount: f64,
}

/// 读按天的一行行：日期认不出的不要，照日期排。
pub fn days(rows: &[UsageRow], currency: &str) -> Vec<Day> {
    let mut out: Vec<Day> = rows
        .iter()
        .filter_map(|row| {
            let date: Date = row.day.as_deref()?.parse().ok()?;
            let amount = row
                .bill
                .amounts
                .iter()
                .filter(|c| c.currency == currency)
                .map(|c| c.amount)
                .sum();
            Some(Day {
                date,
                tokens: row.tokens,
                amount,
            })
        })
        .collect();
    out.sort_by_key(|d| d.date);
    out
}

/// 用得最多的那天（照 token）；一天都没有的是 `None`。
pub fn peak(days: &[Day]) -> Option<&Day> {
    days.iter()
        .filter(|d| d.tokens > 0)
        .max_by_key(|d| d.tokens)
}

/// 连续使用几天：现在连着几天（今天没用的从昨天往回数）、最长连过几天。有 token 的才算用了。
pub fn streaks(days: &[Day], today: Date) -> (u32, u32) {
    let used: Vec<Date> = days
        .iter()
        .filter(|d| d.tokens > 0)
        .map(|d| d.date)
        .collect();
    let mut longest = 0;
    let mut run = 0;
    let mut last: Option<Date> = None;
    for &date in &used {
        run = match last {
            Some(prev) if prev.tomorrow().ok() == Some(date) => run + 1,
            _ => 1,
        };
        longest = longest.max(run);
        last = Some(date);
    }
    // 现在的：从今天（今天没用的从昨天）往回数到断开。
    let mut current = 0;
    let mut day = if used.contains(&today) {
        Some(today)
    } else {
        today.yesterday().ok()
    };
    while let Some(d) = day.filter(|d| used.contains(d)) {
        current += 1;
        day = d.yesterday().ok();
    }
    (current, longest)
}

/// 热度图：`weeks` 列，最右一列是今天那一周；每列七格，周一在最上。今天以后的格子是 `None`，没用的是 `Some(0.0)`。
/// `months` 是每个月开头落在哪一列（这一列的周一是这个月的头一周）和月份（1 到 12）。
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    /// 一列一周。
    pub weeks: Vec<[Option<f64>; 7]>,
    /// 月份标在哪一列。
    pub months: Vec<(usize, i8)>,
}

/// 照 `value`（token 或者金额）排热度图。
pub fn grid(days: &[Day], today: Date, weeks: usize, value: impl Fn(&Day) -> f64) -> Grid {
    let weeks = weeks.max(1);
    let monday = today - i64::from(today.weekday().to_monday_zero_offset()).days();
    let first = monday - i64::try_from((weeks - 1) * 7).unwrap_or(0).days();
    let mut out = Grid {
        weeks: Vec::with_capacity(weeks),
        months: Vec::new(),
    };
    for w in 0..weeks {
        let start = first + i64::try_from(w * 7).unwrap_or(0).days();
        let mut column = [None; 7];
        for (i, cell) in column.iter_mut().enumerate() {
            let date = start + i64::try_from(i).unwrap_or(0).days();
            if date <= today {
                let v = days.iter().find(|d| d.date == date).map_or(0.0, &value);
                *cell = Some(v);
            }
        }
        // 这一列的周一前七天在上个月：这一列是这个月的头一周。
        let before = start - 7.days();
        if w == 0 || before.month() != start.month() {
            out.months.push((w, start.month()));
        }
        out.weeks.push(column);
    }
    out
}

/// 分档的三道门槛：这段时间里有用的那些天的四分位（25%、50%、75%）；一天都没用的是空的。
pub fn thresholds(values: impl Iterator<Item = f64>) -> Vec<f64> {
    let mut used: Vec<f64> = values.filter(|v| *v > 0.0).collect();
    if used.is_empty() {
        return Vec::new();
    }
    used.sort_by(f64::total_cmp);
    let at = |q: usize| used[(used.len() - 1) * q / 4];
    vec![at(1), at(2), at(3)]
}

/// 一格是第几档：0 是没用，1 到 4 照门槛。
pub fn level(value: f64, thresholds: &[f64]) -> usize {
    if value <= 0.0 {
        return 0;
    }
    1 + thresholds.iter().filter(|t| value > **t).count()
}

#[cfg(test)]
mod tests;
