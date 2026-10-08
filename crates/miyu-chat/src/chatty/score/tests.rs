//! 算分（`chat.md` 第三条「守着它的」）：18 第七节的两个例子；`adjust` 正负和 0；分超过 10；权重全 0；`total`、`threshold`
//! 不低于 0；违规的门槛；只查违规的不打分。

use super::super::test_support::{OTHER, cents, chatty, facts, hits, mills, now, reply};
use super::super::{Chatty, Conditions, Hit, Kind, Reply};
use super::{Judgement, Score, score};

/// 例子里的判官：五维 6/5/4/6/3，该回，不是在跟她说话，没查违规。
fn judged() -> Judgement {
    Judgement {
        scores: [6.0, 5.0, 4.0, 6.0, 3.0],
        should_reply: true,
        to_bot: false,
        severity: None,
        reason: String::new(),
    }
}

/// 她此刻刚回了别人两轮：刚说过话成立，近期发言量 p = 2。
fn twice() -> Vec<Reply> {
    vec![reply(0, &[OTHER]), reply(0, &[OTHER])]
}

fn scored(
    judgement: &Judgement,
    conditions: &Conditions,
    replies: &[Reply],
    chatty: &Chatty,
) -> Score {
    score(judgement, conditions, replies, now(), chatty)
}

/// 只有一个加 0 分的抽样：冷静不抬（没有回复），门槛就是 `base`。
fn sampled() -> Conditions {
    Conditions {
        hits: vec![Hit {
            kind: Kind::Probability,
            bonus: 0.0,
        }],
    }
}

#[test]
fn example_chiming_in_is_held_back() {
    // 18 第七节：刚说过话时有人接一句，raw 0.485，加 0.2、0.1 成 0.785；她最近连说了两句，门槛约 0.92，不回。
    let replies = twice();
    let conditions = hits(&facts(), &[], &replies, &chatty());
    assert_eq!(conditions.primary(), Some(Kind::AfterSpeaking));
    let got = scored(&judged(), &conditions, &replies, &chatty());
    assert_eq!(mills(got.raw), 485);
    assert_eq!(mills(got.adjust), 200);
    assert_eq!(mills(got.bonus), 100);
    assert_eq!(mills(got.total), 785);
    assert_eq!(cents(got.lift), 12);
    assert_eq!(cents(got.threshold), 92);
    assert!(!got.reply);
}

#[test]
fn example_addressed_goes_through() {
    // 同一句话 @ 了她：再加 0.3 成 1.085，免冷静，门槛 0.8，回。
    let replies = twice();
    let mut msg = facts();
    msg.said.addressed = true;
    let conditions = hits(&msg, &[], &replies, &chatty());
    let got = scored(&judged(), &conditions, &replies, &chatty());
    assert_eq!(mills(got.total), 1085);
    assert_eq!(mills(got.lift), 0);
    assert_eq!(mills(got.threshold), 800);
    assert!(got.reply);
}

#[test]
fn adjust_follows_should_reply() {
    let mut no = judged();
    no.should_reply = false;
    let got = scored(&no, &sampled(), &[], &chatty());
    assert_eq!(mills(got.adjust), -200);
    assert_eq!(mills(got.total), 285);
    // `adjust` 是 0 就是关了：该不该回都不动。
    let off = Chatty {
        adjust: 0.0,
        ..chatty()
    };
    for should_reply in [true, false] {
        let judgement = Judgement {
            should_reply,
            ..judged()
        };
        let got = scored(&judgement, &sampled(), &[], &off);
        assert_eq!(mills(got.adjust), 0);
        assert_eq!(mills(got.total), 485);
    }
}

#[test]
fn bonus_is_the_sum_of_hits() {
    let conditions = Conditions {
        hits: vec![
            Hit {
                kind: Kind::Direct,
                bonus: 0.3,
            },
            Hit {
                kind: Kind::Continuation,
                bonus: 0.25,
            },
        ],
    };
    let got = scored(&judged(), &conditions, &[], &chatty());
    assert_eq!(mills(got.bonus), 550);
    assert_eq!(mills(got.total), 1235);
}

#[test]
fn scores_above_ten_count_as_ten() {
    let judgement = Judgement {
        scores: [20.0, 10.0, 11.0, 10.0, 99.0],
        ..judged()
    };
    let got = scored(&judgement, &sampled(), &[], &chatty());
    assert_eq!(mills(got.raw), 1000);
}

#[test]
fn raw_weighs_each_dimension() {
    // 只有一维有分，raw 就是那一维的权重占比乘分的十分之一：五维各自换个位置也算得对。
    for (dimension, weight) in [0.25, 0.25, 0.15, 0.15, 0.20].into_iter().enumerate() {
        let mut scores = [0.0; 5];
        scores[dimension] = 10.0;
        let judgement = Judgement { scores, ..judged() };
        let got = scored(&judgement, &sampled(), &[], &chatty());
        assert_eq!(mills(got.raw), mills(weight), "{dimension}");
    }
}

#[test]
fn zero_weights_give_zero_raw() {
    let flat = Chatty {
        weights: [0.0; 5],
        ..chatty()
    };
    let got = scored(&judged(), &sampled(), &[], &flat);
    assert_eq!(got.raw, 0.0);
    assert_eq!(mills(got.total), 200);
}

#[test]
fn total_and_threshold_never_go_below_zero() {
    let judgement = Judgement {
        scores: [0.0; 5],
        should_reply: false,
        ..judged()
    };
    let got = scored(&judgement, &sampled(), &[], &chatty());
    assert_eq!(got.total, 0.0);
    assert!(!got.reply);
    let low = Chatty {
        base: -1.0,
        ..chatty()
    };
    let got = scored(&judgement, &sampled(), &[], &low);
    assert_eq!(got.threshold, 0.0);
    assert!(got.reply, "0 不低于 0");
}

#[test]
fn severity_overrides_the_score() {
    let quiet = Judgement {
        scores: [0.0; 5],
        should_reply: false,
        ..judged()
    };
    for (severity, reply) in [
        (None, false),
        (Some(0), false),
        (Some(6), false),
        (Some(7), true),
        (Some(10), true),
    ] {
        let judgement = Judgement {
            severity,
            ..quiet.clone()
        };
        let got = scored(&judgement, &sampled(), &[], &chatty());
        assert_eq!(got.reply, reply, "{severity:?}");
        assert_eq!(mills(got.threshold), 800, "违规不动门槛");
    }
}

#[test]
fn moderation_only_is_not_scored() {
    let flagged = Conditions {
        hits: vec![Hit {
            kind: Kind::Moderation,
            bonus: 0.0,
        }],
    };
    let eager = Judgement {
        scores: [10.0; 5],
        ..judged()
    };
    let got = scored(&eager, &flagged, &twice(), &chatty());
    let zero = Score {
        raw: 0.0,
        adjust: 0.0,
        bonus: 0.0,
        lift: 0.0,
        threshold: 0.0,
        total: 0.0,
        reply: false,
    };
    assert_eq!(got, zero, "满分也不回：只看违规");
    let violation = Judgement {
        severity: Some(7),
        ..judged()
    };
    let got = scored(&violation, &flagged, &twice(), &chatty());
    assert_eq!(
        got,
        Score {
            reply: true,
            ..zero
        }
    );
    // 违规旗和别的条件一起成立的照常打分。
    let mut both = flagged.clone();
    both.hits.insert(
        0,
        Hit {
            kind: Kind::AfterSpeaking,
            bonus: 0.1,
        },
    );
    let got = scored(&eager, &both, &[], &chatty());
    assert_eq!(mills(got.total), 1300);
    assert!(got.reply);
}
