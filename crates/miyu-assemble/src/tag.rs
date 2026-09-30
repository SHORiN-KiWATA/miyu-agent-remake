//! 一块带标签的事实（施工 7-7 从 `jobs.rs` 拿出来，施工 7-10 别的 harness 发来的话也用）：别处来的一句话，开头一行是标签，
//! 接着是原话，最后是收尾的标签，拼成一块字；字以外的块接在这一块后面。

use miyu_kernel::block::{Block, Text};

/// 把 `blocks` 包进标签：`open` 是填好字段的开头一行，几块字照先后接上、原样不转义（是别人写的多行正文），末尾没有换行
/// 的补一个，再接 `close`。字以外的块（附件）照先后接在这一块后面。一块字都没有的，这一块是开头接收尾。
pub(crate) fn tagged(open: String, blocks: Vec<Block>, close: &str) -> Vec<Block> {
    let mut text = open;
    let mut rest = Vec::new();
    for block in blocks {
        match block {
            Block::Text(said) => text.push_str(&said.text),
            other => rest.push(other),
        }
    }
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(close);
    let mut tagged = vec![Block::Text(Text { text })];
    tagged.extend(rest);
    tagged
}
