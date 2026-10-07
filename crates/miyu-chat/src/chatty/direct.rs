//! 冲她来（`docs/blueprint/chat.md` 第三条「怎么走」第 2 条，18 第七节）：@ 她、回复她、叫到名字或触发词，由外面算好交进来
//! （[`Facts::addressed`](super::Facts::addressed)）。加 `direct` 分，免冷静（免冷静写在冷静里，施工时定的第 3 条）。

use super::{Bonus, BonusCtx, Hit, Kind};

/// 冲她来这一项。
pub(super) struct Item;

impl Bonus for Item {
    fn name(&self) -> &str {
        "direct"
    }

    fn judge(&self, ctx: &BonusCtx<'_>, _before: &[Hit]) -> Option<Hit> {
        ctx.facts.addressed.then_some(Hit {
            kind: Kind::Direct,
            bonus: ctx.chatty.direct,
        })
    }
}
