//! 花了多少钱（核心 8-15，蓝图 `tui.md`「配置与模型」第 5 条）：一次请求冻结在 `model.called` 的 `cost`，
//! `usage.query` 交回一行行合计。头不自己算金额，几种币各加各的、不换算。

use serde_json::Value;

/// 一笔钱：多少、什么币。相等照位比（和核心的 `Real` 一样），所以能放进要 `Eq` 的推送里。
#[derive(Debug, Clone)]
pub struct Cost {
    /// 多少，不取整。
    pub amount: f64,
    /// ISO 4217 的三个大写字母。
    pub currency: String,
}

impl PartialEq for Cost {
    fn eq(&self, other: &Self) -> bool {
        self.amount.to_bits() == other.amount.to_bits() && self.currency == other.currency
    }
}

impl Eq for Cost {}

impl Cost {
    /// 读 `model.called` 的 `cost`；算不出的（没有这一格、写坏了）是 `None`。
    pub fn read(cost: &Value) -> Option<Cost> {
        Some(Cost {
            amount: cost["amount"].as_f64()?,
            currency: cost["currency"].as_str()?.to_string(),
        })
    }
}

/// 加起来的账：照币种各加各的，另记有用量、没金额的几次。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bill {
    /// 每种币一项，照头一次见到的先后（显示时再排）。
    pub amounts: Vec<Cost>,
    /// 没有价格的几次。
    pub unpriced: u64,
}

impl Bill {
    /// 记一次请求：有金额的加进它的币种，没有的算一次没价格。
    pub fn add(&mut self, cost: Option<&Cost>) {
        match cost {
            Some(cost) => self.put(cost),
            None => self.unpriced += 1,
        }
    }

    /// 把另一份账（子代理的）加进来。
    pub fn merge(&mut self, other: &Bill) {
        for cost in &other.amounts {
            self.put(cost);
        }
        self.unpriced += other.unpriced;
    }

    fn put(&mut self, cost: &Cost) {
        match self
            .amounts
            .iter_mut()
            .find(|c| c.currency == cost.currency)
        {
            Some(have) => have.amount += cost.amount,
            None => self.amounts.push(cost.clone()),
        }
    }
}

/// `/usage` 要的三样（蓝图「配置与模型」第 5 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageKind {
    /// 全部的合计，不分组。
    Total,
    /// 按天，全部时间。
    Days,
    /// 按模型，全部时间。
    Models,
    /// 按会话，全部时间。
    Sessions,
    /// 按用途，全部时间。
    Purposes,
}

/// 一次 `usage.query`：哪一样、从哪个时刻起、分天照哪个时区（`+09:00`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAsk {
    /// 哪一样。
    pub kind: UsageKind,
    /// 从这个时刻起（RFC 3339）；全部时间的是 `None`。
    pub from: Option<String>,
    /// 分天的时区；不分天的是 `None`。
    pub offset: Option<String>,
}

impl UsageAsk {
    /// 请求的参数。
    pub fn params(&self) -> Value {
        let mut params = match self.kind {
            UsageKind::Total => serde_json::json!({}),
            UsageKind::Days => serde_json::json!({"group": ["day"]}),
            UsageKind::Models => serde_json::json!({"group": ["model"]}),
            UsageKind::Sessions => serde_json::json!({"group": ["session"]}),
            UsageKind::Purposes => serde_json::json!({"group": ["purpose"]}),
        };
        if let Some(from) = &self.from {
            params["from"] = Value::from(from.clone());
        }
        if let Some(offset) = &self.offset {
            params["offset"] = Value::from(offset.clone());
        }
        params
    }
}

/// `usage.query` 交回的一行。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageRow {
    /// 按天分的：那一天（`2026-10-07`）。
    pub day: Option<String>,
    /// 按模型分的：`供应商/模型`。
    pub model: Option<String>,
    /// 按会话分的：会话编号（一次性调用没有）。
    pub session: Option<String>,
    /// 按用途分的：`title`、`recap`、`vision`……；主请求、摘要请求的是 `None`。
    pub purpose: Option<String>,
    /// 发出去的请求数。
    pub requests: u64,
    /// 四项用量加起来。
    pub tokens: u64,
    /// 金额和没价格的几次。
    pub bill: Bill,
}

/// 读 `usage.query` 的回应：一行行。
pub fn rows(result: &Value) -> Vec<UsageRow> {
    let Some(rows) = result["rows"].as_array() else {
        return Vec::new();
    };
    rows.iter()
        .map(|row| {
            let usage = &row["usage"];
            let n = |k: &str| usage[k].as_u64().unwrap_or_default();
            let amounts = row["amounts"]
                .as_array()
                .map(|list| list.iter().filter_map(Cost::read).collect())
                .unwrap_or_default();
            UsageRow {
                day: row["day"].as_str().map(str::to_string),
                model: row["model"].as_str().map(str::to_string),
                session: row["session"].as_str().map(str::to_string),
                purpose: row["purpose"].as_str().map(str::to_string),
                requests: row["requests"].as_u64().unwrap_or_default(),
                tokens: n("uncached") + n("cache_read") + n("cache_write") + n("output"),
                bill: Bill {
                    amounts,
                    unpriced: row["unpriced"].as_u64().unwrap_or_default(),
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Bill, Cost, rows};

    #[test]
    fn currencies_add_up_separately_and_missing_prices_are_counted() {
        let usd = Cost::read(&json!({"amount": 0.25, "currency": "USD", "multiplier": 1})).unwrap();
        let cny = Cost::read(&json!({"amount": 1.3, "currency": "CNY"})).unwrap();
        let mut bill = Bill::default();
        bill.add(Some(&usd));
        bill.add(None);
        bill.add(Some(&cny));
        let mut child = Bill::default();
        child.add(Some(&usd));
        bill.merge(&child);
        assert_eq!(bill.amounts.len(), 2, "不换算");
        assert!((bill.amounts[0].amount - 0.5).abs() < 1e-12);
        assert_eq!(bill.unpriced, 1);
        assert!(Cost::read(&json!(null)).is_none(), "算不出的没有");
    }

    #[test]
    fn a_usage_query_reply_reads_into_rows() {
        let got = rows(
            &json!({"rows": [{"day": "2026-10-07", "model": null, "requests": 2,
            "usage": {"uncached": 10, "cache_read": 20, "cache_write": 0, "output": 5},
            "amounts": [{"currency": "USD", "amount": 0.0002}], "unpriced": 1}]}),
        );
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].day.as_deref(), Some("2026-10-07"));
        assert_eq!(got[0].tokens, 35);
        assert_eq!(got[0].bill.unpriced, 1);
        assert!(rows(&json!({})).is_empty());
    }
}
