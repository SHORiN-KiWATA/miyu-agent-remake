//! 场所的格、事件和 `events.append`（施工 O-13 上，`docs/construction/O-13-场所的格、事件和 events.append（上）.md`）：
//! `session.send` 带 `venue` 原样记进 `message.user`、旁听的不开回合、写错的什么都不记；`events.append` 记扩展自己的
//! `ext.*` 和核心认得的 `venue.recalled`、`venue.delivered`，不带回合编号、回应交序号；种类、大小、格不对的拒。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::venues::bound_core;
use crate::support::*;

/// 照对应表起来的核心，连上、握手，找到主人（`qq:10001`）的私聊，交回连接和会话编号。
async fn owners_chat(home: &Home, script: &Script) -> (Client, String) {
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
        .unwrap_or_else(|| panic!("{made}"))
        .to_string();
    (client, session)
}

/// 代表主人说一句，带上 `venue`。
fn said(session: &str, venue: Value) -> Value {
    json!({"session": session, "text": "今天谁值班", "as": {"external": "qq:10001"}, "venue": venue})
}

#[tokio::test]
async fn a_venue_message_is_kept_as_written_and_an_ambient_one_opens_no_turn() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home, &Script::new([Play::Says("我来。")])).await;
    let venue = json!({"msg": "8810", "reply_to": "8800", "name": "小林", "mentions": ["qq:20017"],
        "mentions_me": true, "mentions_all": false,
        "media": [{"kind": "file", "id": "f-1", "name": "排班.pdf"}, {"kind": "voice", "id": "v-1"}],
        "ambient": true, "asleep": false, "show_ids": true});
    let reply = client
        .call("s1", "session.send", said(&session, venue))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    // 旁听的：等一会儿，日志里还是只有这一条，没开回合。
    tokio::time::sleep(Duration::from_millis(200)).await;
    let log = home.log(&session);
    let kept = log.last().expect("记下了");
    assert_eq!(kept.turn, None);
    let Body::MessageUser(message) = &kept.body else {
        panic!("应该是 message.user：{kept:?}");
    };
    let written = serde_json::to_value(message.venue.as_ref().expect("有 venue")).expect("写得出");
    assert_eq!(
        written,
        json!({"msg": "8810", "reply_to": "8800", "name": "小林", "mentions": ["qq:20017"],
            "mentions_me": true, "media": [{"kind": "file", "id": "f-1", "name": "排班.pdf"}, {"kind": "voice", "id": "v-1"}],
            "ambient": true, "show_ids": true}),
        "原样记下，假的不写"
    );
    assert!(
        log.iter()
            .all(|event| !matches!(event.body, Body::TurnStarted(_))),
        "旁听的不开回合"
    );
    // 不旁听的照常开回合。
    let reply = client
        .call("s2", "session.send", said(&session, json!({"msg": "8811"})))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 1).await;
}

#[tokio::test]
async fn a_bad_venue_records_nothing() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home, &Script::new([])).await;
    let before = home.log(&session).len();
    let long = "名".repeat(65);
    for (n, params) in [
        json!({"session": session, "text": "hi", "venue": {"msg": "1"}}),
        said(&session, json!({})),
        said(&session, json!({"msg": ""})),
        said(&session, json!({"msg": "1", "name": long})),
        said(&session, json!({"msg": "1", "mentions": [""]})),
        said(
            &session,
            json!({"msg": "1", "media": [{"kind": "gif", "id": "x"}]}),
        ),
        said(&session, json!({"msg": "1", "media": [{"kind": "file"}]})),
        said(&session, json!({"msg": "1", "colour": "red"})),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client.call(&format!("b{n}"), "session.send", params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{n}：{reply}");
    }
    assert_eq!(home.log(&session).len(), before, "什么都没记");
}

#[tokio::test]
async fn events_append_takes_ext_and_the_two_venue_kinds() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home, &Script::new([])).await;
    let append = |id: &str, kind: &str, body: Value| {
        (
            id.to_string(),
            json!({"session": session, "kind": kind, "body": body}),
        )
    };
    let mut seqs = Vec::new();
    for (id, params) in [
        append(
            "a1",
            "ext.onebot.chat.decided",
            json!({"to": [3], "why": "被点名"}),
        ),
        append(
            "a2",
            "venue.recalled",
            json!({"msg": "8810", "by": "qq:20017"}),
        ),
        append(
            "a3",
            "venue.delivered",
            json!({"line": session, "turn": 5, "to": ["qq:10001"], "msg": "8811", "text": "我来",
                   "images": ["sha256:5f70bf18a086007016e948b04aed3b82103a36bea41755b6cddfaf10ace3c6ef"]}),
        ),
    ] {
        let reply = client.call(&id, "events.append", params).await;
        let seq = reply["result"]["seq"]
            .as_u64()
            .unwrap_or_else(|| panic!("{id}：{reply}"));
        seqs.push(seq);
    }
    let log = home.log(&session);
    let kinds: Vec<(u64, &str, bool)> = log
        .iter()
        .filter(|event| seqs.contains(&event.seq.get()))
        .map(|event| (event.seq.get(), event.body.kind(), event.turn.is_none()))
        .collect();
    assert_eq!(
        kinds,
        [
            (seqs[0], "ext.onebot.chat.decided", true),
            (seqs[1], "venue.recalled", true),
            (seqs[2], "venue.delivered", true),
        ]
    );
}

#[tokio::test]
async fn events_append_refuses_wrong_kinds_sizes_and_fields() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home, &Script::new([])).await;
    let local = client.create("c1", "~").await;
    let big = "x".repeat(16 * 1024);
    let before = home.log(&session).len();
    for (n, params) in [
        json!({"session": session, "kind": "message.user", "body": {}}),
        json!({"session": session, "kind": "ext.onebot", "body": {}}),
        json!({"session": session, "kind": "ext.Onebot.x", "body": {}}),
        json!({"session": session, "kind": "ext.onebot..x", "body": {}}),
        json!({"session": session, "kind": format!("ext.onebot.{}", "a".repeat(130)), "body": {}}),
        json!({"session": session, "kind": "ext.onebot.x", "body": [1, 2]}),
        json!({"session": session, "kind": "ext.onebot.x", "body": {"text": big}}),
        json!({"session": session, "kind": "venue.recalled", "body": {"msg": "1"}}),
        json!({"session": session, "kind": "venue.recalled", "body": {"msg": "1", "by": "qq:1", "extra": 1}}),
        json!({"session": session, "kind": "venue.delivered", "body": {"line": "nope", "turn": 1, "to": [], "msg": "1", "text": ""}}),
        json!({"session": local, "kind": "venue.recalled", "body": {"msg": "1", "by": "qq:1"}}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client.call(&format!("r{n}"), "events.append", params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{n}：{reply}");
    }
    assert_eq!(home.log(&session).len(), before, "什么都没记");
    // 本机的会话也收扩展自己的。
    let reply = client
        .call(
            "ok",
            "events.append",
            json!({"session": local, "kind": "ext.notes.seen", "body": {}}),
        )
        .await;
    assert!(reply["result"]["seq"].is_u64(), "{reply}");
}
