//! 顶替（施工 O-23 下，`onebot.md` 第一条「群里怎么叫她」第 11 条，`chat.md` 第四条）：同一个人在顶替窗口里补发一条。前一条
//! 判过要回、她还没回完的，这一条接过去、并进她这一轮；前一条还在判的，放下那一次（晚回来的回答丢掉），几条一起重判。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Barrier;

use miyu_session::testkit::{Play, Script};

use crate::support::group::*;
use crate::support::judge::*;
use crate::support::*;

/// 不抽样的群。
const GROUP: i64 = 555;

/// 另一个群：在这里说一句，等桥办完手上的。
const OTHER: i64 = 559;

/// 群里的两个别人。
const LIN: i64 = 20002;
const JIE: i64 = 20003;

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 系统的场所规则：群不抽样。
const RULES: &str = "[[rule]]\nmatch = { kind = \"group\" }\nchatty = { probability = 0 }\n";

/// 场所编号。
fn venue() -> String {
    format!("qq:group:{GROUP}")
}

/// 第 `message` 条消息的命令编号加 `/what`。
fn cause(message: i64, what: &str) -> String {
    format!("qq:{BOT}:{message}:{TIME}/{what}")
}

/// 第 `message` 条消息的那一笔判断的 `body`；没有的是空的。
fn decided(events: &[Value], message: i64) -> Option<Value> {
    events
        .iter()
        .find(|event| {
            event["kind"] == "ext.onebot.chat.decided"
                && event["cause"] == cause(message, "decided")
        })
        .map(|event| event["body"].clone())
}

/// 平台编号是 `message` 的那一条人说的话的序号。
fn seq_of(events: &[Value], message: i64) -> Value {
    said(events)
        .iter()
        .find(|said| said["body"]["venue"]["msg"].as_str() == Some(message.to_string().as_str()))
        .map(|said| said["seq"].clone())
        .unwrap_or_else(|| panic!("没有第 {message} 条：{events:#?}"))
}

#[tokio::test]
async fn a_follow_up_joins_the_turn_she_is_in() {
    // 小林 @ 她，判官说回；她还没开口，小林补一句没 @ 的：接过去，并进她这一轮，不再问判官。
    let server = judge(vec![yes("asked her")]).await;
    let script = Script::new([Play::Stalls]);
    let (home, napcat, _) = started_judged(&script, &server, (RULES, ""), "", MEMBERS).await;
    napcat.send(group_frame(
        GROUP,
        LIN,
        1,
        json!([at(BOT), plain(" 明天几点开会")]),
        ("小林", "lin"),
    ));
    until_event(&home.root, &venue(), |event| {
        event["kind"] == "turn.started"
    })
    .await;
    napcat.send(group_frame(
        GROUP,
        LIN,
        2,
        json!([plain("我是说后天")]),
        ("小林", "lin"),
    ));
    let events = until_event(&home.root, &venue(), |event| event["kind"] == "turn.joined").await;
    let joined = &of_kind(&events, "turn.joined")[0];
    assert_eq!(joined["body"]["triggers"], json!([seq_of(&events, 2)]));
    assert_eq!(joined["cause"], cause(2, "respond"));
    let events = until_events(&home.root, &venue(), |events| decided(events, 2).is_some()).await;
    assert_eq!(
        decided(&events, 2),
        Some(json!({
            "msgs": [seq_of(&events, 2)], "standing": "member", "inbound": "pass", "discipline": "chatty",
            "conditions": [{"kind": "direct", "bonus": 0.3}], "supersede": {"inherit": seq_of(&events, 1)},
            "route": "commit", "outcome": "reply",
        })),
        "条件是前一条的，接过去不再判"
    );
    assert_eq!(server.received().len(), 1, "补的那一句不再问判官");
    assert_eq!(of_kind(&events, "turn.started").len(), 1, "不另开一轮");
    assert_eq!(script.requests().len(), 1);
    stopped(home).await;
}

