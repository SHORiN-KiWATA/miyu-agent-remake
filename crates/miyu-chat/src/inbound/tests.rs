//! 进站链（`chat.md` 第二条「守着它的」）：先后（前面停了后面不看、旗和放行同时有、五条都过是放行）；睡眠（不跨午夜、
//! 跨午夜、正好在 `start`、正好在 `end`、时区差正的负的跨日、纪元以前、各种人）；禁言、谁能叫她（各种人、没设 `allow`）。

use crate::VenueKind;

use super::test_support::{MINUTE, PEOPLE, SECOND, at, ctx, judge, member, msg, rate, sleep, utc};
use super::{Chain, Clock, Flag, Outcome, Standing, Verdict, Why};

fn pass() -> Verdict {
    Verdict {
        outcome: Outcome::Pass,
        flags: Vec::new(),
    }
}

fn record(why: Why) -> Verdict {
    Verdict {
        outcome: Outcome::RecordOnly(why),
        flags: Vec::new(),
    }
}

#[test]
fn builtin_has_five_rules_in_order() {
    let names: Vec<_> = Chain::builtin()
        .rules
        .iter()
        .map(|rule| rule.name().to_string())
        .collect();
    assert_eq!(names, ["sleep", "muted", "allow", "moderation", "rate"]);
}

#[test]
fn nothing_set_passes() {
    for (standing, kind) in PEOPLE {
        assert_eq!(judge(&msg(standing, kind), &ctx(), at(12, 0)), pass());
    }
}

#[test]
fn earlier_stop_hides_later_rules() {
    // 睡着、被禁言、不让叫、命中关键词、限流满了，全占：睡眠先停，旗也没插。
    let mut all = ctx();
    all.sleep = sleep("11:00-13:00");
    all.muted = true;
    all.allow = Some(false);
    all.moderation.keywords = vec!["hello".to_string()];
    all.rate = rate("1/60s");
    all.turns = vec![at(12, 0).now];
    let clock = at(12, 0);
    assert_eq!(judge(&member(), &all, clock), record(Why::Asleep));
    all.sleep = None;
    assert_eq!(judge(&member(), &all, clock), record(Why::Muted));
    all.muted = false;
    assert_eq!(judge(&member(), &all, clock), record(Why::NotAllowed));
    all.allow = None;
    let limited = Verdict {
        outcome: Outcome::RecordOnly(Why::RateLimited),
        flags: vec![Flag::Moderation],
    };
    assert_eq!(judge(&member(), &all, clock), limited);
    all.rate = None;
    let flagged = Verdict {
        outcome: Outcome::Pass,
        flags: vec![Flag::Moderation],
    };
    assert_eq!(judge(&member(), &all, clock), flagged);
}

#[test]
fn owner_passes_sleep_and_allow_but_not_mute() {
    let mut all = ctx();
    all.sleep = sleep("11:00-13:00");
    all.allow = Some(false);
    let owner = msg(Standing::Owner, VenueKind::Group);
    assert_eq!(judge(&owner, &all, at(12, 0)), pass());
    all.muted = true;
    assert_eq!(judge(&owner, &all, at(12, 0)), record(Why::Muted));
}

/// 场所睡在 `span`，此刻 `clock`，群里别的人发的消息：睡着没有。
fn asleep(span: &str, clock: Clock) -> bool {
    let mut asleep = ctx();
    asleep.sleep = sleep(span);
    match judge(&member(), &asleep, clock).outcome {
        Outcome::RecordOnly(Why::Asleep) => true,
        Outcome::Pass => false,
        other => panic!("{other:?}"),
    }
}

#[test]
fn sleep_within_one_day() {
    let span = "13:00-14:00";
    assert!(!asleep(span, at(12, 59)));
    assert!(asleep(span, at(13, 0)), "正好在 start 是睡着的");
    assert!(asleep(span, at(13, 30)));
    let last = Clock {
        now: at(14, 0).now - 1,
        offset: 0,
    };
    assert!(asleep(span, last), "end 前一毫秒还睡着");
    assert!(!asleep(span, at(14, 0)), "正好在 end 是醒着的");
    assert!(!asleep(span, at(23, 0)));
    assert!(!asleep(span, at(0, 0)));
}

