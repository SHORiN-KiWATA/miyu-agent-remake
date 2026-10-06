//! 本会话放行过的（施工 D-1，`docs/blueprint/kernel/asking.md`「本会话放行过的」）：人回答确认时选了「本会话都允许」，
//! 那一次请求提的放行规则就记下来，以后交给链的每一次都带上，由链照它判（内核只记，不判，`02-内核.md` 第六节「确认怎么走」
//! 第 3 条）。
//!
//! - 照日志算：活着时每追加一条记一次，载入时照日志再走一遍，和 `sight.rs` 一样。不另开事件：`tool.approval_requested`
//!   的规则加上 `tool.approval_decided` 的 `session` 就是全部。
//! - 撤销、压缩都不收回：放行是人的决定，不是她说的话（施工 D-1 定）。
//! - 请求等不到决定就了结的（打断、跳过、收紧成只读拦下），它的规则留在 `asked` 里不再有人认领：调用编号在一个会话里不会
//!   重复，留着不会放行任何东西，一个请求几十个字节，不另外清。

use std::collections::BTreeMap;

use crate::event::{Body, Decision, Event};
use crate::id::CallId;
use crate::raw::RawJson;

/// 这个会话放行过的规则，和还在等人决定的请求提的规则。
#[derive(Debug, Default)]
pub(super) struct Grants {
    /// 还在等人决定的请求提的规则：调用编号 → 规则。没提规则的不记。
    asked: BTreeMap<CallId, RawJson>,
    /// 人选了「本会话都允许」的规则，照先后；一样的只记一次。
    granted: Vec<RawJson>,
}

impl Grants {
    /// 记下追加的一条：请求记下它提的规则；决定是「本会话都允许」的，把那条规则挪进放行过的。
    pub(super) fn note(&mut self, event: &Event) {
        match &event.body {
            Body::ApprovalRequested(requested) => {
                if let Some(rule) = &requested.rule {
                    self.asked.insert(requested.call_id, rule.clone());
                }
            }
            Body::ApprovalDecided(decided) => {
                let rule = self.asked.remove(&decided.call_id);
                if let (Some(rule), Decision::Session) = (rule, &decided.decision)
                    && !self.granted.contains(&rule)
                {
                    self.granted.push(rule);
                }
            }
            _ => {}
        }
    }

    /// 放行过的规则，照先后：交给链。
    pub(super) fn granted(&self) -> Vec<RawJson> {
        self.granted.clone()
    }
}

#[cfg(test)]
mod tests;