#[tokio::test]
async fn a_follow_up_while_judging_is_judged_together() {
    // 小林 @ 她，判官还没回，小林补一句：放下前一次，两条一起问。前一次晚回来说不回，丢掉；一起问的说回，回。两份回答都等
    // 两个请求都到了才一起放行（闸），不靠哪一边快：前一次在一起问的那一次发出去以前一定回不来。
    let gate = Arc::new(Barrier::new(2));
    let server = judge(vec![
        gated(&gate, no("first ask")),
        gated(&gate, yes("both together")),
    ])
    .await;
    let script = Script::new([Play::Says("后天十点。")]);
    let (home, mut napcat, _) = started_judged(&script, &server, (RULES, ""), "", MEMBERS).await;
    napcat.send(group_frame(
        GROUP,
        LIN,
        1,
        json!([
            at(BOT),
            plain(" 明天几点开会 56eY5a+G55qE6K+d5Zyo6L+Z6YeM6JeP552A")
        ]),
        ("小林", "lin"),
    ));
    // 前一次的请求先到了假服务器（第一个连接拿第一份回答），再补那一句。
    let deadline = tokio::time::Instant::now() + WAIT;
    while server.received().is_empty() {
        assert!(tokio::time::Instant::now() < deadline, "判官等不到请求");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    napcat.send(group_frame(
        GROUP,
        LIN,
        2,
        json!([plain("我是说后天")]),
        ("小林", "lin"),
    ));
    assert_eq!(napcat.group_reply(GROUP).await, "后天十点。");
    let events = until_events(&home.root, &venue(), |events| {
        of_kind(events, "turn.ended").len() == 1
    })
    .await;
    let (first, second) = (seq_of(&events, 1), seq_of(&events, 2));
    let together = decided(&events, 2).expect("记了判断");
    assert_eq!(together["msgs"], json!([first, second]), "{together}");
    assert_eq!(
        together["supersede"],
        json!({"rejudge": first}),
        "{together}"
    );
    assert_eq!(
        together["conditions"],
        json!([{"kind": "direct", "bonus": 0.3}])
    );
    assert_eq!(together["judge"]["answer"]["reason"], "both together");
    assert_eq!(together["outcome"], "reply");
    let started = of_kind(&events, "turn.started");
    assert_eq!(started.len(), 1, "{started:#?}");
    assert_eq!(started[0]["body"]["triggers"], json!([first, second]));
    assert_eq!(started[0]["cause"], cause(2, "respond"));
    // 一起问的那一次，判官看的这一条是后一条，前一条在记录里；前一条里的 base64 解出来的字照样给判官看。
    let user = asked(&server, 1)["messages"][1]["content"]
        .as_str()
        .expect("是字")
        .to_string();
    let (records, current) = user.split_once("<current-message>").expect("有这一条");
    assert!(records.contains("明天几点开会"), "{user}");
    assert!(current.contains("我是说后天"), "{user}");
    assert!(
        current.contains("秘密的话在这里藏着"),
        "几条一起解 base64，前一条里的也给判官看：{user}"
    );
    // 前一次晚回来的说不回：等假服务器把它回完，再叫她一次，判断只多那一笔，前一条没有自己的判断。
    let mut server = server;
    server.wait_closed(2).await;
    // 另一个群里说一句、等它记下：桥一件件照先后办，晚到的那一次这时已经办过了。
    napcat.send(group_frame(
        OTHER,
        JIE,
        3,
        json!([plain("路过")]),
        ("阿杰", "jie"),
    ));
    until_events(&home.root, &format!("qq:group:{OTHER}"), |events| {
        decided(events, 3).is_some()
    })
    .await;
    let events = venue_events(&home.root, &venue());
    assert!(decided(&events, 1).is_none(), "放下的那一次不记判断");
    assert_eq!(of_kind(&events, "ext.onebot.chat.decided").len(), 1);
    assert_eq!(of_kind(&events, "turn.started").len(), 1);
    assert_eq!(script.requests().len(), 1);
    stopped(home).await;
}

#[tokio::test]
async fn the_turn_opened_for_a_follow_up_answers_that_person() {
    // 小林 @ 她、判官说回；她这一轮还没说完（头一次请求压着），小林补一句：并进这一轮。这一轮没再请求就结束了，核心接着开
    // 一轮（`turn.started` 只有指向那条 `turn.joined` 的 `trigger`）：这一轮的话也是回小林的，他再说一句认得出是续聊。
    let server = judge(vec![yes("asked her"), no("not this time")]).await;
    let script = Script::new([Play::Says("在。"), Play::Says("补上。")]);
    let (release, held) = tokio::sync::oneshot::channel();
    let models = holding(&script, held);
    let (home, mut napcat, _) =
        started_with_models(models, &server, (RULES, ""), "", MEMBERS).await;
    napcat.send(group_frame(
        GROUP,
        LIN,
        1,
        json!([at(BOT), plain(" 明天几点开会")]),
        ("小林", "lin"),
    ));
    until_event(&home.root, &venue(), |event| {
        event["kind"] == "turn.started"
    })
    .await;
    napcat.send(group_frame(
        GROUP,
        LIN,
        2,
        json!([plain("我是说后天")]),
        ("小林", "lin"),
    ));
    let events = until_event(&home.root, &venue(), |event| event["kind"] == "turn.joined").await;
    let joined = of_kind(&events, "turn.joined")[0]["seq"].clone();
    release.send(()).expect("还在等");
    assert_eq!(napcat.group_reply(GROUP).await, "在。");
    assert_eq!(napcat.group_reply(GROUP).await, "补上。");
    let events = until_events(&home.root, &venue(), |events| {
        of_kind(events, "venue.delivered").len() == 2 && of_kind(events, "turn.ended").len() == 2
    })
    .await;
    let started = of_kind(&events, "turn.started");
    assert_eq!(started.len(), 2, "{started:#?}");
    assert_eq!(started[1]["body"]["trigger"], joined, "核心接着开的一轮");
    assert!(
        started[1]["body"].get("triggers").is_none(),
        "{:#}",
        started[1]
    );
    let delivered = of_kind(&events, "venue.delivered");
    for one in &delivered {
        assert_eq!(one["body"]["to"], json!([format!("qq:{LIN}")]), "{one}");
    }
    assert_eq!(delivered[1]["body"]["turn"], started[1]["seq"]);
    // 小林接着说（没 @ 她）：最近一轮回的是他，算续聊。
    napcat.send(group_frame(
        GROUP,
        LIN,
        3,
        json!([plain("那就这样")]),
        ("小林", "lin"),
    ));
    let events = until_events(&home.root, &venue(), |events| decided(events, 3).is_some()).await;
    let body = decided(&events, 3).expect("记了判断");
    let kinds: Vec<&Value> = body["conditions"]
        .as_array()
        .expect("有条件")
        .iter()
        .map(|hit| &hit["kind"])
        .collect();
    assert!(
        kinds.contains(&&json!("continuation")),
        "续聊认得出：{body}"
    );
    assert_eq!(body["outcome"], "record", "判官说不回：{body}");
    assert_eq!(script.requests().len(), 2);
    stopped(home).await;
}
