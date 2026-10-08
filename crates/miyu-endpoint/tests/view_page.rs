//! 历史按页读（施工 9-6 下，`docs/blueprint/protocol.md`「`view.page`」）：真核心走一遍。一页一页往前翻，照序号去重以后
//! 拼起来就是整份日志；最新一页的 `last` 接 `subscribe {"after"}`，什么都不补。参数不对、没有这个会话的拒绝。

mod support;

use std::collections::BTreeSet;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use support::*;

/// 一页的回应。
async fn page(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "view.page", params).await
}

fn seqs(reply: &Value) -> Vec<u64> {
    reply["result"]["events"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(|event| event["seq"].as_u64())
        .collect()
}

#[tokio::test]
async fn pages_back_to_the_start_add_up_to_the_whole_log() {
    let home = Home::new();
    let script = Script::new([Play::Says("一。"), Play::Says("二。"), Play::Says("三。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    for (n, text) in ["a", "b", "c"].iter().enumerate() {
        client.say(&format!("s{n}"), &session, text).await;
        home.until_turns(&session, n + 1).await;
    }
    let whole: Vec<u64> = home
        .log(&session)
        .iter()
        .map(|event| event.seq.get())
        .collect();

    let newest = page(&mut client, "p1", json!({"session": session, "turns": 2})).await;
    assert_eq!(newest["result"]["more"], true, "{newest}");
    assert!(newest["result"].get("capped").is_none(), "没到上限的不写");
    assert_eq!(
        newest["result"]["last"],
        json!(whole.last()),
        "最新一页到日志最后一条"
    );
    let first = newest["result"]["first"].as_u64().expect("有切点");
    let started: Vec<u64> = home
        .log(&session)
        .iter()
        .filter(|event| event.body.kind() == "turn.started")
        .map(|event| event.seq.get())
        .collect();
    assert_eq!(first, started[1], "两轮：从第二轮的 turn.started 切");
    assert!(
        seqs(&newest).first().is_some_and(|seq| *seq < first),
        "第二轮的话在切点前，也带上：{newest}"
    );

    let older = page(
        &mut client,
        "p2",
        json!({"session": session, "before": first, "turns": 2}),
    )
    .await;
    assert_eq!(older["result"]["more"], false, "{older}");
    assert_eq!(older["result"]["first"], json!(whole[0]));
    let mut seen = BTreeSet::new();
    seen.extend(seqs(&newest));
    seen.extend(seqs(&older));
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        whole,
        "去重以后拼起来是整份"
    );

    let last = newest["result"]["last"].clone();
    let (pushed, reply) = client.subscribe_after("s9", &session, last.clone()).await;
    assert!(pushed.is_empty(), "接着 last 订阅：什么都不补");
    assert_eq!(reply["result"]["upto"], last);
}

#[tokio::test]
async fn wrong_parameters_and_missing_sessions_are_refused() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let only = page(&mut client, "p0", json!({"session": session})).await;
    assert_eq!(seqs(&only), [1], "只有 session.created：{only}");
    assert_eq!(only["result"]["more"], false);
    for (n, params) in [
        json!({"session": session, "turns": 0}),
        json!({"session": session, "turns": 51}),
        json!({"session": session, "before": 0}),
        json!({"session": session, "before": -1}),
        json!({"session": session, "around": 3}),
        json!({"session": "nope"}),
        json!({}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = page(&mut client, &format!("b{n}"), params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
    let empty = page(&mut client, "e", json!({"session": session, "before": 1})).await;
    assert_eq!(
        empty["result"],
        json!({"events": [], "more": false}),
        "{empty}"
    );
    let missing = page(
        &mut client,
        "m",
        json!({"session": "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91"}),
    )
    .await;
    assert_eq!(reason(&missing), Some("session_not_found"), "{missing}");
}
