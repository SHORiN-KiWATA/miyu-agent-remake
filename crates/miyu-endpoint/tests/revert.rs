//! 撤销、恢复（施工 4-7 上）：协议上撤掉一轮、再恢复它，回应的是它们产生的事件；没有能恢复的、没有这一轮的，照头的
//! 语言拒绝；回合编号写成 0 的是参数不对。改回文件的那一半在会话的测试里（`miyu-session` 的 tests/restore.rs）。

mod support;

use serde_json::json;

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};

use support::*;

#[tokio::test]
async fn a_turn_is_undone_and_redone_over_the_protocol() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Says("好。")])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let turn = home
        .log(&session)
        .iter()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| event.seq.get())
        .expect("开过一轮");
    let reply = client
        .call(
            "c3",
            "session.revert",
            json!({"session": session, "turn": turn}),
        )
        .await;
    let events = reply["result"]["events"].as_array().expect("有事件");
    assert_eq!(events.len(), 1, "没改过文件，只有撤销那一条：{reply}");
    assert!(matches!(
        home.log(&session).last().map(|event| &event.body),
        Some(Body::TurnReverted(_))
    ));
    let reply = client
        .call("c4", "session.unrevert", json!({"session": session}))
        .await;
    assert_eq!(
        reply["result"]["events"].as_array().map(Vec::len),
        Some(1),
        "{reply}"
    );
    // 再恢复一次：没有能恢复的了。
    let reply = client
        .call("c5", "session.unrevert", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("nothing_to_unrevert"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        json!("没有能恢复的撤销：没撤过，或者撤了以后又开过一轮、压缩过。")
    );
}

#[tokio::test]
async fn a_turn_that_is_not_there_is_refused() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call(
            "c2",
            "session.revert",
            json!({"session": session, "turn": 99}),
        )
        .await;
    assert_eq!(reason(&reply), Some("unknown_turn"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        json!("没有这一轮，或者它已经撤掉了。")
    );
    let reply = client
        .call(
            "c3",
            "session.revert",
            json!({"session": session, "turn": 0}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}

/// 一轮都没有的，撤最后一轮拒绝，照头的语言说。
#[tokio::test]
async fn with_no_turn_there_is_nothing_to_undo() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call("c2", "session.revert", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("nothing_to_revert"), "{reply}");
    assert_eq!(reply["error"]["message"], json!("没有能撤销的回合。"));
}
