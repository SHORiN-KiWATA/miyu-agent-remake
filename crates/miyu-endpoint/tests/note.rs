//! `session.note`（施工 O-14 补，`docs/construction/O-14-记几块事实（补）.md`）：只记几块事实、不开回合。空闲的不带回合编号，
//! 下一轮的请求里排在触发前面；正在跑一轮的带这一轮的回合编号；写错的、空的、太多的什么都不记；同一个请求编号再发只算一次。

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Event};
use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 一块退信的事实。
fn note(session: &str) -> Value {
    json!({"session": session, "facts": [{"kind": "failed", "text": "<upload-failed/>\n"}]})
}

/// 回应里的序号。
fn seqs(reply: &Value) -> Vec<u64> {
    reply["result"]["events"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(Value::as_u64)
        .collect()
}

/// 日志里序号是 `seq` 的那一条。
fn event(log: &[Event], seq: u64) -> &Event {
    log.iter()
        .find(|event| event.seq.get() == seq)
        .unwrap_or_else(|| panic!("记下了 {seq}"))
}

#[tokio::test]
async fn a_note_while_idle_opens_no_turn_and_comes_before_the_next_trigger() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client.call("n1", "session.note", note(&session)).await;
    let noted = seqs(&reply);
    assert_eq!(noted.len(), 1, "{reply}");
    let log = home.log(&session);
    let fact = event(&log, noted[0]);
    assert!(
        matches!(&fact.body, Body::ContextInjected(injected) if injected.text == "<upload-failed/>\n"),
        "{fact:?}"
    );
    assert_eq!(fact.turn, None, "空闲的不带回合编号");
    assert!(
        !log.iter()
            .any(|event| matches!(event.body, Body::TurnStarted(_))),
        "不开回合"
    );
    let again = client.call("n1", "session.note", note(&session)).await;
    assert_eq!(again["result"], reply["result"], "同一个请求编号只记一次");
    client.say("s1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    let request = format!("{:?}", script.requests()[0].1);
    let (Some(fact), Some(said)) = (request.find("<upload-failed/>"), request.find("在吗"))
    else {
        panic!("请求里两样都有：{request}");
    };
    assert!(fact < said, "排在触发前面：{request}");
    assert_eq!(
        request.matches("<upload-failed/>").count(),
        1,
        "只记了一次：{request}"
    );
}

#[tokio::test]
async fn a_note_during_a_turn_carries_the_turn() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Holds])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let said = client.say("s1", &session, "在吗").await;
    assert!(said.get("error").is_none(), "{said}");
    let noted = seqs(&client.call("n1", "session.note", note(&session)).await);
    let log = home.log(&session);
    assert_eq!(
        event(&log, noted[0]).turn.map(|turn| turn.started().get()),
        log.iter()
            .find(|event| matches!(event.body, Body::TurnStarted(_)))
            .map(|event| event.seq.get()),
        "带这一轮的回合编号"
    );
}

#[tokio::test]
async fn bad_notes_record_nothing() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Says("好。")])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let before = home.log(&session).len();
    let fact = json!({"kind": "failed", "text": "x"});
    for (n, params) in [
        json!({"session": session, "facts": []}),
        json!({"session": session, "facts": vec![fact.clone(); 17]}),
        json!({"session": session, "facts": [fact, {"kind": "Not A Kind", "text": "x"}]}),
        json!({"session": session, "facts": [{"kind": "failed", "text": "x".repeat(4097)}]}),
        json!({"session": session, "facts": [{"kind": "failed", "text": "x"}], "colour": "red"}),
        json!({"session": "nope", "facts": [{"kind": "failed", "text": "x"}]}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client.call(&format!("b{n}"), "session.note", params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{n}：{reply}");
    }
    assert_eq!(home.log(&session).len(), before, "什么都没记");
    let most =
        json!({"session": session, "facts": vec![json!({"kind": "failed", "text": "x"}); 16]});
    let reply = client.call("m1", "session.note", most).await;
    assert_eq!(seqs(&reply).len(), 16, "十六块收：{reply}");
}
