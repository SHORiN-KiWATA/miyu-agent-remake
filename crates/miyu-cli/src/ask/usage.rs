//! 用量（`docs/designs/22-命令行.md` 第三节，施工 3-9 下）：这一轮每次请求的用量加起来。输入 = 没命中 + 命中 +
//! 写进缓存，命中率 = 命中 ÷ 输入。

use serde_json::{Value, json};

/// 加起来的用量。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sum {
    /// 没命中缓存的输入。
    pub(crate) uncached: u64,
    /// 命中缓存的输入。
    pub(crate) cache_read: u64,
    /// 写进缓存的输入。
    pub(crate) cache_write: u64,
    /// 输出。
    pub(crate) output: u64,
    /// 供应商报过用量：没报过的，那一行不印。
    pub(crate) seen: bool,
}

impl Sum {
    /// 加上一次请求的用量（`model.called` 的 `usage`）。
    pub(crate) fn add(&mut self, usage: &Value) {
        if !usage.is_object() {
            return;
        }
        self.seen = true;
        let field = |name: &str| usage[name].as_u64().unwrap_or(0);
        self.uncached += field("uncached");
        self.cache_read += field("cache_read");
        self.cache_write += field("cache_write");
        self.output += field("output");
    }

    /// 输入一共多少。
    pub(crate) fn input(&self) -> u64 {
        self.uncached + self.cache_read + self.cache_write
    }

    /// 命中率，四舍五入到百分之一；没有输入的没有。
    pub(crate) fn percent(&self) -> Option<u64> {
        let input = self.input();
        (input > 0).then(|| (self.cache_read * 100 + input / 2) / input)
    }

    /// `--format json` 里的写法。
    pub(crate) fn json(&self) -> Value {
        json!({
            "input": self.input(),
            "cache_read": self.cache_read,
            "cache_write": self.cache_write,
            "output": self.output,
        })
    }
}

/// 三位一撇：`1830` 写成 `1,830`。
pub(crate) fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut grouped = String::new();
    for (k, digit) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

#[cfg(test)]
mod tests;
