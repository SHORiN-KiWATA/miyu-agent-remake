//! 回合闸（`chat.md` 第二条「守着它的」）：只睡着、只满了、两样都占取晚的、跨午夜醒来的时刻、时区差、都不占是 `Now`。

use super::super::Ctx;
use super::super::test_support::{DAY0, MINUTE, SECOND, at, clock, ctx, rate, sleep, time, utc};
use super::{Gate, gate};

/// 那一天 UTC `hour:minute` 整的毫秒。
fn utc_ms(hour: i64, minute: i64) -> i64 {
    DAY0 + (hour * 60 + minute) * MINUTE
}

/// 睡在 `span` 的情形。
fn sleeping(span: &str) -> Ctx {
    let mut night = ctx();
    night.sleep = sleep(span);
    night
}

#[test]
fn nothing_set_opens_now() {
    assert_eq!(gate(&ctx(), at(12, 0)), Gate::Now);
}

#[test]
fn awake_and_not_full_opens_now() {
    let mut day = sleeping("23:00-07:00");
    day.rate = rate("2/60s");
    day.turns = vec![at(12, 0).now];
    assert_eq!(gate(&day, at(12, 0)), Gate::Now);
    assert_eq!(gate(&day, at(7, 0)), Gate::Now);
}

#[test]
fn asleep_waits_for_the_start_of_the_waking_minute() {
    // 同一天里：13:15:30 睡着，14:00 整醒。
    let mid = clock(utc_ms(13, 15) + 30 * SECOND, 0);
    assert_eq!(
        gate(&sleeping("13:00-14:00"), mid),
        Gate::Later(time(utc_ms(14, 0)))
    );
    // 正好在 start：还是等到 end。
    assert_eq!(
        gate(&sleeping("13:00-14:00"), at(13, 0)),
        Gate::Later(time(utc_ms(14, 0)))
    );
    // end 前最后一毫秒：等到下一毫秒。
    let last = clock(utc_ms(14, 0) - 1, 0);
    assert_eq!(
        gate(&sleeping("13:00-14:00"), last),
        Gate::Later(time(utc_ms(14, 0)))
    );
}

#[test]
fn asleep_across_midnight_wakes_the_right_day() {
    let night = sleeping("23:00-07:00");
    // 23:30 睡着，第二天 07:00 醒。
    assert_eq!(
        gate(&night, at(23, 30)),
        Gate::Later(time(utc_ms(24 + 7, 0)))
    );
    // 02:00 睡着，同一天 07:00 醒。
    assert_eq!(gate(&night, at(2, 0)), Gate::Later(time(utc_ms(7, 0))));
    // 睡到午夜的：23:59 睡着，下一分钟醒。
    assert_eq!(
        gate(&sleeping("23:59-00:00"), at(23, 59)),
        Gate::Later(time(utc_ms(24, 0)))
    );
}

#[test]
fn asleep_wakes_at_local_time() {
    let night = sleeping("23:00-07:00");
    // 北京（+8）：UTC 15:30:42.123 是当地 23:30:42.123，当地第二天 07:00 是 UTC 23:00。
    let east = clock(utc_ms(15, 30) + 42 * SECOND + 123, 480);
    assert_eq!(gate(&night, east), Gate::Later(time(utc_ms(23, 0))));
    // 纽约（-5）：UTC 05:00 是当地 00:00，当地 07:00 是 UTC 12:00。
    assert_eq!(
        gate(&night, utc(5, 0, -300)),
        Gate::Later(time(utc_ms(12, 0)))
    );
    // +5:45：UTC 17:15 是当地 23:00，当地第二天 07:00 是 UTC 01:15。
    assert_eq!(
        gate(&night, utc(17, 15, 345)),
        Gate::Later(time(utc_ms(24 + 1, 15)))
    );
}

#[test]
fn asleep_before_the_epoch() {
    // 纪元前一毫秒是 23:59:59.999，睡到 00:00 的话，下一毫秒就醒。
    let before = clock(-1, 0);
    assert_eq!(gate(&sleeping("23:00-00:00"), before), Gate::Later(time(0)));
}

#[test]
fn full_waits_until_the_earliest_turn_leaves_the_window() {
    let mut full = ctx();
    full.rate = rate("2/60s");
    let now = at(12, 0).now.unix_millis();
    full.turns = vec![time(now - 10 * SECOND), time(now - 50 * SECOND)];
    assert_eq!(gate(&full, at(12, 0)), Gate::Later(time(now + 10 * SECOND)));
    // 窗口里的比额度多：还是最早那个。
    full.turns.push(time(now - 30 * SECOND));
    assert_eq!(gate(&full, at(12, 0)), Gate::Later(time(now + 10 * SECOND)));
    // 窗口外的不算。
    full.turns.push(time(now - 60 * SECOND));
    assert_eq!(gate(&full, at(12, 0)), Gate::Later(time(now + 10 * SECOND)));
}

#[test]
fn both_take_the_later() {
    let now = at(12, 0).now.unix_millis();
    // 睡到 12:01，额度 12:00:10 空出来：等到 12:01。
    let mut both = sleeping("11:00-12:01");
    both.rate = rate("1/60s");
    both.turns = vec![time(now - 50 * SECOND)];
    assert_eq!(gate(&both, at(12, 0)), Gate::Later(time(utc_ms(12, 1))));
    // 额度 1/90s，12:01:20 空出来，比醒得晚：等到 12:01:20。
    both.rate = rate("1/90s");
    both.turns = vec![time(now - 10 * SECOND)];
    assert_eq!(gate(&both, at(12, 0)), Gate::Later(time(now + 80 * SECOND)));
}

#[test]
fn past_the_last_moment_waits_at_now() {
    // 9999-12-31T23:30Z 睡着，醒来的时刻在 10000 年，内核的时刻写不出：推迟到此刻，闸照样不放。
    let last = miyu_kernel::time::Timestamp::parse("9999-12-31T23:30:00.000Z").expect("合写法");
    let now = clock(last.unix_millis(), 0);
    assert_eq!(gate(&sleeping("23:00-07:00"), now), Gate::Later(last));
    // 额度满了、出窗口的时刻也写不出的，一样。
    let mut full = ctx();
    full.rate = rate("1/1h");
    full.turns = vec![last];
    assert_eq!(gate(&full, now), Gate::Later(last));
}
