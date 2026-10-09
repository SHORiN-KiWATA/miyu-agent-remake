//! `command.catalog`（施工 O-6 补，`docs/construction/O-6-斜杠命令由核心解析（补）.md`）：不带会话列核心认的全部，照名字排，
//! 带别名、参数提示，照连接的语言；带会话的只列这个会话里打了不会被拒的（记忆关着的没有 `remember`）；场所会话不收，写错的
//! 编号是参数不对，没有的会话照找不到说。

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::memories::with_persona;
use crate::support::venues::bound_core;
use crate::support::*;

/// 列出来的名字，照先后。
fn names(reply: &Value) -> Vec<String> {
    reply["result"]["commands"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .map(|command| command["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[tokio::test]
async fn without_a_session_every_command_is_listed_in_the_connections_language() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let reply = client.call("c1", "command.catalog", json!({})).await;
    assert_eq!(
        reply["result"]["commands"],
        json!([
            {"name": "clear", "aliases": ["reset"], "summary": "清空上下文"},
            {"name": "remember", "aliases": [], "summary": "记一条记忆", "argument": "<内容>"},
            {"name": "stop", "aliases": [], "summary": "全部停止"},
            {"name": "workspace", "aliases": [], "summary": "切换工作区", "argument": "<路径>"},
        ]),
        "{reply}"
    );
}

#[tokio::test]
async fn with_a_session_only_what_would_be_accepted_is_listed() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call("c2", "command.catalog", json!({"session": session}))
        .await;
    assert_eq!(
        names(&reply),
        ["clear", "stop", "workspace"],
        "这个会话没开记忆"
    );

    let home = Home::new();
    let mut client = Client::connect(with_persona(&home, &script, Catalog::default()));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call("c2", "command.catalog", json!({"session": session}))
        .await;
    assert_eq!(names(&reply), ["clear", "remember", "stop", "workspace"]);
}

#[tokio::test]
async fn a_venue_session_and_a_bad_id_are_refused() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(bound_core(&home, &script, Catalog::default()));
    client.hello().await;
    let made = client
        .call(
            "v1",
            "venue.session",
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001"}),
        )
        .await;
    let session = made["result"]["session"].as_str().expect("有编号");
    let reply = client
        .call("c1", "command.catalog", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("venue_session"), "{reply}");
    let reply = client
        .call("c2", "command.catalog", json!({"session": "nope"}))
        .await;
    assert_eq!(reply["error"]["code"], json!(-32602), "{reply}");
    let missing = "01900000-0000-7000-8000-00000000abcd";
    let reply = client
        .call("c3", "command.catalog", json!({"session": missing}))
        .await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
}
