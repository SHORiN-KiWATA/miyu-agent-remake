//! 斜杠命令 `/remember`（施工 R-3 补，`docs/blueprint/memory.md`「协议」）：真核心走一遍。记进这个会话那一间，`by` 是打命令
//! 的人、出处空，回执照连接的语言带编号，记一条 `command.ran`；不请求模型。空的、超长的、记忆关着的、场所会话里的不记。

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Event};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::memories::with_persona;
use crate::support::venues::bound_core;
use crate::support::*;

async fn run(client: &mut Client, id: &str, session: &str, text: &str) -> Value {
    client
        .call(id, "command.run", json!({"session": session, "text": text}))
        .await
}

fn noted(log: &[Event]) -> Vec<String> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::CommandRan(ran) => Some(ran.command.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn remember_saves_in_the_sessions_room_without_asking_the_model() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(with_persona(&home, &script, Catalog::default()));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = run(&mut client, "k1", &session, "/remember   用户喜欢猫  ").await;
    assert_eq!(reply["result"]["command"], "remember", "{reply}");
    assert_eq!(reply["result"]["said"], "记下了：m1。");
    assert_eq!(noted(&home.log(&session)), ["remember"]);
    assert!(script.requests().is_empty(), "不请求模型");
    let again = run(&mut client, "k1", &session, "/remember   用户喜欢猫  ").await;
    assert_eq!(again["result"], reply["result"], "同一个编号只算一次");
    let listed = client
        .call("l1", "memory.list", json!({"session": session}))
        .await;
    assert_eq!(
        listed["result"]["memories"].as_array().map(Vec::len),
        Some(1),
        "重发的没多记"
    );
    let memory = &listed["result"]["memories"][0];
    assert_eq!(
        (
            &memory["id"],
            &memory["text"],
            &memory["class"],
            &memory["by"]
        ),
        (
            &json!("m1"),
            &json!("用户喜欢猫"),
            &json!("user"),
            &json!("person")
        ),
        "{listed}"
    );
    assert_eq!(memory["sources"], json!([]));

    let long = format!("/remember {}", "长".repeat(121));
    for (id, text, why) in [
        ("k2", "/remember", "bad_params"),
        ("k3", "/remember    ", "bad_params"),
        ("k4", long.as_str(), "memory_too_long"),
    ] {
        let reply = run(&mut client, id, &session, text).await;
        assert_eq!(reason(&reply), Some(why), "{text}：{reply}");
    }
    assert_eq!(noted(&home.log(&session)), ["remember"], "被拒的不记");

    let off = client
        .call("c2", "session.create", json!({"cwd": "~", "memory": "off"}))
        .await;
    let off = off["result"]["session"].as_str().expect("造了").to_string();
    let reply = run(&mut client, "k5", &off, "/remember 用户喜欢猫").await;
    assert_eq!(reason(&reply), Some("memory_unavailable"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        "这个会话里没有记忆：记忆关着，或者是通讯平台的会话。"
    );
}

#[tokio::test]
async fn in_a_venue_remember_waits_for_the_o_line() {
    let home = Home::new();
    let script = Script::new([Play::Says("在。")]);
    let mut client = Client::connect(bound_core(&home, &script, Catalog::default()));
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
    let owner = json!({"external": "qq:10001"});
    let reply = client
        .call(
            "k1",
            "command.run",
            json!({"session": session, "text": "/remember 用户喜欢猫", "as": owner}),
        )
        .await;
    assert_eq!(reason(&reply), Some("memory_unavailable"), "{reply}");
    assert!(noted(&home.log(&session)).is_empty());
}
