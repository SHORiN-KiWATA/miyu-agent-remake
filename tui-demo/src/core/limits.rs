//! 会话的限额（`protocol.md` 的 subscribe，施工 6-3 补）：核心照它的模型算好，订阅的回应里交给头。
//! 头照它画上下文的窗口，不自己照模型名查。

use serde_json::Value;

/// 会话实际用的模型给这个会话多少地方；没报的是 `None`。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Limits {
    /// 上下文窗口，token 数。
    pub window: Option<u64>,
    /// 到这里自动压缩，token 数；窗口太小算不出的没有。
    pub compaction_line: Option<u64>,
}

impl Limits {
    /// 一条回应里的限额：`result.limits`；不是订阅的回应（没有这一格）的是 `None`。
    pub fn of(message: &Value) -> Option<Self> {
        message["result"].get("limits").map(Self::read)
    }

    /// 照 `{window, compaction_line}` 这样一格读（`model.changed` 的 `limits`，核心 8-9）。
    pub fn read(limits: &Value) -> Self {
        Self {
            window: limits["window"].as_u64(),
            compaction_line: limits["compaction_line"].as_u64(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Limits;

    #[test]
    fn a_subscribe_reply_carries_the_limits_and_others_do_not() {
        let full = json!({"id": "c2", "jsonrpc": "2.0",
            "result": {"limits": {"compaction_line": 967_000, "window": 1_000_000}}});
        assert_eq!(
            Limits::of(&full),
            Some(Limits {
                window: Some(1_000_000),
                compaction_line: Some(967_000)
            })
        );
        let none = json!({"id": "c2", "jsonrpc": "2.0", "result": {"limits": {}}});
        assert_eq!(
            Limits::of(&none),
            Some(Limits::default()),
            "没报窗口的是空的"
        );
        let small = json!({"id": "c2", "jsonrpc": "2.0", "result": {"limits": {"window": 8000}}});
        assert_eq!(Limits::of(&small).unwrap().compaction_line, None);
        let other = json!({"id": "c3", "jsonrpc": "2.0", "result": {}});
        assert_eq!(Limits::of(&other), None, "别的回应不算");
    }
}
