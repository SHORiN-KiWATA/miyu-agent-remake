//! 判一条群消息（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 5、6、10、11、14 条）：主人冲她来的回；没条件的只记下；
//! 别人、自己人冲她来的问判官打分，只有违规旗的问判官只查违规；限流满了冲她来的头一回提示、再来只记下，自己人、主人不受限流，
//! 可额度满了的这段时间不抽样、不问判官；睡着、不让叫的只记下，主人照回。线路规程（O-23 下）：`when-called`、`every-message`
//! 不问判官（`follow_tests.rs`）；顶替：同一个人在窗口里，前一条判过要回的接过去，还在判的几条一起重判（同上）。参数照出厂的
//! `defaults.toml`。

use miyu_chat::{
    Chatty, Clock, Conditions, Ctx, Facts, File, Hit, Kind, Mode, Moderation, Params, Pending,
    Rate, Said, Sleep, Source, Standing, Status, Why,
};
use miyu_kernel::id::{ExternalId, Seq, VenueId};
use miyu_kernel::time::{Timestamp, UtcOffset};
use serde_json::json;

use super::super::body::body as written;
use super::super::discipline::Discipline;
use super::{Case, Conclusion, Decision, decide};

/// 出厂参数的原文。
fn defaults() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/software/onebot/defaults.toml"
    );
    std::fs::read_to_string(path).expect("出厂的读得到")
}

/// 读一份参数的原文。
fn read(text: String) -> Params {
    let file = File {
        source: Source::Factory,
        name: "defaults.toml".to_string(),
        text,
    };
    Params::read(&file).expect("读得出")
}

/// 出厂参数。
pub(super) fn params() -> Params {
    read(defaults())
}

/// 出厂参数，只是抽样必中。
pub(super) fn sampling() -> Params {
    read(defaults().replace("probability = 50 ", "probability = 1000 "))
}

/// 此刻：UTC 的中午。
fn clock() -> Clock {
    Clock {
        now: Timestamp::from_unix_millis(1_760_011_200_000).expect("在范围里"),
        offset: UtcOffset::UTC,
    }
}

/// 此刻往前 `millis` 毫秒。
pub(super) fn ago_millis(millis: i64) -> Timestamp {
    Timestamp::from_unix_millis(clock().now.unix_millis() - millis).expect("在范围里")
}

/// 此刻往前 `seconds` 秒。
pub(super) fn ago(seconds: i64) -> Timestamp {
    ago_millis(seconds * 1000)
}

/// 这个群的情形：什么都没设，没开过回合。
pub(super) fn ctx(params: &Params) -> Ctx {
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

/// 发这些消息的人。
fn lin() -> ExternalId {
    ExternalId::parse("qq:20002").expect("合写法")
}

/// 第 12 条：`standing` 的人发的，冲不冲她来是 `addressed`。抽样的种子 `SHA-256("qq:group:5\n12")` 落在千分之 116，出厂的千分之
/// 50 抽不中（拿 Python 的 hashlib 算的）：没条件的结论不随抽样变。
pub(super) fn facts(standing: Standing, addressed: bool) -> Facts {
    Facts {
        venue: VenueId::parse("qq:group:5").expect("合写法"),
        msg: Seq::new(12).expect("不是 0"),
        said: Said {
            sender: lin(),
            standing,
            addressed,
        },
        mentions_others: false,
        quotes_other: false,
        textless: false,
        media_only: false,
    }
}

/// 判一条要的另外几样：线路规程、还没回完的。
pub(super) struct Around<'a> {
    /// 线路规程。
    pub(super) discipline: Discipline,
    /// 还没回完的。
    pub(super) pendings: &'a [Pending],
}

/// `chatty`、没有还没回完的。
const PLAIN: Around<'static> = Around {
    discipline: Discipline::Chatty,
    pendings: &[],
};

/// 判一条：正文 `text`。
fn judged(facts: Facts, text: &str, ctx: Ctx, chatty: &Chatty) -> Decision {
    judged_in(facts, text, ctx, chatty, &PLAIN)
}

/// 同 [`judged`]，线路规程、还没回完的照 `around`；顶替窗口照出厂的。
pub(super) fn judged_in(
    facts: Facts,
    text: &str,
    ctx: Ctx,
    chatty: &Chatty,
    around: &Around<'_>,
) -> Decision {
    decide(&Case {
        facts,
        text: text.to_string(),
        ctx,
        replies: &[],
        clock: clock(),
        chatty,
        discipline: around.discipline,
        pendings: around.pendings,
        window: params().supersede_window,
    })
}

