//! `ext.onebot.chat.decided` 的 `body`（施工 O-23 下，`onebot.md` 第一条「群里怎么叫她」第 7 条那张表）：问了判官的记判官
//! 那一次（模式、几次、多久、哪个模型、读出来的回答或者为什么判不了）和算分的每一项，结论照分；额度满了没问的只记模式和
//! `rate_full`；顶替记接过的、放下的那一条；条件的写法读得回来。

use miyu_chat::{
    Conditions, Hit, Judgement, Kind, Mode, Outcome, Route, Score, Standing, Supersede, Unreadable,
    Verdict,
};
use miyu_kernel::id::Seq;
use serde_json::json;

use super::super::ask::{Answer, Unjudged};
use super::super::decide::{Conclusion, Decision, Passed};
use super::super::discipline::Discipline;
use super::{Finale, Judged, body, finale, kind_of};

/// 序号。
fn seq(n: u64) -> Seq {
    Seq::new(n).expect("不是 0")
}

/// 冲她来的一条，走 `route`、结论 `conclusion`，顶替 `supersede`。
fn decision(route: Route, conclusion: Conclusion, supersede: Supersede) -> Decision {
    Decision {
        msgs: vec![11, 12],
        verdict: Verdict {
            outcome: Outcome::Pass,
            flags: Vec::new(),
        },
        passed: Some(Passed {
            discipline: Discipline::Chatty,
            conditions: Conditions {
                hits: vec![Hit {
                    kind: Kind::Direct,
                    bonus: 0.3,
                }],
            },
            supersede,
            route,
            rate_full: false,
        }),
        conclusion,
    }
}

/// 判官说的一份。
fn judgement(severity: Option<u8>) -> Judgement {
    Judgement {
        scores: [8.0, 7.0, 5.0, 6.0, 7.0],
        should_reply: true,
        to_bot: true,
        severity,
        reason: "asked her".to_string(),
    }
}

/// 算出的分，回不回是 `reply`。
fn score(reply: bool) -> Score {
    Score {
        raw: 0.5,
        adjust: 0.2,
        bonus: 0.3,
        lift: 0.1,
        threshold: 0.9,
        total: 1.0,
        reply,
    }
}

#[test]
fn a_judged_one_writes_the_judge_and_every_part_of_the_score() {
    let asked = decision(
        Route::Judge,
        Conclusion::Judge(Mode::Reply),
        Supersede::None,
    );
    let answer = Answer {
        tries: 2,
        millis: 812,
        model: Some("judge/m".to_string()),
        result: Ok(judgement(Some(3))),
    };
    let scored = score(true);
    let judged = Judged {
        answer: &answer,
        score: Some(&scored),
    };
    assert_eq!(
        body(&asked, Standing::Member, Some(judged)),
        json!({
            "msgs": [11, 12], "standing": "member", "inbound": "pass", "discipline": "chatty",
            "conditions": [{"kind": "direct", "bonus": 0.3}], "route": "judge",
            "judge": {
                "mode": "reply", "tries": 2, "millis": 812, "model": "judge/m",
                "answer": {"scores": [8.0, 7.0, 5.0, 6.0, 7.0], "should_reply": true, "to_bot": true, "severity": 3, "reason": "asked her"},
            },
            "score": {"raw": 0.5, "adjust": 0.2, "bonus": 0.3, "lift": 0.1, "threshold": 0.9, "total": 1.0, "reply": true},
            "outcome": "reply",
        })
    );
    assert_eq!(finale(asked.conclusion, Some(judged)), Finale::Reply);
    let low = score(false);
    let declined = Judged {
        answer: &answer,
        score: Some(&low),
    };
    assert_eq!(
        body(&asked, Standing::Member, Some(declined))["outcome"],
        "record",
        "分不够只记下"
    );
    let unchecked = Answer {
        result: Ok(judgement(None)),
        ..answer.clone()
    };
    let written = body(
        &asked,
        Standing::Member,
        Some(Judged {
            answer: &unchecked,
            score: Some(&scored),
        }),
    );
    assert!(
        written["judge"]["answer"].get("severity").is_none(),
        "没查违规的不写：{written}"
    );
}

