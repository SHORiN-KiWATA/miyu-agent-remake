//! 压缩的摘要请求，和从回复里取摘要（`docs/blueprint/compaction.md` 第三条第 3、6 条，施工 6-2 上）。
//!
//! 摘要请求是有效历史到第 N 条的投影，原样，最后是摘要指令：第 N 条刚写下时的有效历史就是这样，所以它是那时
//! 那次请求的前缀延伸，几乎全部命中缓存。回复先是 `<analysis>` 草稿，再是 `<summary>` 摘要，只存摘要。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::request::Message;

/// 草稿、摘要的标签。
const ANALYSIS: (&str, &str) = ("<analysis>", "</analysis>");
const SUMMARY: (&str, &str) = ("<summary>", "</summary>");

/// 摘要指令接在最后：最后一条是 user 的，并进这一条，做它的最后一块；不是的，另起一条 user。人这一边挨着的块本来
/// 就合成一条，不出连着的两条 user。
pub(crate) fn instruct(messages: &mut Vec<Message>, instruction: &str) {
    let block = Block::Text(Text {
        text: instruction.to_string(),
    });
    match messages.last_mut() {
        Some(Message::User { blocks }) => blocks.push(block),
        _ => messages.push(Message::User {
            blocks: vec![block],
        }),
    }
}

/// 从回复里取出摘要：只看正文块，思考不要。有 `<summary>` 的，取它和 `</summary>` 之间的，没有收尾的（输出到了上限）
/// 取到末尾；没有的，去掉 `<analysis>…</analysis>` 那一段，剩下的当摘要。前后空白去掉，是空的就是没取到。
pub(crate) fn extract(reply: &[Block]) -> Option<String> {
    let text: String = reply
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect();
    let summary = match text.find(SUMMARY.0) {
        Some(start) => {
            let body = &text[start + SUMMARY.0.len()..];
            body.find(SUMMARY.1).map_or(body, |end| &body[..end])
        }
        None => &without_analysis(&text),
    };
    let summary = summary.trim();
    (!summary.is_empty()).then(|| summary.to_string())
}

/// 去掉第一段 `<analysis>…</analysis>`；没收尾的草稿一直到末尾都算草稿。
fn without_analysis(text: &str) -> String {
    let Some(start) = text.find(ANALYSIS.0) else {
        return text.to_string();
    };
    let rest = &text[start..];
    let after = rest
        .find(ANALYSIS.1)
        .map_or("", |end| &rest[end + ANALYSIS.1.len()..]);
    format!("{}{after}", &text[..start])
}

#[cfg(test)]
mod tests;
