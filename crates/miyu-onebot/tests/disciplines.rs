//! 线路规程（施工 O-23 下，`onebot.md` 第一条「群里怎么叫她」第 10 条，18 第七节那张表）：`every-message` 有字的都开一轮；
//! `when-called` 只认冲她来、续聊、违规旗，看顶替；`wake` 只认唤醒词（触发词）和续聊，不看顶替；三种都不抽样、不打分。工作群
//! （`when-called`）里只有违规旗的问判官只查违规：判官说不够就不回，够了才回。规则把抽样开到必中：不是 `chatty` 的照样不抽。

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::support::group::*;
use crate::support::judge::*;
use crate::support::*;

/// 三个群，各挂一种线路规程。
const EVERY: i64 = 601;
const CALLED: i64 = 602;
const WAKE: i64 = 603;

/// 群里的两个别人。
const LIN: i64 = 20002;
const JIE: i64 = 20003;

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 系统的违规词表里的一个词。
const BAD: &str = "坏词";

/// 核心照中文写的 `/stop` 的回执。
const STOPPED: &str = "已全部停下。";

/// 系统的场所规则：群都抽样必中；三个群各挂一种线路规程，[`WAKE`] 的唤醒词是她的名字。
fn rules() -> String {
    format!(
        "[[rule]]\nmatch = {{ kind = \"group\" }}\nchatty = {{ probability = 1000 }}\n\n\
         [[rule]]\nmatch = {{ kind = \"group\", group = [{EVERY}] }}\ndiscipline = \"every-message\"\n\n\
         [[rule]]\nmatch = {{ kind = \"group\", group = [{CALLED}] }}\ndiscipline = \"when-called\"\n\n\
         [[rule]]\nmatch = {{ kind = \"group\", group = [{WAKE}] }}\ndiscipline = \"wake\"\nkeywords = [\"米尤\"]\n"
    )
}

/// 场所编号。
fn venue(group: i64) -> String {
    format!("qq:group:{group}")
}

/// 第 `message` 条消息的那一笔判断的 `body`；没有的是空的。
fn decided(events: &[Value], message: i64) -> Option<Value> {
    let cause = format!("qq:{BOT}:{message}:{TIME}/decided");
    events
        .iter()
        .find(|event| event["kind"] == "ext.onebot.chat.decided" && event["cause"] == cause)
        .map(|event| event["body"].clone())
}

/// 等到群 `group` 里第 `message` 条消息的判断记下了，交回它。
async fn until_decided(home: &Home, group: i64, message: i64) -> Value {
    let events = until_events(&home.root, &venue(group), |events| {
        decided(events, message).is_some()
    })
    .await;
    decided(&events, message).expect("记了判断")
}

/// 等到群 `group` 里已经有 `turns` 轮走完。
async fn until_turns(home: &Home, group: i64, turns: usize) {
    until_events(&home.root, &venue(group), |events| {
        of_kind(events, "turn.ended").len() == turns
    })
    .await;
}

/// 判断里的走的路和结论。
fn way(body: &Value) -> (Value, Value) {
    (body["route"].clone(), body["outcome"].clone())
}

