//! 判一条群消息（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 5 到 7 条）：主人冲她来的回；没条件的只记下；别人、自己人
//! 冲她来、只有违规旗的要问判官；限流满了冲她来的头一回提示、再来只记下，自己人、主人不受限流；睡着、不让叫的只记下，主人
//! 照回。判断的 `body` 照图纸那张表。参数照出厂的 `defaults.toml`。

use miyu_chat::{
    Chatty, Clock, Ctx, Facts, File, Moderation, Params, Rate, Said, Sleep, Source, Standing, Why,
};
use miyu_kernel::id::{ExternalId, Seq, VenueId};
use miyu_kernel::time::{Timestamp, UtcOffset};
use serde_json::json;

use super::{Case, Conclusion, decide};

/// 出厂参数。
fn params() -> Params {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/software/onebot/defaults.toml"
    );
    let file = File {
        source: Source::Factory,
        name: "defaults.toml".to_string(),
        text: std::fs::read_to_string(path).expect("出厂的读得到"),
    };
    Params::read(&file).expect("出厂的读得出")
}

/// 此刻：UTC 的中午。
fn clock() -> Clock {
    Clock {
        now: Timestamp::from_unix_millis(1_760_011_200_000).expect("在范围里"),
        offset: UtcOffset::UTC,
    }
}

/// 此刻往前 `seconds` 秒。
fn ago(seconds: i64) -> Timestamp {
    Timestamp::from_unix_millis(clock().now.unix_millis() - seconds * 1000).expect("在范围里")
}

/// 这个群的情形：什么都没设，没开过回合。
fn ctx(params: &Params) -> Ctx {
    Ctx {
        rate: None,
        sleep: None,
        allow: None,
        muted: false,
        turns: Vec::new(),
        notices: Vec::new(),
        moderation: Moderation {
            keywords: vec!["违规".to_string()],
            base64: params.base64,
        },
    }
}

/// 第 12 条：`standing` 的人发的，冲不冲她来是 `addressed`。抽样的种子 `SHA-256("qq:group:5\n12")` 落在千分之 116，出厂的千分之
/// 50 抽不中（拿 Python 的 hashlib 算的）：没条件的结论不随抽样变。
fn facts(standing: Standing, addressed: bool) -> Facts {
    Facts {
        venue: VenueId::parse("qq:group:5").expect("合写法"),
        msg: Seq::new(12).expect("不是 0"),
        said: Said {
            sender: ExternalId::parse("qq:20002").expect("合写法"),
            standing,
            addressed,
        },
        mentions_others: false,
        quotes_other: false,
        textless: false,
        media_only: false,
    }
}

/// 判一条：正文 `text`。
fn judged(facts: Facts, text: &str, ctx: Ctx, chatty: &Chatty) -> super::Decision {
    decide(&Case {
        facts,
        text: text.to_string(),
        ctx,
        replies: &[],
        clock: clock(),
        chatty,
    })
}

#[test]
fn the_owner_calling_her_is_answered_and_the_decision_reads_as_drawn() {
    let params = params();
    let decision = judged(
        facts(Standing::Owner, true),
        "@米尤 在吗",
        ctx(&params),
        &params.chatty,
    );
    assert_eq!(decision.conclusion, Conclusion::Reply);
    assert_eq!(
        decision.body(&[12], Standing::Owner),
        json!({
            "msgs": [12], "standing": "owner", "inbound": "pass", "conditions": [{"kind": "direct", "bonus": 0.3}],
            "route": "commit", "outcome": "reply",
        })
    );
}

