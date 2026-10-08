//! 改人格、预设时的两种拒绝的写法（施工 P-3 中、补）：撞上别人先改的、写错了多带一句的。`refusal.rs` 到了 500 行，挪到这里。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 改人格、预设撞上了别人先改（施工 P-3 中）：`expect` 对不上的 `data.current` 是你那一层现在的样子；写的那一瞬间
    /// 有人手改、重来三次还不行的不带。
    pub(crate) fn conflict(reason: &'static str, current: Option<serde_json::Value>) -> Refusal {
        match current {
            Some(current) => Refusal::with(reason, "current", current),
            None => Refusal {
                code: REFUSED,
                reason,
                data: None,
            },
        }
    }

    /// 写错了的人格、预设多带一句照连接的语言的 `message` 和第几行 `line`（施工 P-3 补，同 `check` 的写法，不带层）；没有的
    /// 不写。
    pub(crate) fn telling(mut self, message: Option<String>, line: Option<usize>) -> Refusal {
        let data = self.data.get_or_insert_with(serde_json::Map::new);
        if let Some(message) = message {
            data.insert("message".to_string(), serde_json::Value::String(message));
        }
        if let Some(line) = line {
            data.insert("line".to_string(), serde_json::json!(line));
        }
        self
    }
}
