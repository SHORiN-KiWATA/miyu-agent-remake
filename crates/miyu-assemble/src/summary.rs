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

/// 从回复里取出摘要：只看正文块，思考不要。先去掉草稿：草稿里会顺嘴提到 `<summary>` 这个标签
/// （`Now writing the <summary>.`），从那儿取就把草稿的尾巴带进了摘要（施工 6-3 下真模型两次都这样）。剩下的有 `<summary>` 的，取它到
/// 最后一个 `</summary>` 之间的：摘要里引的 HTML 也有 `</summary>`，真收尾的在最后；没有收尾的（输出到了上限）取到
/// 末尾。没有 `<summary>` 的，剩下的当摘要。前后空白去掉，是空的就是没取到。
pub(crate) fn extract(reply: &[Block]) -> Option<String> {
    let text: String = reply
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect();
    let text = without_analysis(&text);
    let summary = match text.find(SUMMARY.0) {
        Some(start) => {
            let body = &text[start + SUMMARY.0.len()..];
            body.rfind(SUMMARY.1).map_or(body, |end| &body[..end])
        }
        None => &text,
    };
    let summary = summary.trim();
    (!summary.is_empty()).then(|| summary.to_string())
}

/// 去掉第一段 `<analysis>…</analysis>`。没收尾的草稿，摘要写在它里面的，从最后一个 `<summary>` 起留下（前面提到的
/// 标签都在草稿里）；没有的一直到末尾都算草稿。
fn without_analysis(text: &str) -> String {
    let Some(start) = text.find(ANALYSIS.0) else {
        return text.to_string();
    };
    let rest = &text[start..];
    let after = match rest.find(ANALYSIS.1) {
        Some(end) => &rest[end + ANALYSIS.1.len()..],
        None => rest.rfind(SUMMARY.0).map_or("", |at| &rest[at..]),
    };
    format!("{}{after}", &text[..start])
}

#[cfg(test)]
mod tests;
