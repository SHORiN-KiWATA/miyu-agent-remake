//! 睡眠（`docs/blueprint/chat.md` 第二条「怎么走」第 2 条，`18-通讯平台.md` 第五节、第六节）：此刻照时区换成当地时间，
//! 落在睡眠时间里算睡着。睡着时只放行主人和私聊里的自己人，别的只记下：不参与主动插话、不占限流额度、也不回话。
//! 回合闸要的醒来时刻也在这里算（第 8 条）。

use super::{Clock, Ctx, Inbound, InboundRule, Outcome, Step, Why};

/// 一分钟的毫秒数。
const MINUTE: i64 = 60_000;

/// 一天的分钟数。
const DAY: i64 = 24 * 60;

/// 睡眠时间：只能从场所规则 `sleep` 的原文读（[`Sleep::read`]），格不公开：读出来的开始、结束都在一天里，不会相等
/// （施工时定的第 13 条；相等的话回合闸会推迟到已经过去的时刻，桥空转一分钟）。按场所会话的时区算，时区在
/// [`Clock::offset`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sleep {
    /// 一天里的第几分钟开始睡，`0` 到 `1439`；这一分钟算睡着。
    pub(crate) start: u16,
    /// 一天里的第几分钟醒，`0` 到 `1439`；这一分钟算醒着。比 `start` 早的是跨午夜；不等于 `start`。
    pub(crate) end: u16,
}

impl Sleep {
    /// 当地时间一天里的第 `minute` 分钟睡着没有：落在 `[start, end)` 里；跨午夜的落在 `[start, 1440)` 或 `[0, end)` 里。
    fn covers(self, minute: i64) -> bool {
        let (start, end) = (i64::from(self.start), i64::from(self.end));
        match start < end {
            true => start <= minute && minute < end,
            false => minute >= start || minute < end,
        }
    }
}

/// 睡眠这条规则。
pub(super) struct Rule;

impl InboundRule for Rule {
    fn judge(&self, msg: &Inbound, ctx: &Ctx, clock: Clock) -> Step {
        match wakes(ctx, clock).is_some() && !msg.excused() {
            true => Step::Stop(Outcome::RecordOnly(Why::Asleep)),
            false => Step::Continue,
        }
    }
}

/// 睡着的话，醒来的时刻（自 Unix 纪元起的毫秒）：当地时间下一次到 `end` 那一分钟的开头。醒着、没设睡眠是 `None`。
///
/// 内核的时刻只交得出写成字的当地钟点（`Timestamp::local_minute`），这里照它的公开方法（毫秒数、时区的分钟数）自己算
/// 一天里的第几分钟（施工 O-12）。时区差是整分钟，先把此刻落到 UTC 的整分钟再加时区差，和先换成当地时间再落到整分钟是
/// 一回事；纪元以前的负数用 `div_euclid` 往下取，不往零取。
pub(super) fn wakes(ctx: &Ctx, clock: Clock) -> Option<i64> {
    let sleep = ctx.sleep?;
    let minutes = clock.now.unix_millis().div_euclid(MINUTE);
    let minute = (minutes + i64::from(clock.offset.minutes())).rem_euclid(DAY);
    if !sleep.covers(minute) {
        return None;
    }
    // 睡着时此刻不在 `end` 那一分钟里，还有 1 到 1439 分钟醒。时刻只到 9999 年，乘加都不会溢出。
    let ahead = (i64::from(sleep.end) - minute).rem_euclid(DAY);
    Some((minutes + ahead) * MINUTE)
}
