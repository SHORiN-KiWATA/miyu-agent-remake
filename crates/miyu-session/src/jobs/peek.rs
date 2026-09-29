//! 子代理在做什么（施工 7-4，`docs/blueprint/tools/jobs.md`「output」）：照子会话的日志看它最近的回答、这一轮完没完、这一步
//! 在跑哪几件工具。会话表照子会话的日志读出来，交给 [`peek`] 算（`SessionPort::peek`）。

use std::collections::BTreeSet;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event};

/// 一个子会话这会儿的样子。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Peek {
    /// 它最近的回答：最后一条回复里的字，几段接起来。一条回复都没有、最后那条一个字都没有的是空的。
    pub reply: Option<String>,
    /// 最近的回答是这一轮（最后一次开的那一轮）里说的。停掉它时交回的回报只带这一轮的。
    pub this_turn: bool,
    /// 它这一轮还没完。
    pub working: bool,
    /// 这一轮最后那条回复里调了、还没有结果的工具，照先后。
    pub doing: Vec<String>,
}

/// 照子会话日志里的全部事件算它这会儿的样子。
pub fn peek(events: &[Event]) -> Peek {
    let mut peek = Peek::default();
    let mut calls = Vec::new();
    let mut done = BTreeSet::new();
    for event in events {
        match &event.body {
            Body::TurnStarted(_) => {
                peek.working = true;
                peek.this_turn = false;
                calls.clear();
            }
            Body::TurnEnded(_) => {
                peek.working = false;
                calls.clear();
            }
            Body::MessageAssistant(message) => {
                let text: Vec<&str> = message
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        Block::Text(text) => Some(text.text.as_str()),
                        _ => None,
                    })
                    .collect();
                if !text.is_empty() {
                    peek.reply = Some(text.concat());
                    peek.this_turn = true;
                }
                calls = message
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        Block::ToolCall(call) => Some((call.call_id, call.name.clone())),
                        _ => None,
                    })
                    .collect();
                done.clear();
            }
            Body::ToolResult(result) => {
                done.insert(result.call_id);
            }
            _ => {}
        }
    }
    if peek.working {
        peek.doing = calls
            .into_iter()
            .filter(|(call, _)| !done.contains(call))
            .map(|(_, name)| name)
            .collect();
    }
    peek
}

#[cfg(test)]
mod tests;