#[test]
fn sleep_across_midnight() {
    let span = "23:00-07:00";
    assert!(!asleep(span, at(22, 59)));
    assert!(asleep(span, at(23, 0)));
    assert!(asleep(span, at(23, 59)));
    assert!(asleep(span, at(0, 0)));
    assert!(asleep(span, at(6, 59)));
    assert!(!asleep(span, at(7, 0)));
    assert!(!asleep(span, at(12, 0)));
}

#[test]
fn sleep_edges_of_the_day() {
    assert!(asleep("00:00-23:59", at(0, 0)));
    assert!(!asleep("00:00-23:59", at(23, 59)));
    assert!(asleep("23:59-00:00", at(23, 59)));
    assert!(!asleep("23:59-00:00", at(0, 0)));
    assert!(!asleep("23:59-00:00", at(23, 58)));
}

#[test]
fn sleep_uses_local_time() {
    let span = "23:00-07:00";
    // 北京（+8）：UTC 15:00 是当地 23:00，UTC 14:59 是 22:59。
    assert!(asleep(span, utc(15, 0, 480)));
    assert!(!asleep(span, utc(14, 59, 480)));
    // 跨日：UTC 22:30 是当地第二天 06:30，UTC 23:00 是第二天 07:00。
    assert!(asleep(span, utc(22, 30, 480)));
    assert!(!asleep(span, utc(23, 0, 480)));
    // 纽约（-5）：UTC 04:00 是当地前一天 23:00，UTC 12:00 是 07:00，UTC 03:59 是前一天 22:59。
    assert!(asleep(span, utc(4, 0, -300)));
    assert!(!asleep(span, utc(12, 0, -300)));
    assert!(!asleep(span, utc(3, 59, -300)));
    // 不整小时的时区（+5:45）：UTC 17:15 是当地 23:00。
    assert!(asleep(span, utc(17, 15, 345)));
    assert!(!asleep(span, utc(17, 14, 345)));
}

#[test]
fn sleep_before_the_epoch() {
    // 纪元前一毫秒是 1969-12-31 23:59:59.999，不是 00:00。
    let before = Clock { now: -1, offset: 0 };
    assert!(asleep("23:00-07:00", before));
    assert!(!asleep("00:00-01:00", before));
    let night = Clock {
        now: -30 * MINUTE,
        offset: 0,
    };
    assert!(asleep("23:30-23:31", night));
}

#[test]
fn sleep_excuses_owner_and_trusted_in_private() {
    let mut night = ctx();
    night.sleep = sleep("23:00-07:00");
    let clock = utc(1, 0, 0);
    let expected = [
        pass(),
        pass(),
        record(Why::Asleep),
        record(Why::Asleep),
        record(Why::Asleep),
    ];
    for ((standing, kind), expected) in PEOPLE.into_iter().zip(expected) {
        assert_eq!(
            judge(&msg(standing, kind), &night, clock),
            expected,
            "{standing:?} {kind:?}"
        );
    }
}

#[test]
fn muted_records_everyone() {
    let mut muted = ctx();
    muted.muted = true;
    for (standing, kind) in PEOPLE {
        let mut addressed = msg(standing, kind);
        addressed.addressed = true;
        assert_eq!(judge(&addressed, &muted, at(12, 0)), record(Why::Muted));
    }
}

#[test]
fn not_allowed_excuses_owner_and_trusted_in_private() {
    let mut closed = ctx();
    closed.allow = Some(false);
    let expected = [
        pass(),
        pass(),
        record(Why::NotAllowed),
        record(Why::NotAllowed),
        record(Why::NotAllowed),
    ];
    for ((standing, kind), expected) in PEOPLE.into_iter().zip(expected) {
        assert_eq!(
            judge(&msg(standing, kind), &closed, at(12, 0)),
            expected,
            "{standing:?} {kind:?}"
        );
    }
}

#[test]
fn allow_unset_or_true_lets_everyone_call() {
    for allow in [None, Some(true)] {
        let mut open = ctx();
        open.allow = allow;
        for (standing, kind) in PEOPLE {
            assert_eq!(judge(&msg(standing, kind), &open, at(12, 0)), pass());
        }
    }
}

#[test]
fn awake_hours_do_not_stop_anyone() {
    let mut day = ctx();
    day.sleep = sleep("23:00-07:00");
    let mut addressed = member();
    addressed.addressed = true;
    assert_eq!(judge(&addressed, &day, utc(7, 0, 0)), pass());
    let late = Clock {
        now: utc(22, 59, 0).now + 59 * SECOND,
        offset: 0,
    };
    assert_eq!(judge(&addressed, &day, late), pass());
}
