//! 一条记忆印成一行（`docs/blueprint/cli/memory.md`「样子」）：`<编号>  <日期>  <正文>`，编号照最长的那个对齐；日期照这台
//! 机器此刻的时区换成那一天；作废的后面接一截。只放人要的：出处、谁记的、类不印（2026-10-08 项目主人定的设计原则）。

use serde_json::Value;

use miyu_kernel::time::{Timestamp, UtcOffset};

use crate::language::Language;

/// 核心回的那一串（`memory.list`、`memory.search` 的 `memories`），一条一行，不带换行。读不懂的时刻照原样写。
pub(crate) fn listed(memories: &[Value], offset: UtcOffset, language: &Language) -> Vec<String> {
    let id = |memory: &Value| memory["id"].as_str().unwrap_or_default().to_string();
    let width = memories
        .iter()
        .map(|memory| id(memory).len())
        .max()
        .unwrap_or(0);
    memories
        .iter()
        .map(|memory| {
            let at = memory["at"].as_str().unwrap_or_default();
            let day =
                Timestamp::parse(at).map_or_else(|_| at.to_string(), |at| at.local_date(offset));
            let text = memory["text"].as_str().unwrap_or_default();
            let mut row = format!("{:width$}  {day}  {text}", id(memory));
            if let Some(why) = memory["retired"].as_str() {
                row.push_str(&language.forgotten_mark(why));
            }
            row
        })
        .collect()
}
