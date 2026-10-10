//! 判一条群消息的线路规程和顶替（施工 O-23 下，`onebot.md` 第一条「群里怎么叫她」第 10、11 条）：`when-called` 不抽样，只有
//! 违规旗的问判官只查违规（额度满了不问），冲她来的照开；`every-message` 有字的都回，违规旗不改它；同一个人在窗口里补发，前一条
//! 判过要回的接过去，还在判的几条一起重判，`wake` 不看顶替。夹具在 `tests.rs`。

use miyu_chat::{Ctx, Facts, Mode, Rate, Standing, Status, Supersede};
use miyu_kernel::id::Seq;
use serde_json::json;

use super::super::body::body as written;
use super::super::discipline::Discipline;
use super::Conclusion;
use super::tests::{Around, ago, ago_millis, ctx, facts, judged_in, params, pending, sampling};

#[test]
fn other_disciplines_ask_the_judge_only_about_violations() {
    let params = sampling();
    let chatty = &params.chatty;
    let called = Around {
        discipline: Discipline::WhenCalled,
        pendings: &[],
    };
    let calling = judged_in(
        facts(Standing::Member, true),
        "@米尤",
        ctx(&params),
        chatty,
        &called,
    );
    assert_eq!(calling.conclusion, Conclusion::Reply, "冲她来的直接回");
    let idle = judged_in(
        facts(Standing::Member, false),
        "大家好",
        ctx(&params),
        chatty,
        &called,
    );
    assert_eq!(idle.conclusion, Conclusion::Record, "抽样必中也不抽");
    assert_eq!(
        written(&idle, Standing::Member, None)["discipline"],
        "when-called"
    );
    // 只有违规旗：问判官只查违规；额度满了不问，只记下。冲她来又有违规旗的照回。
    let flagged = judged_in(
        facts(Standing::Member, false),
        "这是违规的话",
        ctx(&params),
        chatty,
        &called,
    );
    assert_eq!(flagged.conclusion, Conclusion::Judge(Mode::ModerationOnly));
    let body = written(&flagged, Standing::Member, None);
    assert_eq!(
        (&body["route"], &body["judge"]),
        (
            &json!("moderation_only"),
            &json!({"mode": "moderation_only"})
        ),
        "{body}"
    );
    let full = Ctx {
        rate: Rate::read("1/1h"),
        turns: vec![ago(60)],
        ..ctx(&params)
    };
    let held = judged_in(
        facts(Standing::Whitelisted, false),
        "这是违规的话",
        full,
        chatty,
        &called,
    );
    assert_eq!(held.conclusion, Conclusion::Record, "额度满了不问判官");
    assert_eq!(
        written(&held, Standing::Whitelisted, None)["judge"],
        json!({"mode": "moderation_only", "unjudged": "rate_full"})
    );
    let both = judged_in(
        facts(Standing::Member, true),
        "@米尤 这是违规的话",
        ctx(&params),
        chatty,
        &called,
    );
    assert_eq!(both.conclusion, Conclusion::Reply, "冲她来的照开");
    let every = Around {
        discipline: Discipline::EveryMessage,
        pendings: &[],
    };
    let any = judged_in(
        facts(Standing::Member, false),
        "大家好",
        ctx(&params),
        chatty,
        &every,
    );
    assert_eq!(any.conclusion, Conclusion::Reply, "有字的都回");
    let flagged = judged_in(
        facts(Standing::Member, false),
        "这是违规的话",
        ctx(&params),
        chatty,
        &every,
    );
    assert_eq!(flagged.conclusion, Conclusion::Reply, "违规旗不改它");
    let image = Facts {
        media_only: true,
        ..facts(Standing::Member, false)
    };
    let pictured = judged_in(image, "", ctx(&params), chatty, &every);
    assert_eq!(pictured.conclusion, Conclusion::Record, "只有图的只记下");
}

#[test]
fn a_follow_up_takes_over_or_is_judged_with_the_one_before() {
    let params = params();
    let chatty = &params.chatty;
    // 前一条判过要回、她还没回完：接过去，不再判，条件是前一条的。
    let committed = [pending(10, &[], ago(3), Status::Committed)];
    let around = Around {
        discipline: Discipline::Chatty,
        pendings: &committed,
    };
    let follow = judged_in(
        facts(Standing::Member, false),
        "我是说明天",
        ctx(&params),
        chatty,
        &around,
    );
    assert_eq!(follow.conclusion, Conclusion::Reply);
    assert_eq!(follow.msgs, [12]);
    let body = written(&follow, Standing::Member, None);
    assert_eq!(body["supersede"], json!({"inherit": 10}), "{body}");
    assert_eq!(
        body["conditions"],
        json!([{"kind": "direct", "bonus": 0.3}])
    );
    assert_eq!(body["route"], "commit");
    // when-called 也看顶替；wake 不看。
    let called = Around {
        discipline: Discipline::WhenCalled,
        pendings: &committed,
    };
    let taken = judged_in(
        facts(Standing::Member, false),
        "我是说明天",
        ctx(&params),
        chatty,
        &called,
    );
    assert_eq!(taken.conclusion, Conclusion::Reply);
    let woken = Around {
        discipline: Discipline::Wake,
        pendings: &committed,
    };
    let ignored = judged_in(
        facts(Standing::Member, false),
        "我是说明天",
        ctx(&params),
        chatty,
        &woken,
    );
    assert_eq!(ignored.conclusion, Conclusion::Record);
    // 前一条还在判：几条一起重判，判的是那一条接过的、那一条、这一条。
    let judging = [pending(10, &[8], ago(3), Status::Judging)];
    let around = Around {
        discipline: Discipline::Chatty,
        pendings: &judging,
    };
    let together = judged_in(
        facts(Standing::Member, false),
        "我是说明天",
        ctx(&params),
        chatty,
        &around,
    );
    assert_eq!(together.conclusion, Conclusion::Judge(Mode::Reply));
    assert_eq!(together.msgs, [8, 10, 12]);
    assert_eq!(
        together.passed.as_ref().map(|passed| &passed.supersede),
        Some(&Supersede::Rejudge {
            cancel: Seq::new(10).expect("不是 0"),
            msgs: [8, 10, 12].iter().filter_map(|n| Seq::new(*n)).collect(),
            conditions: pending(10, &[], ago(3), Status::Judging).conditions,
        })
    );
    // 终端管理员补一句 @ 她：几条一起，不过判官，回。
    let admin = judged_in(
        facts(Standing::Admin, true),
        "@米尤 我是说明天",
        ctx(&params),
        chatty,
        &around,
    );
    assert_eq!(admin.conclusion, Conclusion::Reply);
    assert_eq!(admin.msgs, [8, 10, 12]);
    // 正好 7 秒以前的不算顶替；差一毫秒的算。
    let late = [pending(10, &[], ago(7), Status::Committed)];
    let around = Around {
        discipline: Discipline::Chatty,
        pendings: &late,
    };
    let alone = judged_in(
        facts(Standing::Member, false),
        "我是说明天",
        ctx(&params),
        chatty,
        &around,
    );
    assert_eq!(alone.conclusion, Conclusion::Record);
    assert!(
        written(&alone, Standing::Member, None)
            .get("supersede")
            .is_none()
    );
    let just = [pending(10, &[], ago_millis(6999), Status::Committed)];
    let around = Around {
        discipline: Discipline::Chatty,
        pendings: &just,
    };
    let inside = judged_in(
        facts(Standing::Member, false),
        "我是说明天",
        ctx(&params),
        chatty,
        &around,
    );
    assert_eq!(inside.conclusion, Conclusion::Reply, "窗口照这个群的参数");
}
