//! 抽取交回来的怎么读（施工 R-6 上，`docs/blueprint/memory.md` 第六条第 4 款）：模型交回一个 JSON 对象
//! `{"memories": [{"class", "text", "turn", "about"}]}`，零条是常态。纯逻辑：调的一方把这一段有哪几轮、一条最多几个字交进来。
//!
//! - 照规矩取：从第一个 `{` 到最后一个 `}`，前后多说的话不管（模型爱包一层说明、代码块）；这一段读不成 JSON 对象、没有
//!   `memories` 列表的，整次算失败。
//! - 一条不合的丢掉、别的照收：类不在出厂四类里、正文空的或超了字数、`turn` 不是这一段里的一轮。`about` 不是
//!   `YYYY-MM-DD` 的当没写，这一条照收。

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use serde_json::Value;

use crate::memory::CLASSES;

/// 一条候选。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// 类：出厂四类之一。
    pub class: String,
    /// 正文，去掉了前后空白。
    pub text: String,
    /// 出处那一轮的编号（回合编号的数）：一定是交进来的那几轮之一。
    pub turn: u64,
    /// 说的是哪天的事（`YYYY-MM-DD`）；没写的、写法不对的没有。
    pub about: Option<String>,
}

/// 读交回来的字：这一段有 `turns` 这几轮，一条最多 `max_chars` 个字。
///
/// # Errors
///
/// 找不到 `{…}`、读不成 JSON 对象、没有 `memories` 列表：英文的一句为什么。
pub fn candidates(
    answer: &str,
    turns: &BTreeSet<u64>,
    max_chars: usize,
) -> Result<Vec<Candidate>, String> {
    let (Some(start), Some(end)) = (answer.find('{'), answer.rfind('}')) else {
        return Err("no JSON object in the answer".to_string());
    };
    if end < start {
        return Err("no JSON object in the answer".to_string());
    }
    let value: Value = serde_json::from_str(&answer[start..=end])
        .map_err(|error| format!("answer not readable: {error}"))?;
    let listed = value["memories"]
        .as_array()
        .ok_or_else(|| "no memories list in the answer".to_string())?;
    Ok(listed
        .iter()
        .filter_map(|item| one(item, turns, max_chars))
        .collect())
}

/// 一条：不合的是 `None`。
fn one(item: &Value, turns: &BTreeSet<u64>, max_chars: usize) -> Option<Candidate> {
    let class = item["class"].as_str()?;
    if !CLASSES.contains(&class) {
        return None;
    }
    let text = item["text"].as_str()?.trim();
    if text.is_empty() || text.chars().count() > max_chars {
        return None;
    }
    let turn = item["turn"].as_u64().filter(|turn| turns.contains(turn))?;
    let about = item["about"].as_str().filter(|about| is_date(about));
    Some(Candidate {
        class: class.to_string(),
        text: text.to_string(),
        turn,
        about: about.map(str::to_string),
    })
}

/// 是不是 `YYYY-MM-DD` 的样子：只看字符，不查日子在不在。
fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(at, byte)| match at {
            4 | 7 => *byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}
