//! 冷静（`chat.md` 第三条「守着它的」）：p 从 0 到 8 的曲线，各到两位小数；冲她来、`to_bot` 不抬；关着的不抬；晚于此刻的
//! 回复不算；近期发言量照半衰期衰减。

use super::super::test_support::{MINUTE, OTHER, cents, chatty, mills, now, reply};
use super::super::{Chatty, Conditions, Hit, Judgement, Kind, Lift, LiftCtx, Reply, Restraint};
use super::{Item, pressure};

/// 一次插嘴：只有刚说过话，判官说不是在跟她说话。
fn chiming() -> (Judgement, Conditions) {
    let judgement = Judgement {
        scores: [5.0; 5],
        should_reply: true,
        to_bot: false,
        severity: None,
        reason: String::new(),
    };
    let conditions = Conditions {
        hits: vec![Hit {
            kind: Kind::AfterSpeaking,
            bonus: 0.1,
        }],
    };
    (judgement, conditions)
}

/// 冷静抬多少。
fn lift(judgement: &Judgement, conditions: &Conditions, replies: &[Reply], chatty: &Chatty) -> f64 {
    let ctx = LiftCtx {
        judgement,
        conditions,
        replies,
        clock: now(),
        chatty,
    };
    Item.lift(&ctx)
}

/// 此刻刚发出的 `count` 轮：近期发言量正好是 `count`。
fn fresh(count: usize) -> Vec<Reply> {
    vec![reply(0, Some(OTHER)); count]
}

#[test]
fn curve() {
    // 18 第七节：p=1 抬 0.02，p=2 抬 0.12，p=3 抬 0.22，p=4 抬 0.28，p=5 抬 0.31，p=8 抬 0.34。
    let (judgement, conditions) = chiming();
    let curve = [
        (0, 0),
        (1, 2),
        (2, 12),
        (3, 22),
        (4, 28),
        (5, 31),
        (6, 33),
        (7, 33),
        (8, 34),
    ];
    for (p, expected) in curve {
        let replies = fresh(p);
        assert_eq!(mills(pressure(&replies, now(), &chatty())), 1000 * p as i64);
        let got = lift(&judgement, &conditions, &replies, &chatty());
        assert_eq!(cents(got), expected, "p={p}");
    }
    // 再多也不超过 cap。
    let many = lift(&judgement, &conditions, &fresh(200), &chatty());
    assert!(many < 0.35 && cents(many) == 35, "{many}");
}

#[test]
fn pressure_halves_every_half_life() {
    let params = chatty();
    let at = |ago: &[i64]| -> i64 {
        let replies: Vec<Reply> = ago.iter().map(|&ago| reply(ago, None)).collect();
        mills(pressure(&replies, now(), &params))
    };
    assert_eq!(at(&[]), 0);
    assert_eq!(at(&[0]), 1000);
    assert_eq!(at(&[3 * MINUTE]), 500);
    assert_eq!(at(&[6 * MINUTE]), 250);
    assert_eq!(at(&[90 * 1000]), 707);
    assert_eq!(at(&[0, 3 * MINUTE, 6 * MINUTE]), 1750);
    // 晚于此刻的不算，差一毫秒也不算。
    assert_eq!(at(&[-1]), 0);
    assert_eq!(at(&[-MINUTE, 0]), 1000);
    // 半衰期是参数。
    let slow = Chatty {
        restraint: Restraint {
            half_life: 6 * MINUTE,
            ..params.restraint
        },
        ..chatty()
    };
    let replies = [reply(6 * MINUTE, None)];
    assert_eq!(mills(pressure(&replies, now(), &slow)), 500);
}

#[test]
fn future_replies_do_not_lift() {
    let (judgement, conditions) = chiming();
    let later: Vec<Reply> = vec![reply(-1, None); 5];
    assert_eq!(lift(&judgement, &conditions, &later, &chatty()), 0.0);
}

#[test]
fn addressed_or_to_bot_is_not_restrained() {
    let replies = fresh(5);
    let (judgement, mut conditions) = chiming();
    conditions.hits.insert(
        0,
        Hit {
            kind: Kind::Direct,
            bonus: 0.3,
        },
    );
    assert_eq!(
        lift(&judgement, &conditions, &replies, &chatty()),
        0.0,
        "冲她来"
    );
    let (mut judgement, conditions) = chiming();
    judgement.to_bot = true;
    assert_eq!(
        lift(&judgement, &conditions, &replies, &chatty()),
        0.0,
        "to_bot"
    );
    // 别的条件不豁免。
    let (judgement, conditions) = chiming();
    assert_eq!(
        cents(lift(&judgement, &conditions, &replies, &chatty())),
        31
    );
}

#[test]
fn switched_off_does_not_lift() {
    let off = Chatty {
        restraint: Restraint {
            on: false,
            ..chatty().restraint
        },
        ..chatty()
    };
    let (judgement, conditions) = chiming();
    assert_eq!(lift(&judgement, &conditions, &fresh(8), &off), 0.0);
}

#[test]
fn curve_follows_cap_and_k() {
    // cap、k 是参数：p = k 时正好抬 cap 的一半。
    let (judgement, conditions) = chiming();
    let tuned = Chatty {
        restraint: Restraint {
            cap: 0.5,
            k: 4.0,
            ..chatty().restraint
        },
        ..chatty()
    };
    assert_eq!(mills(lift(&judgement, &conditions, &fresh(4), &tuned)), 250);
}
