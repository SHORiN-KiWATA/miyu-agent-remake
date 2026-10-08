//! 续聊（`docs/blueprint/chat.md` 第三条「怎么走」第 3 条，18 第七节）：她最近一轮回的是这个人，他在窗口里接着说。只认
//! 最近一轮：那一轮回的人里有他就算（一轮可以回几个人）；她后来回了别人，前面回过他的就不算了。这条消息 @ 了别人、或者引用的是别人的消息，是对别人说的，不算
//! （2026-10-07 项目主人定）。没有明确回谁的回复不算谁的续聊。

use super::{Bonus, BonusCtx, Hit, Kind, latest, within};

/// 续聊这一项。
pub(super) struct Item;

impl Bonus for Item {
    fn judge(&self, ctx: &BonusCtx<'_>, _before: &[Hit]) -> Option<Hit> {
        let facts = ctx.facts;
        let window = ctx.chatty.continuation;
        let now = ctx.clock.now;
        let last = latest(ctx.replies, now)?;
        let holds = last.to.contains(&facts.said.sender)
            && within(last.at, now, window.window)
            && !facts.mentions_others
            && !facts.quotes_other;
        holds.then_some(Hit {
            kind: Kind::Continuation,
            bonus: window.bonus,
        })
    }
}
