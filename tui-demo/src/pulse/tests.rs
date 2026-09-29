//! 运行状态行的词（蓝图 `tui.md`「运行状态行和排队的消息」第 1 条）：一阵事件忙完再换、没停够等停够、
//! 没有事件到点也换、按这一轮跑了多久分档、新的一轮从头挑。

use std::time::{Duration, Instant};

use super::{Pulse, Tier, Words};

/// 范围的两头一样：测试里停多久是定的。
fn words() -> Words {
    Words {
        dwell_ms: [12_000, 12_000],
        quiet_ms: 2000,
        idle_ms: [30_000, 30_000],
        tiers: vec![
            Tier {
                after: 0,
                words: vec!["甲".into(), "乙".into(), "丙".into()],
            },
            Tier {
                after: 60,
                words: vec!["久一".into(), "久二".into()],
            },
        ],
    }
}

fn at(t0: Instant, ms: u64) -> Instant {
    t0 + Duration::from_millis(ms)
}

#[test]
fn a_burst_of_events_changes_the_word_once_it_is_quiet_and_the_word_has_stayed() {
    let w = words();
    let mut p = Pulse::new(7);
    let t0 = Instant::now();
    let first = p.word(t0, 0, t0, &w).to_string();
    // 一阵事件：0.3 秒到 3 秒之间接连来了好几件。
    for (ms, beat) in [(300, 1), (900, 2), (1500, 3), (3000, 4)] {
        assert_eq!(p.word(t0, beat, at(t0, ms), &w), first, "事件还在来，不换");
    }
    // 安静了 2 秒，这一阵完了；但这个词才停了 5 秒，没停够 12 秒：等。
    assert_eq!(p.word(t0, 4, at(t0, 5000), &w), first);
    // 停够了：换，而且不是刚才那个。
    let next = p.word(t0, 4, at(t0, 12_000), &w).to_string();
    assert_ne!(next, first);
    // 没有新事件、没到 30 秒：不换。
    assert_eq!(p.word(t0, 4, at(t0, 25_000), &w), next);
}

#[test]
fn events_that_never_stop_do_not_flicker_the_word() {
    let w = words();
    let mut p = Pulse::new(7);
    let t0 = Instant::now();
    let first = p.word(t0, 0, t0, &w).to_string();
    // 每 500 毫秒一件事，一直不安静：到 29 秒都不换（还没到 30 秒的兜底）。
    for n in 1..58u64 {
        let beat = usize::try_from(n).unwrap();
        assert_eq!(p.word(t0, beat, at(t0, n * 500), &w), first);
    }
    // 30 秒到了：兜底换一个。
    assert_ne!(p.word(t0, 60, at(t0, 30_000), &w), first);
}

#[test]
fn the_tier_follows_the_turn_and_a_new_turn_starts_over() {
    let w = words();
    let mut p = Pulse::new(7);
    let t0 = Instant::now();
    p.word(t0, 0, t0, &w);
    // 跑过 60 秒：换到下一档（停够了）。
    let late = p.word(t0, 0, at(t0, 61_000), &w).to_string();
    assert!(["久一", "久二"].contains(&late.as_str()));
    let t1 = at(t0, 70_000);
    let fresh = p.word(t1, 0, t1, &w).to_string();
    assert!(
        ["甲", "乙", "丙"].contains(&fresh.as_str()),
        "新的一轮回到第一档"
    );
    assert_eq!(w.widest(), 4, "两个汉字四列");
}
