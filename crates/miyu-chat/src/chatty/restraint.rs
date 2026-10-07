//! 冷静（`docs/blueprint/chat.md` 第三条「怎么走」第 10 条，18 第七节）：自带的唯一一个门槛修正。她在这个场所每真发出
//! 一轮回复记一笔，每笔照半衰期衰减，加起来是近期发言量 p（[`pressure`]）；插嘴的门槛抬 `cap × p³ ÷ (p³ + k³)`：一两句
//! 几乎不抬，连说三四句才明显，再多也不超过 `cap`。
//!
//! 冷静只管插嘴：冲她来的、判官说是在跟她说话的，不抬。这条规矩写在这个修正里，不写在算分的框架里（施工时定的第 3 条）。

use crate::Clock;

use super::{Chatty, Kind, Lift, LiftCtx, Reply};

/// 冷静的近期发言量 p：对 `replies` 里不晚于此刻的每一轮，加 `0.5^((now − at) ÷ half_life)`。此刻刚发的一轮算 1，过一个
/// 半衰期算 0.5。数的是轮，不是段（施工时定的第 4 条）；先后不要紧。
///
/// 不看冷静开没开：开关在 [`Lift`] 那一层看。
pub fn pressure(replies: &[Reply], clock: Clock, chatty: &Chatty) -> f64 {
    // 毫秒换成 f64：差出 2⁵³ 毫秒（约 28 万年）才丢精度，用不着管。
    let half_life = chatty.restraint.half_life as f64;
    replies
        .iter()
        .filter(|reply| reply.at <= clock.now)
        .map(|reply| {
            let age = (clock.now.unix_millis() - reply.at.unix_millis()) as f64;
            0.5_f64.powf(age / half_life)
        })
        .sum()
}

/// 冷静这个修正。
pub(super) struct Item;

impl Lift for Item {
    fn lift(&self, ctx: &LiftCtx<'_>) -> f64 {
        let restraint = ctx.chatty.restraint;
        let exempt = ctx.conditions.has(Kind::Direct) || ctx.judgement.to_bot;
        if !restraint.on || exempt {
            return 0.0;
        }
        let cubed = pressure(ctx.replies, ctx.clock, ctx.chatty).powi(3);
        restraint.cap * cubed / (cubed + restraint.k.powi(3))
    }
}

#[cfg(test)]
mod tests;