#[test]
fn an_unjudged_one_says_why_and_is_only_recorded() {
    let asked = decision(
        Route::ModerationOnly,
        Conclusion::Judge(Mode::ModerationOnly),
        Supersede::None,
    );
    let cases = [
        (Unjudged::Queue, json!({"unjudged": "queue"})),
        (Unjudged::Timeout, json!({"unjudged": "timeout"})),
        (
            Unjudged::Refused("no_model".to_string()),
            json!({"unjudged": "refused", "detail": "no_model"}),
        ),
        (
            Unjudged::Unreadable(Unreadable::NoObject),
            json!({"unjudged": "unreadable", "detail": "no_object"}),
        ),
        (
            Unjudged::Unreadable(Unreadable::NoSeverity),
            json!({"unjudged": "unreadable", "detail": "no_severity"}),
        ),
        (
            Unjudged::Unreadable(Unreadable::Dimension("timing")),
            json!({"unjudged": "unreadable", "detail": "dimension:timing"}),
        ),
    ];
    for (unjudged, expected) in cases {
        let answer = Answer {
            tries: 1,
            millis: 5,
            model: None,
            result: Err(unjudged),
        };
        let judged = Judged {
            answer: &answer,
            score: None,
        };
        let written = body(&asked, Standing::Whitelisted, Some(judged));
        let mut wanted = json!({"mode": "moderation_only", "tries": 1, "millis": 5});
        for (key, value) in expected.as_object().expect("是对象") {
            wanted[key] = value.clone();
        }
        assert_eq!(written["judge"], wanted, "{written}");
        assert!(written.get("score").is_none(), "{written}");
        assert_eq!(written["outcome"], "record", "判不了照不回算");
        assert_eq!(finale(asked.conclusion, Some(judged)), Finale::Record);
    }
}

#[test]
fn a_full_rate_writes_only_the_mode_and_why() {
    let mut skipped = decision(Route::Judge, Conclusion::Record, Supersede::None);
    if let Some(passed) = skipped.passed.as_mut() {
        passed.rate_full = true;
    }
    let written = body(&skipped, Standing::Whitelisted, None);
    assert_eq!(
        written["judge"],
        json!({"mode": "reply", "unjudged": "rate_full"})
    );
    assert_eq!(written["outcome"], "record");
}

#[test]
fn a_follow_up_names_the_one_it_took_over_or_put_down() {
    let inherited = decision(
        Route::Commit,
        Conclusion::Reply,
        Supersede::Inherit {
            msg: seq(9),
            conditions: Conditions::default(),
        },
    );
    let written = body(&inherited, Standing::Member, None);
    assert_eq!(written["supersede"], json!({"inherit": 9}));
    assert_eq!(written["route"], "commit");
    assert!(written.get("judge").is_none(), "接过去的不问判官");
    let rejudged = decision(
        Route::Judge,
        Conclusion::Judge(Mode::Reply),
        Supersede::Rejudge {
            cancel: seq(11),
            msgs: vec![seq(11), seq(12)],
            conditions: Conditions::default(),
        },
    );
    assert_eq!(
        body(&rejudged, Standing::Member, None)["supersede"],
        json!({"rejudge": 11})
    );
    let plain = decision(Route::Commit, Conclusion::Reply, Supersede::None);
    assert!(
        body(&plain, Standing::Admin, None)
            .get("supersede")
            .is_none()
    );
}

#[test]
fn kinds_read_back_as_written() {
    let mut names = Vec::new();
    for kind in [
        Kind::Direct,
        Kind::Continuation,
        Kind::AfterSpeaking,
        Kind::Probability,
        Kind::Moderation,
    ] {
        let conditions = Conditions {
            hits: vec![Hit { kind, bonus: 0.0 }],
        };
        let written = body(
            &Decision {
                passed: Some(Passed {
                    conditions,
                    ..decision(Route::Commit, Conclusion::Reply, Supersede::None)
                        .passed
                        .expect("放行了")
                }),
                ..decision(Route::Commit, Conclusion::Reply, Supersede::None)
            },
            Standing::Admin,
            None,
        );
        let name = written["conditions"][0]["kind"].as_str().expect("写成字");
        assert_eq!(kind_of(name), Some(kind), "{name}");
        names.push(name.to_string());
    }
    assert_eq!(
        names,
        [
            "direct",
            "continuation",
            "after_speaking",
            "probability",
            "moderation"
        ]
    );
    assert_eq!(kind_of("inherited"), None);
    assert_eq!(kind_of(""), None);
}
