//! 别的 harness 发来的话（施工 7-10，`docs/blueprint/kernel/request.md`「别的 harness 发来的话」，`agents.md` 第九条第 4 条）：
//! `by` 是 `harness` 的 `message.user` 包一层带名字的标签，她分得清这句是别的代理说的，不是人说的。
//!
//! 名字是对方自己报的，不可信：照模板的规矩转义（`render`），伪造不了标签和属性。原话原样放，和人说的话一样。它不带回合
//! 编号，照它在日志里的位置排，和子代理的留言一样（`render.rs`）。

use std::collections::BTreeMap;

use miyu_kernel::block::Block;
use miyu_kernel::origin::Harness;

use crate::tag::tagged;
use crate::texts::HarnessTexts;

/// 别的 harness `harness` 发来的一句的块：包一层标签，标签里是它的名字；字以外的块接在后面。以前造的快照没有标签的
/// （`texts` 是没有），原样交回，和人的话一字不差。
pub(crate) fn message(
    harness: &Harness,
    blocks: Vec<Block>,
    texts: Option<&HarnessTexts>,
) -> Vec<Block> {
    let Some(texts) = texts else {
        return blocks;
    };
    let fields = BTreeMap::from([("name", harness.name.as_str())]);
    // 造快照时试换过，这里换不出只会是造的时候没查到的 bug，照空的写。
    let open = texts.open.render(&fields).unwrap_or_default();
    tagged(open, blocks, &texts.close)
}

#[cfg(test)]
mod tests;
