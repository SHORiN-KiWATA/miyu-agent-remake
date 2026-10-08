//! 限流（`docs/blueprint/chat.md` 第二条「怎么走」第 6、7 条，`18-通讯平台.md` 第六节 Q22）：数这个场所最近一段时间开了
//! 几个回合（不数消息），滑动窗口。满了，冲她来的这一回第一次回一句，之后只记下；不冲她来的只记下。主人、自己人不限。
//!
//! 回合的时刻、提示过的时刻由外面从场所会话的日志投影出来交进来，核心或桥重启都不丢。

use std::num::NonZeroU32;

use miyu_kernel::time::Timestamp;

use super::{Clock, Ctx, Inbound, InboundRule, Outcome, Standing, Step, Why};

/// 限流的额度：窗口里最多几个回合。只能从场所规则 `rate` 的原文读（[`Rate::read`]），格不公开：读出来的回合数不会是 0、
/// 窗口不会是 0（施工时定的第 13 条）。不限的读成 `None`，不在这里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    /// 窗口里最多几个回合，1 到 10000：到了这个数就是满了。
    pub(crate) turns: NonZeroU32,
    /// 窗口多少毫秒，1 秒到 1 天：此刻往前这么长（不含正好那一刻），到此刻（含）。
    pub(crate) window: i64,
}

/// 额度满了的情形。
pub(super) struct Full {
    /// 这一回满了从什么时候算起：窗口里从早往晚数第 `turns` 个回合的时刻。窗口从这一刻起一直是满的；降下去再满，
    /// 它往后挪，就是新的一回（施工时定的第 4 条）。
    pub(super) since: Timestamp,
    /// 窗口里最早那个回合出窗口的时刻（毫秒）：回合闸推迟到它（「怎么走」第 8 条）。
    pub(super) frees: i64,
}

/// 此刻 `now` 额度满了没有：`turns` 里落在 `(now − window, now]` 的个数不少于额度。没有限流的不会满。
pub(super) fn full(ctx: &Ctx, now: Timestamp) -> Option<Full> {
    let rate = ctx.rate?;
    let from = now.unix_millis().saturating_sub(rate.window);
    let mut recent: Vec<Timestamp> = ctx
        .turns
        .iter()
        .copied()
        .filter(|&turn| from < turn.unix_millis() && turn <= now)
        .collect();
    recent.sort_unstable();
    // `turns` 至少是 1；放不进 usize 的平台上当永远数不满。
    let nth = usize::try_from(rate.turns.get() - 1).unwrap_or(usize::MAX);
    let (&earliest, &since) = (recent.first()?, recent.get(nth)?);
    Some(Full {
        since,
        frees: earliest.unix_millis().saturating_add(rate.window),
    })
}

/// 额度满了没有（「怎么走」第 7 条）：满了的这段时间，线路规程不抽样、不调判官（`18-通讯平台.md` 第六节）。不看发的人。
pub fn rate_full(ctx: &Ctx, clock: Clock) -> bool {
    full(ctx, clock.now).is_some()
}

/// 限流这条规则。
pub(super) struct Rule;

impl InboundRule for Rule {
    fn judge(&self, msg: &Inbound, ctx: &Ctx, clock: Clock) -> Step {
        // 主人、自己人开的回合本来就不在 `turns` 里，他们也不受限。
        if msg.said.standing != Standing::Member {
            return Step::Continue;
        }
        let Some(full) = full(ctx, clock.now) else {
            return Step::Continue;
        };
        let noticed = ctx.notices.iter().any(|&notice| notice >= full.since);
        match msg.said.addressed && !noticed {
            true => Step::Stop(Outcome::Notice(Why::RateLimited)),
            false => Step::Stop(Outcome::RecordOnly(Why::RateLimited)),
        }
    }
}

#[cfg(test)]
mod tests;
