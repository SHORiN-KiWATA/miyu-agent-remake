//! `session.respond`（施工 O-14 上，`docs/construction/O-14-照记下的开一轮（上）.md`）：照旁听记下的几条开一轮，`triggers` 排好
//! 去重，事实接在内核的后面、`by` 是这个连接（本机的是管理员）；写错的什么都不记；不是旁听的、当过触发的拒，`data.messages`
//! 是那几条；同一个请求编号再发只算一次。

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::venues::bound_core;
use crate::support::*;

/// 照对应表起来的核心，连上、握手，找到主人（`qq:10001`）的私聊，旁听记下两条，交回连接、会话编号和两条的序号。
async fn overheard_twice(home: &Home, script: &Script) -> (Client, String, Vec<u64>) {
    let mut client = Client::connect(bound_core(home, script, Catalog::default()));
    client.hello().await;
    let made = client
        .call(
            "v1",
            "venue.session",
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001"}),
        )
        .await;
    let session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let mut seqs = Vec::new();
    for (n, msg) in ["8801", "8802"].into_iter().enumerate() {
        let reply = client
            .call(
                &format!("o{n}"),
                "session.send",
                json!({"session": session, "text": format!("第 {n} 句"), "as": {"external": "qq:10001"},
                       "venue": {"msg": msg, "ambient": true}}),
            )
            .await;
        seqs.push(
            reply["result"]["events"][0]
                .as_u64()
                .unwrap_or_else(|| panic!("{reply}")),
        );
    }
    (client, session, seqs)
}

fn respond(session: &str, to: Value) -> Value {
    json!({"session": session, "to": to, "facts": [{"kind": "judge", "text": "<judge/>\n"}]})
}

#[tokio::test]
async fn respond_opens_a_turn_on_what_was_overheard() {
    let home = Home::new();
    let (mut client, session, seqs) =
        overheard_twice(&home, &Script::new([Play::Says("在。")])).await;
    let (a, b) = (seqs[0], seqs[1]);
    let reply = client
        .call("r1", "session.respond", respond(&session, json!([b, a, b])))
        .await;
    let events: Vec<u64> = reply["result"]["events"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(Value::as_u64)
        .collect();
    home.until_turns(&session, 1).await;
    let log = home.log(&session);
    let started = log
        .iter()
        .find(|event| event.seq.get() == events[0])
        .expect("记下了");
    let Body::TurnStarted(turn) = &started.body else {
        panic!("第一条是开这一轮：{started:?}");
    };
    let triggers: Vec<u64> = turn.triggers.iter().map(|seq| seq.get()).collect();
    assert_eq!(triggers, [a, b], "排好、去重");
    assert_eq!(turn.trigger.map(|seq| seq.get()), Some(b));
    let fact = log
        .iter()
        .find(|event| event.seq.get() == *events.last().unwrap())
        .expect("记下了");
    assert!(
        matches!(&fact.body, Body::ContextInjected(injected) if injected.text == "<judge/>\n"),
        "{fact:?}"
    );
    assert_eq!(fact.by, sessions_admin(), "本机的连接记成管理员");
    // 同一个请求编号再发只算一次，回应一样。
    let again = client
        .call("r1", "session.respond", respond(&session, json!([b, a])))
        .await;
    assert_eq!(again["result"], reply["result"]);
}

#[tokio::test]
async fn bad_requests_record_nothing_and_refusals_name_the_messages() {
    let home = Home::new();
    let (mut client, session, seqs) =
        overheard_twice(&home, &Script::new([Play::Says("在。")])).await;
    let before = home.log(&session).len();
    let many: Vec<u64> = (1..=65).collect();
    for (n, params) in [
        json!({"session": session, "to": []}),
        json!({"session": session, "to": many}),
        json!({"session": session, "to": [0]}),
        json!({"session": session, "to": [seqs[0]], "facts": [{"kind": "Not A Kind", "text": "x"}]}),
        json!({"session": session, "to": [seqs[0]], "facts": [{"kind": "judge", "text": "x".repeat(4097)}]}),
        json!({"session": session, "to": [seqs[0]], "colour": "red"}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client.call(&format!("b{n}"), "session.respond", params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{n}：{reply}");
    }
    assert_eq!(home.log(&session).len(), before, "什么都没记");
    let reply = client
        .call(
            "n1",
            "session.respond",
            respond(&session, json!([seqs[0], 1])),
        )
        .await;
    assert_eq!(reason(&reply), Some("not_ambient"), "{reply}");
    assert_eq!(reply["error"]["data"]["messages"], json!([1]));
    client
        .call("r1", "session.respond", respond(&session, json!([seqs[0]])))
        .await;
    home.until_turns(&session, 1).await;
    let reply = client
        .call(
            "n2",
            "session.respond",
            respond(&session, json!([seqs[1], seqs[0]])),
        )
        .await;
    assert_eq!(reason(&reply), Some("already_answered"), "{reply}");
    assert_eq!(reply["error"]["data"]["messages"], json!([seqs[0]]));
}

fn sessions_admin() -> miyu_kernel::origin::By {
    miyu_kernel::origin::By::Person(miyu_kernel::origin::Person::new(alice()))
}
