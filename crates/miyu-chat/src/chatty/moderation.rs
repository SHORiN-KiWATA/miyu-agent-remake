//! 违规旗（`docs/blueprint/chat.md` 第三条「怎么走」第 5 条，18 第七节）：进站链插了 [`Flag::Moderation`]。加 0 分，只让
//! 判官认真查一眼；只有它的，判官只查违规（[`Route::ModerationOnly`](super::Route::ModerationOnly)）。

use crate::Flag;

use super::{Bonus, BonusCtx, Hit, Kind};

/// 违规旗这一项。
pub(super) struct Item;

impl Bonus for Item {
    fn name(&self) -> &str {
        "moderation"
    }

    fn judge(&self, ctx: &BonusCtx<'_>, _before: &[Hit]) -> Option<Hit> {
        ctx.flags.contains(&Flag::Moderation).then_some(Hit {
            kind: Kind::Moderation,
            bonus: 0.0,
        })
    }
}
