//! 抽样（`docs/blueprint/chat.md` 第三条「怎么走」第 6 条，18 第七节）：前面几项一个都不成立、这条消息不是只有图的时候，
//! 照千分比抽一次。加 0 分。别的条件成立时不抽：反正要交给判官，少抽一次，日志也干净（施工时定的第 1 条）。
//!
//! 抽中没有照「场所编号加序号」的哈希算（[`drawn`]），同一份日志回放出同样的结果。

use super::sample::drawn;
use super::{Bonus, BonusCtx, Hit, Kind};

/// 抽样这一项。插槽里排最后：它要看前面的成立了没有。
pub(super) struct Item;

impl Bonus for Item {
    fn judge(&self, ctx: &BonusCtx<'_>, before: &[Hit]) -> Option<Hit> {
        let facts = ctx.facts;
        let holds = before.is_empty()
            && !facts.media_only
            && drawn(&facts.venue, facts.msg, ctx.chatty.probability);
        holds.then_some(Hit {
            kind: Kind::Probability,
            bonus: 0.0,
        })
    }
}