#[tokio::test]
async fn each_discipline_goes_its_own_way() {
    let checked = |severity: u8| {
        verdict(json!({
            "relevance": 0, "willingness": 0, "social": 0, "timing": 0, "continuity": 0,
            "should_reply": false, "to_bot": false, "severity": severity, "reason": "checked",
        }))
    };
    let server = judge(vec![checked(2), checked(8)]).await;
    let script = Script::new([
        Play::Says("好。"),
        Play::Stalls,
        Play::Says("别这样。"),
        Play::Stalls,
        Play::Says("三点。"),
        Play::Says("也热。"),
    ]);
    let words = format!("{BAD}\n");
    let (home, mut napcat, _) =
        started_judged(&script, &server, (&rules(), &words), "", MEMBERS).await;
    let send = |group, user, message, segments: Value| {
        group_frame(group, user, message, segments, ("某人", "x"))
    };

    // every-message：有字的开一轮，只有图的只记下。
    napcat.send(send(EVERY, LIN, 1, json!([plain("大家好")])));
    assert_eq!(napcat.group_reply(EVERY).await, "好。");
    let first = until_decided(&home, EVERY, 1).await;
    assert_eq!(first["discipline"], "every-message", "{first}");
    assert_eq!(way(&first), (json!("commit"), json!("reply")), "{first}");
    let image = json!([{"type": "image", "data": {"file": "a.jpg", "file_id": "a.jpg"}}]);
    napcat.send(send(EVERY, LIN, 2, image));
    let only_image = until_decided(&home, EVERY, 2).await;
    assert_eq!(
        way(&only_image),
        (json!("record"), json!("record")),
        "{only_image}"
    );

    // when-called：没叫她的只记下（不抽样）；@ 她开一轮；她还没回完，同一个人补一句接过去；违规旗直接开一轮。
    napcat.send(send(CALLED, LIN, 3, json!([plain("随便聊聊")])));
    let idle = until_decided(&home, CALLED, 3).await;
    assert_eq!(idle["discipline"], "when-called", "{idle}");
    assert_eq!(idle["conditions"], json!([]), "不抽样：{idle}");
    assert_eq!(way(&idle), (json!("record"), json!("record")));
    napcat.send(send(CALLED, LIN, 4, json!([at(BOT), plain(" 在吗")])));
    until_event(&home.root, &venue(CALLED), |event| {
        event["kind"] == "turn.started"
    })
    .await;
    napcat.send(send(CALLED, LIN, 5, json!([plain("我是说明天")])));
    let follow = until_decided(&home, CALLED, 5).await;
    assert!(follow["supersede"]["inherit"].is_u64(), "{follow}");
    assert_eq!(way(&follow), (json!("commit"), json!("reply")));
    napcat.send(send(CALLED, ADMIN, 6, json!([plain("/stop")])));
    assert_eq!(napcat.group_reply(CALLED).await, STOPPED);
    until_turns(&home, CALLED, 1).await;
    // 有人说了违规词、没叫她：关键词只把判官拉起来。判官说严重程度 2，不够，不回；再一句判官说 8，回。
    napcat.send(send(CALLED, JIE, 7, json!([plain(&format!("这有{BAD}"))])));
    let mild = until_decided(&home, CALLED, 7).await;
    assert_eq!(
        mild["conditions"],
        json!([{"kind": "moderation", "bonus": 0.0}]),
        "只有违规旗：{mild}"
    );
    assert_eq!(way(&mild), (json!("moderation_only"), json!("record")));
    assert_eq!(mild["judge"]["mode"], "moderation_only", "{mild}");
    assert_eq!(mild["judge"]["answer"]["severity"], 2, "{mild}");
    assert_eq!(mild["score"]["reply"], false, "{mild}");
    napcat.send(send(CALLED, JIE, 70, json!([plain(&format!("又是{BAD}"))])));
    assert_eq!(napcat.group_reply(CALLED).await, "别这样。");
    let severe = until_decided(&home, CALLED, 70).await;
    assert_eq!(way(&severe), (json!("moderation_only"), json!("reply")));
    assert_eq!(severe["judge"]["answer"]["severity"], 8, "{severe}");
    let system = asked(&server, 0)["messages"][0]["content"]
        .as_str()
        .expect("是字")
        .to_string();
    let only =
        std::fs::read_to_string(resources().join("software/onebot/judge/moderation-only.txt"))
            .expect("读得出");
    assert!(system.contains(&only), "只查违规：{system}");

    // wake：没叫的只记下；唤醒词开一轮；她还没回完，补一句不接（不看顶替）；她回过的人接着说开一轮，别人不算。
    napcat.send(send(WAKE, JIE, 8, json!([plain("今天好热")])));
    let idle = until_decided(&home, WAKE, 8).await;
    assert_eq!(idle["discipline"], "wake", "{idle}");
    assert_eq!(way(&idle), (json!("record"), json!("record")));
    napcat.send(send(WAKE, JIE, 9, json!([plain("米尤，几点了")])));
    until_event(&home.root, &venue(WAKE), |event| {
        event["kind"] == "turn.started"
    })
    .await;
    napcat.send(send(WAKE, JIE, 10, json!([plain("喂喂")])));
    let ignored = until_decided(&home, WAKE, 10).await;
    assert!(ignored.get("supersede").is_none(), "{ignored}");
    assert_eq!(way(&ignored), (json!("record"), json!("record")));
    napcat.send(send(WAKE, ADMIN, 11, json!([plain("/stop")])));
    assert_eq!(napcat.group_reply(WAKE).await, STOPPED);
    until_turns(&home, WAKE, 1).await;
    napcat.send(send(WAKE, JIE, 12, json!([plain("米尤，几点了")])));
    assert_eq!(napcat.group_reply(WAKE).await, "三点。");
    until_turns(&home, WAKE, 2).await;
    // 续聊照她的回复算（`venue.delivered`）：NapCat 收到她的话时桥还没记送达，等记下了再接着说，不然这一句可能先判、算不上续聊。
    until_events(&home.root, &venue(WAKE), |events| {
        !of_kind(events, "venue.delivered").is_empty()
    })
    .await;
    napcat.send(send(WAKE, JIE, 13, json!([plain("那明天呢")])));
    assert_eq!(napcat.group_reply(WAKE).await, "也热。");
    let continued = until_decided(&home, WAKE, 13).await;
    assert_eq!(
        continued["conditions"],
        json!([{"kind": "continuation", "bonus": 0.1}]),
        "{continued}"
    );
    until_turns(&home, WAKE, 3).await;
    napcat.send(send(WAKE, LIN, 14, json!([plain("我也问")])));
    let other = until_decided(&home, WAKE, 14).await;
    assert_eq!(other["conditions"], json!([]), "刚说过话不算：{other}");
    assert_eq!(way(&other), (json!("record"), json!("record")));

    assert_eq!(server.received().len(), 2, "只为那两句违规的问过判官");
    assert_eq!(script.requests().len(), 6);
    assert!(napcat.pending().is_none(), "别的什么都不发");
    stopped(home).await;
}
