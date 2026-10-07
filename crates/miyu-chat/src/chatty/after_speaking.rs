//! 刚说过话（`docs/blueprint/chat.md` 第三条「怎么走」第 4 条，18 第七节）：她最近一轮回复（回谁都算）还在窗口里，任何人
//! 接着说。只有表情、没有字的不算：贴个表情不是在接话。

use super::{Bonus, BonusCtx, Hit, Kind, latest, within};

/// 刚说过话这一项。
pub(super) struct Item;

impl Bonus for Item {
    fn judge(&self, ctx: &BonusCtx<'_>, _before: &[Hit]) -> Option<Hit> {
        let window = ctx.chatty.after_speaking;
        let now = ctx.clock.now;
        let last = latest(ctx.replies, now)?;
        let holds = !ctx.facts.textless && within(last.at, now, window.window);
        holds.then_some(Hit {
            kind: Kind::AfterSpeaking,
            bonus: window.bonus,
        })
    }
}