/// 第 `msg` 条，小林在 `at` 发的，条件只有冲她来；前面接过 `absorbed`。
pub(super) fn pending(msg: u64, absorbed: &[u64], at: Timestamp, status: Status) -> Pending {
    Pending {
        msg: Seq::new(msg).expect("不是 0"),
        absorbed: absorbed.iter().filter_map(|one| Seq::new(*one)).collect(),
        sender: lin(),
        at,
        status,
        conditions: Conditions {
            hits: vec![Hit {
                kind: Kind::Direct,
                bonus: 0.3,
            }],
        },
    }
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
        written(&decision, Standing::Owner, None),
        json!({
            "msgs": [12], "standing": "owner", "inbound": "pass", "discipline": "chatty",
            "conditions": [{"kind": "direct", "bonus": 0.3}], "route": "commit", "outcome": "reply",
        })
    );
}

#[test]
fn nothing_holding_is_recorded_and_others_go_to_the_judge() {
    let params = params();
    let chatty = &params.chatty;
    let quiet = judged(
        facts(Standing::Member, false),
        "大家好",
        ctx(&params),
        chatty,
    );
    assert_eq!(quiet.conclusion, Conclusion::Record);
    let body = written(&quiet, Standing::Member, None);
    assert_eq!(
        (&body["conditions"], &body["route"]),
        (&json!([]), &json!("record"))
    );
    for standing in [Standing::Member, Standing::Trusted] {
        let calling = judged(facts(standing, true), "@米尤 在吗", ctx(&params), chatty);
        assert_eq!(
            calling.conclusion,
            Conclusion::Judge(Mode::Reply),
            "{standing:?}"
        );
        assert_eq!(calling.msgs, [12]);
        let body = written(&calling, standing, None);
        assert_eq!(body["route"], "judge", "{body}");
        assert_eq!(body["judge"], json!({"mode": "reply"}), "{body}");
    }
    // 只有违规旗：判官只查违规。
    let flagged = judged(
        facts(Standing::Member, false),
        "这是违规的话",
        ctx(&params),
        chatty,
    );
    assert_eq!(flagged.conclusion, Conclusion::Judge(Mode::ModerationOnly));
    let body = written(&flagged, Standing::Member, None);
    assert_eq!(body["flags"], json!(["moderation"]), "{body}");
    assert_eq!(body["route"], "moderation_only", "{body}");
    // 主人没冲她来、什么都没成立的只记下。
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
        written(&first, Standing::Member, None),
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
        written(&again, Standing::Member, None)["inbound"],
        "record_only"
    );
    let quiet = judged(
        facts(Standing::Member, false),
        "大家好",
        full.clone(),
        chatty,
    );
    assert_eq!(quiet.conclusion, Conclusion::Record, "不冲她来的不提示");
    // 自己人不受限流，可额度满了的这段时间不问判官：只记下（第 14 条）。
    let trusted = judged(
        facts(Standing::Trusted, true),
        "@米尤",
        full.clone(),
        chatty,
    );
    assert_eq!(trusted.conclusion, Conclusion::Record);
    let body = written(&trusted, Standing::Trusted, None);
    assert_eq!(
        (&body["inbound"], &body["route"], &body["judge"]),
        (
            &json!("pass"),
            &json!("judge"),
            &json!({"mode": "reply", "unjudged": "rate_full"})
        ),
        "{body}"
    );
    let owner = judged(facts(Standing::Owner, true), "@米尤", full, chatty);
    assert_eq!(
        owner.conclusion,
        Conclusion::Reply,
        "主人不受限流、不过判官"
    );
}

#[test]
fn a_full_rate_samples_nothing() {
    let params = sampling();
    let chatty = &params.chatty;
    let open = judged(
        facts(Standing::Trusted, false),
        "大家好",
        ctx(&params),
        chatty,
    );
    assert_eq!(open.conclusion, Conclusion::Judge(Mode::Reply), "抽样中了");
    let full = Ctx {
        rate: Rate::read("1/1h"),
        turns: vec![ago(60)],
        ..ctx(&params)
    };
    let closed = judged(facts(Standing::Trusted, false), "大家好", full, chatty);
    assert_eq!(closed.conclusion, Conclusion::Record);
    let body = written(&closed, Standing::Trusted, None);
    assert_eq!(body["conditions"], json!([]), "额度满了不抽样：{body}");
    assert!(body.get("judge").is_none(), "没条件，不是没问判官：{body}");
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
        written(&member, Standing::Member, None),
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
    assert_eq!(
        written(&member, Standing::Member, None)["why"],
        "not_allowed"
    );
}
