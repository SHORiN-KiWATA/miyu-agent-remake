//! 这一轮不说话了（施工 O-26，`onebot.md` 第一条「提供者和不说话」第 3 条）：纯逻辑。会话推来她的一条回复（`message.assistant`），
//! 里面有 `skip_reply` 的工具调用块的，这个会话的这一回合就算不说话了：这一条的字、这一轮以后她说的都不发。照回复里的调用块认，
//! 不等 `tool.call`（施工单「要定的」第 1 条）：同一条回复里先说的字，桥收到时工具还没执行。
//!
//! 只记在内存里（「施工时定的」第 142 条）：（会话，回合编号）的一张表，推来那一轮的 `turn.ended` 清掉。群里桥重启照日志从头补，
//! 那一轮还在跑的照样认得出；私聊不补从前的。事件照原样的 JSON 看，群里、私聊一个看法。

use std::collections::HashSet;

use serde_json::Value;

use crate::rules::SKIP_REPLY;

/// 这一轮不说话了的会话和回合。
#[derive(Debug, Default)]
pub(super) struct Quiet {
    /// （会话编号，回合编号）。
    turns: HashSet<(String, u64)>,
}

impl Quiet {
    /// 会话 `session` 推来的一条事件 `event`：她的回复里有 `skip_reply` 的调用块的，记下这一轮；这一轮的 `turn.ended` 清掉。交回
    /// 这一条是不是她这一轮不说话了以后的回复：是的，里面的字不发。没有回合编号的不看。
    pub(super) fn heard(&mut self, session: &str, event: &Value) -> bool {
        let Some(turn) = event["turn"].as_u64() else {
            return false;
        };
        let key = (session.to_string(), turn);
        match event["kind"].as_str() {
            Some("message.assistant") => {
                if skips(event) {
                    self.turns.insert(key.clone());
                }
                self.turns.contains(&key)
            }
            Some("turn.ended") => {
                self.turns.remove(&key);
                false
            }
            _ => false,
        }
    }
}

/// 回复 `event` 里有没有 `skip_reply` 的工具调用块。
fn skips(event: &Value) -> bool {
    event["body"]["blocks"].as_array().is_some_and(|blocks| {
        blocks
            .iter()
            .any(|block| block["type"] == "tool_call" && block["name"] == SKIP_REPLY)
    })
}

#[cfg(test)]
mod tests;