#[test]
fn nothing_holding_is_recorded_and_others_wait_for_the_judge() {
    let params = params();
    let chatty = &params.chatty;
    let quiet = judged(
        facts(Standing::Member, false),
        "大家好",
        ctx(&params),
        chatty,
    );
    assert_eq!(quiet.conclusion, Conclusion::Record);
    let body = quiet.body(&[12], Standing::Member);
    assert_eq!(
        (&body["conditions"], &body["route"]),
        (&json!([]), &json!("record"))
    );
    for standing in [Standing::Member, Standing::Trusted] {
        let calling = judged(facts(standing, true), "@米尤 在吗", ctx(&params), chatty);
        assert_eq!(calling.conclusion, Conclusion::NoJudge, "{standing:?}");
        let body = calling.body(&[12], standing);
        assert_eq!(body["route"], "judge", "{body}");
        assert_eq!(body["outcome"], "no_judge", "{body}");
    }
    // 只有违规旗：判官只查违规，这一步也是判官还没接。
    let flagged = judged(
        facts(Standing::Member, false),
        "这是违规的话",
        ctx(&params),
        chatty,
    );
    assert_eq!(flagged.conclusion, Conclusion::NoJudge);
    let body = flagged.body(&[12], Standing::Member);
    assert_eq!(body["flags"], json!(["moderation"]), "{body}");
    assert_eq!(body["route"], "moderation_only", "{body}");
    // 主人没冲她来：要问判官（只有主人的 @ 不过判官）的话这一步只记判断；什么都没成立的只记下。
    let owner = judged(
        facts(Standing::Owner, false),
        "辛苦了",
        ctx(&params),
        chatty,
    );
    assert_eq!(owner.conclusion, Conclusion::Record);
}

#[test]
fn a_full_rate_notices_once_then_records_and_spares_owner_and_trusted() {
    let params = params();
    let chatty = &params.chatty;
    let full = Ctx {
        rate: Rate::read("1/1h"),
        turns: vec![ago(60)],
        ..ctx(&params)
    };
    let first = judged(facts(Standing::Member, true), "@米尤", full.clone(), chatty);
    assert_eq!(first.conclusion, Conclusion::Notice(Why::RateLimited));
    assert_eq!(
        first.body(&[12], Standing::Member),
        json!({"msgs": [12], "standing": "member", "inbound": "notice", "why": "rate_limited", "outcome": "notice"}),
        "没放行的不算条件、不走路"
    );
    let noticed = Ctx {
        notices: vec![ago(30)],
        ..full.clone()
    };
    let again = judged(facts(Standing::Member, true), "@米尤", noticed, chatty);
    assert_eq!(again.conclusion, Conclusion::Record);
    assert_eq!(
        again.body(&[12], Standing::Member)["inbound"],
        "record_only"
    );
    let quiet = judged(
        facts(Standing::Member, false),
        "大家好",
        full.clone(),
        chatty,
    );
    assert_eq!(quiet.conclusion, Conclusion::Record, "不冲她来的不提示");
    let trusted = judged(
        facts(Standing::Trusted, true),
        "@米尤",
        full.clone(),
        chatty,
    );
    assert_eq!(trusted.conclusion, Conclusion::NoJudge, "自己人不受限流");
    let owner = judged(facts(Standing::Owner, true), "@米尤", full, chatty);
    assert_eq!(owner.conclusion, Conclusion::Reply, "主人不受限流");
}

#[test]
fn asleep_or_not_allowed_only_records_but_the_owner_is_answered() {
    let params = params();
    let chatty = &params.chatty;
    let asleep = Ctx {
        sleep: Sleep::read("11:00-13:00"),
        ..ctx(&params)
    };
    let member = judged(
        facts(Standing::Member, true),
        "@米尤",
        asleep.clone(),
        chatty,
    );
    assert_eq!(
        member.body(&[12], Standing::Member),
        json!({"msgs": [12], "standing": "member", "inbound": "record_only", "why": "asleep", "outcome": "record"})
    );
    let trusted = judged(
        facts(Standing::Trusted, true),
        "@米尤",
        asleep.clone(),
        chatty,
    );
    assert_eq!(
        trusted.conclusion,
        Conclusion::Record,
        "群里的自己人不豁免睡眠"
    );
    let owner = judged(facts(Standing::Owner, true), "@米尤", asleep, chatty);
    assert_eq!(owner.conclusion, Conclusion::Reply);
    let closed = Ctx {
        allow: Some(false),
        ..ctx(&params)
    };
    let member = judged(facts(Standing::Member, true), "@米尤", closed, chatty);
    assert_eq!(member.body(&[12], Standing::Member)["why"], "not_allowed");
}
