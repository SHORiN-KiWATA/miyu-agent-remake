//! 限流（`chat.md` 第二条「守着它的」）：边界（正好 `window` 以前的不算、`now` 那一刻的算、以后的不算）、不冲她来的、
//! 第一次满提示、再来只记下、降下去再满又提示、不限、主人和自己人；`rate_full`。

use crate::VenueKind;

use super::super::test_support::{PEOPLE, SECOND, at, ctx, judge, member, msg, rate};
use super::super::{Clock, Ctx, Inbound, Outcome, Why, rate_full};

/// 此刻：那一天 UTC 12:00。
fn now() -> Clock {
    at(12, 0)
}

/// 限流 `text`，最近的回合在此刻之前 `ago` 毫秒，提示过的在此刻之前 `noticed` 毫秒。
fn limited(text: &str, ago: &[i64], noticed: &[i64]) -> Ctx {
    let mut limited = ctx();
    limited.rate = rate(text);
    limited.turns = ago.iter().map(|ago| now().now - ago).collect();
    limited.notices = noticed.iter().map(|ago| now().now - ago).collect();
    limited
}

/// 群里别的人冲她来的一条消息。
fn addressed() -> Inbound {
    let mut addressed = member();
    addressed.addressed = true;
    addressed
}

fn outcome(msg: &Inbound, ctx: &Ctx) -> Outcome {
    judge(msg, ctx, now()).outcome
}

const LIMITED: Outcome = Outcome::RecordOnly(Why::RateLimited);
const NOTICE: Outcome = Outcome::Notice(Why::RateLimited);

#[test]
fn window_is_open_on_the_far_side() {
    // 正好 60 秒以前的不算，差一毫秒的算。
    let edge = limited("3/60s", &[60 * SECOND, SECOND, 0], &[]);
    assert_eq!(outcome(&member(), &edge), Outcome::Pass);
    assert!(!rate_full(&edge, now()));
    let inside = limited("3/60s", &[60 * SECOND - 1, SECOND, 0], &[]);
    assert_eq!(outcome(&member(), &inside), LIMITED);
    assert!(rate_full(&inside, now()));
}

#[test]
fn window_is_closed_on_the_near_side() {
    // 此刻那一回合算，以后的不算。
    assert!(rate_full(&limited("2/60s", &[SECOND, 0], &[]), now()));
    assert!(!rate_full(&limited("2/60s", &[SECOND, -1], &[]), now()));
}

#[test]
fn fewer_turns_than_the_limit_pass() {
    let one_short = limited("3/60s", &[2 * SECOND, SECOND], &[]);
    assert_eq!(outcome(&addressed(), &one_short), Outcome::Pass);
    assert!(!rate_full(&one_short, now()));
}

#[test]
fn turns_may_come_in_any_order() {
    let shuffled = limited("2/60s", &[SECOND, 90 * SECOND, 30 * SECOND], &[]);
    assert!(rate_full(&shuffled, now()));
    assert_eq!(outcome(&addressed(), &shuffled), NOTICE);
}

#[test]
fn not_addressed_is_only_recorded() {
    let full = limited("1/60s", &[0], &[]);
    assert_eq!(outcome(&member(), &full), LIMITED);
    let mut private = msg(crate::Standing::Member, VenueKind::Private);
    private.addressed = false;
    assert_eq!(outcome(&private, &full), LIMITED);
}

#[test]
fn first_time_full_gives_a_notice_then_records() {
    let full = limited("2/60s", &[20 * SECOND, 10 * SECOND], &[]);
    assert_eq!(outcome(&addressed(), &full), NOTICE);
    // 提示过以后再来，只记下。
    let noticed = limited("2/60s", &[20 * SECOND, 10 * SECOND], &[5 * SECOND]);
    assert_eq!(outcome(&addressed(), &noticed), LIMITED);
}

#[test]
fn this_round_starts_at_the_turn_that_filled_it() {
    // 窗口里四个回合，额度 2：这一回从窗口里第 2 个（从早往晚数）回合算起，也就是 40 秒以前那个。
    let ago = [50 * SECOND, 40 * SECOND, 30 * SECOND, 20 * SECOND];
    // 正好在那一刻提示的，算这一回的。
    assert_eq!(
        outcome(&addressed(), &limited("2/60s", &ago, &[40 * SECOND])),
        LIMITED
    );
    // 在它之后、更晚的回合之前提示的，也算这一回的。
    assert_eq!(
        outcome(&addressed(), &limited("2/60s", &ago, &[35 * SECOND])),
        LIMITED
    );
    // 早一毫秒、在最早的回合之后提示的，是上一回的。
    let earlier = [40 * SECOND + 1];
    assert_eq!(
        outcome(&addressed(), &limited("2/60s", &ago, &earlier)),
        NOTICE
    );
    assert_eq!(
        outcome(&addressed(), &limited("2/60s", &ago, &[45 * SECOND])),
        NOTICE
    );
}

#[test]
fn full_again_after_dropping_gives_another_notice() {
    // 额度 3/145s。三个回合在 150、140、130 秒以前开，130 秒以前满了，129 秒以前提示过。5 秒以前最早那个出了窗口，
    // 降到两个；此刻又开了一个，又满了：新的一回，再提示一次。
    let ago = [150 * SECOND, 140 * SECOND, 130 * SECOND, 0];
    let mut again = limited("3/145s", &ago, &[129 * SECOND]);
    assert_eq!(outcome(&addressed(), &again), NOTICE);
    // 这一回提示过以后，只记下。
    again.notices.push(now().now);
    assert_eq!(outcome(&addressed(), &again), LIMITED);
    // 降下去之前（10 秒以前）看：还是上一回，提示过了。
    let before = Clock {
        now: now().now - 10 * SECOND,
        offset: 0,
    };
    let mut earlier = limited("3/145s", &ago[..3], &[129 * SECOND]);
    assert!(rate_full(&earlier, before));
    assert_eq!(judge(&addressed(), &earlier, before).outcome, LIMITED);
    earlier.notices.clear();
    assert_eq!(judge(&addressed(), &earlier, before).outcome, NOTICE);
}

#[test]
fn notice_in_the_future_still_counts() {
    // 提示的时刻比此刻还晚（外面的钟拨过）：不早于这一回开始，照提示过算。
    let full = limited("1/60s", &[SECOND], &[-SECOND]);
    assert_eq!(outcome(&addressed(), &full), LIMITED);
}

#[test]
fn unlimited_never_fills() {
    let mut open = ctx();
    open.turns = vec![now().now; 50];
    assert_eq!(outcome(&addressed(), &open), Outcome::Pass);
    assert!(!rate_full(&open, now()));
    open.rate = crate::Rate::read("0");
    assert_eq!(outcome(&addressed(), &open), Outcome::Pass);
    assert!(!rate_full(&open, now()));
}

#[test]
fn owner_and_trusted_are_not_limited() {
    let full = limited("1/60s", &[0], &[]);
    let expected = [Outcome::Pass, Outcome::Pass, Outcome::Pass, NOTICE, NOTICE];
    for ((standing, kind), expected) in PEOPLE.into_iter().zip(expected) {
        let mut sent = msg(standing, kind);
        sent.addressed = true;
        assert_eq!(outcome(&sent, &full), expected, "{standing:?} {kind:?}");
    }
}

#[test]
fn rate_full_does_not_look_at_the_sender() {
    // 只看情形和此刻：满了就是满了。
    let full = limited("1/60s", &[0], &[]);
    assert!(rate_full(&full, now()));
    assert!(!rate_full(&ctx(), now()));
}
