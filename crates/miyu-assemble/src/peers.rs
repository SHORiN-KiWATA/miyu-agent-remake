//! 别的会话发来的话（施工 C-2，`docs/blueprint/kernel/request.md`「别的会话发来的话」，`cross-session.md` 第八条）：`by` 是
//! 别的会话（既不是父会话、也不是这个会话派的子代理，`History::is_peer`）的 `message.user` 包一层标签，标签里是发话的
//! 会话的短编号，她分得清这句是别的会话说的，不是人说的。
//!
//! 短编号照 `by` 的编号算，不另记，不看撞没撞（前缀要稳）。标签不带标题：标题会改。另用一个标签名，不借别的 harness 的：
//! harness 报的名字是它自己写的，借同一个标签它就能冒充一个会话。原话原样放，和人说的话一样；它不带回合编号，照它在
//! 日志里的位置排，和子代理的留言一样（`render.rs`）。

use std::collections::BTreeMap;

use miyu_kernel::block::Block;
use miyu_kernel::id::SessionId;

use crate::tag::tagged;
use crate::texts::PeerTexts;

/// 别的会话 `session` 发来的一句的块：包一层标签，标签里是它的短编号；字以外的块接在后面。以前造的快照没有标签的
/// （`texts` 是没有），原样交回，和人的话一字不差。
pub(crate) fn message(
    session: &SessionId,
    blocks: Vec<Block>,
    texts: Option<&PeerTexts>,
) -> Vec<Block> {
    let Some(texts) = texts else {
        return blocks;
    };
    let fields = BTreeMap::from([("id", session.short())]);
    // 造快照时试换过，这里换不出只会是造的时候没查到的 bug，照空的写。
    let open = texts.open.render(&fields).unwrap_or_default();
    tagged(open, blocks, &texts.close)
}

#[cfg(test)]
mod tests;
