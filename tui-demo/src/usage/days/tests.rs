use jiff::civil::date;

use super::{Day, days, grid, level, peak, streaks, thresholds};
use crate::core::{Bill, Cost, UsageRow};

fn row(day: &str, tokens: u64, usd: f64) -> UsageRow {
    UsageRow {
        day: Some(day.into()),
        tokens,
        requests: 1,
        bill: Bill {
            amounts: vec![Cost {
                amount: usd,
                currency: "USD".into(),
            }],
            unpriced: 0,
        },
        ..UsageRow::default()
    }
}

fn day(d: jiff::civil::Date, tokens: u64) -> Day {
    Day {
        date: d,
        tokens,
        amount: 0.0,
    }
}

#[test]
fn rows_read_into_sorted_days_with_the_chosen_currency() {
    let got = days(
        &[
            row("2026-10-07", 5, 0.5),
            row("2026-10-05", 9, 0.25),
            row("坏的", 1, 0.0),
        ],
        "USD",
    );
    assert_eq!(got.len(), 2, "认不出的日期不要");
    assert_eq!(got[0].date, date(2026, 10, 5));
    assert!((got[1].amount - 0.5).abs() < 1e-12);
    assert_eq!(days(&[row("2026-10-07", 5, 0.5)], "CNY")[0].amount, 0.0);
    assert_eq!(peak(&got).map(|d| d.tokens), Some(9));
    assert!(peak(&[]).is_none());
}

#[test]
fn the_current_streak_counts_back_from_today_or_yesterday_and_the_longest_over_all_time() {
    let today = date(2026, 10, 7);
    let used = [
        day(date(2026, 9, 1), 1),
        day(date(2026, 9, 2), 1),
        day(date(2026, 9, 3), 1),
        day(date(2026, 9, 4), 0),
        day(date(2026, 10, 5), 1),
        day(date(2026, 10, 6), 1),
    ];
    assert_eq!(
        streaks(&used, today),
        (2, 3),
        "今天没用的从昨天往回数；0 的不算用了"
    );
    let mut with_today = used.to_vec();
    with_today.push(day(today, 1));
    assert_eq!(streaks(&with_today, today), (3, 3));
    assert_eq!(streaks(&used[..3], today), (0, 3), "断了的现在是 0");
    assert_eq!(streaks(&[], today), (0, 0));
}

#[test]
fn the_grid_ends_with_this_week_mondays_on_top_and_months_at_their_first_week() {
    // 2026-10-07 是周三。
    let today = date(2026, 10, 7);
    let used = [
        day(date(2026, 10, 5), 10),
        day(today, 30),
        day(date(2026, 9, 28), 20),
    ];
    let g = grid(&used, today, 3, |d| d.tokens as f64);
    assert_eq!(g.weeks.len(), 3);
    let this = g.weeks[2];
    assert_eq!(this[0], Some(10.0), "周一 10-05");
    assert_eq!(this[2], Some(30.0), "周三是今天");
    assert_eq!(this[3], None, "今天以后不画");
    assert_eq!(g.weeks[1][0], Some(20.0), "上一列的周一 09-28");
    assert_eq!(g.weeks[0][0], Some(0.0), "没用的是 0");
    // 第一列（09-21 那周）标 9 月，10-05 那一列是 10 月的头一周。
    assert_eq!(g.months, vec![(0, 9), (2, 10)]);
}

#[test]
fn levels_follow_the_quartiles_of_the_days_that_were_used() {
    let t = thresholds([0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0].into_iter());
    assert_eq!(t, vec![2.0, 4.0, 6.0]);
    assert_eq!(level(0.0, &t), 0);
    assert_eq!(level(1.0, &t), 1);
    assert_eq!(level(3.0, &t), 2);
    assert_eq!(level(5.0, &t), 3);
    assert_eq!(level(8.0, &t), 4);
    assert!(thresholds([0.0].into_iter()).is_empty());
    assert_eq!(level(5.0, &[]), 1, "只有一种值的都是一档");
}
