//! 文本文件照字放进消息（施工 3-9 三补，`docs/blueprint/drivers/openai-chat.md` 第 9 条）：人附的文件是文本的，模型不用
//! 另有本事就读得了，内容照字给她，不写「看不了」。
//!
//! 什么算文本照内容认，不看媒体类型和扩展名：整份是合法的 UTF-8，又没有 NUL 字节（`read` 认二进制也看 NUL，
//! `tools/read.md`）。`blob.put` 定媒体类型时照同一条（`protocol.md`），所以认成 `text/plain` 的一定照字发。
//!
//! 一份最多给 [`LIMIT`] 个字节：放多了一个附件就能占掉大半个上下文，还每次请求都背着。多的截在字的边界上，原文整份
//! 留在 blob 里。

/// 一份文本文件最多给她多少个字节：64 KiB。
pub const LIMIT: usize = 64 * 1024;

/// 这些字节是不是一份文本文件：整份是合法的 UTF-8，又没有 NUL 字节。空的也算。
pub fn is_text(bytes: &[u8]) -> bool {
    as_text(bytes).is_some()
}

/// 是文本文件的，交回它的字；不是的空的。
pub(crate) fn as_text(bytes: &[u8]) -> Option<&str> {
    std::str::from_utf8(bytes)
        .ok()
        .filter(|_| !bytes.contains(&0))
}

/// 给她看的那一截：`text` 不超过 [`LIMIT`] 个字节的整份；超了的截到 [`LIMIT`] 以内、最后一个字的边界上。
pub(crate) fn shown(text: &str) -> &str {
    if text.len() <= LIMIT {
        return text;
    }
    let mut end = LIMIT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests;
