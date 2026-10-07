//! 金额怎么写（蓝图 `tui.md`「配置与模型」第 5 条，照核心 `session_usage` 的写法）：三位有效数字、至少两位小数，
//! 几种币并排「$0.42 + ¥1.30」，不换算；`usage.currency` 那一种排最前，别的照币种代码的字母先后。

use std::collections::HashMap;

use crate::core::{Bill, Cost};

/// 一个数：三位有效数字、至少两位小数，两位以后多出来的 0 去掉（`0.42`、`1.30`、`0.000292`、`12.35`）。
pub fn amount(value: f64) -> String {
    let magnitude = if value > 0.0 {
        value.log10().floor() as i32
    } else {
        0
    };
    let decimals = usize::try_from((2 - magnitude).max(2)).unwrap_or(2);
    let text = format!("{value:.decimals$}");
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    let mut fraction = fraction.to_string();
    while fraction.len() > 2 && fraction.ends_with('0') {
        fraction.pop();
    }
    format!("{whole}.{fraction}")
}

/// 一笔：表里有符号的写在前面（`$0.42`），没有的写在后面（`1.30 HKD`）。
pub fn one(cost: &Cost, symbols: &HashMap<String, String>) -> String {
    let value = amount(cost.amount);
    match symbols.get(&cost.currency) {
        Some(symbol) => format!("{symbol}{value}"),
        None => format!("{value} {}", cost.currency),
    }
}

/// 一份账的金额：`first` 那种币排最前，别的照字母先后，用 ` + ` 连；一笔都没有的是 `None`。
pub fn amounts(bill: &Bill, first: &str, symbols: &HashMap<String, String>) -> Option<String> {
    if bill.amounts.is_empty() {
        return None;
    }
    let mut sorted: Vec<&Cost> = bill.amounts.iter().collect();
    sorted.sort_by(|a, b| {
        (a.currency != first, &a.currency).cmp(&(b.currency != first, &b.currency))
    });
    let parts: Vec<String> = sorted.iter().map(|c| one(c, symbols)).collect();
    Some(parts.join(" + "))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{amount, amounts};
    use crate::core::{Bill, Cost};

    #[test]
    fn three_significant_digits_and_at_least_two_decimals_like_the_core() {
        assert_eq!(amount(0.42), "0.42");
        assert_eq!(amount(1.3), "1.30");
        assert_eq!(amount(0.000292), "0.000292");
        assert_eq!(amount(0.0001), "0.0001");
        assert_eq!(amount(12.345), "12.35");
        assert_eq!(amount(0.0), "0.00");
        assert_eq!(amount(1234.5), "1234.50");
    }

    #[test]
    fn currencies_sit_side_by_side_the_chosen_one_first() {
        let symbols = HashMap::from([
            ("USD".to_string(), "$".to_string()),
            ("CNY".to_string(), "¥".to_string()),
        ]);
        let cost = |amount: f64, currency: &str| Cost {
            amount,
            currency: currency.into(),
        };
        let bill = Bill {
            amounts: vec![cost(1.3, "CNY"), cost(2.0, "HKD"), cost(0.42, "USD")],
            unpriced: 0,
        };
        assert_eq!(
            amounts(&bill, "USD", &symbols).as_deref(),
            Some("$0.42 + ¥1.30 + 2.00 HKD")
        );
        assert_eq!(
            amounts(&bill, "CNY", &symbols).as_deref(),
            Some("¥1.30 + 2.00 HKD + $0.42")
        );
        assert!(amounts(&Bill::default(), "USD", &symbols).is_none());
    }
}
