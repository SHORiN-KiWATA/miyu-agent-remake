//! 别的 harness 发来的话（施工 7-10，`docs/blueprint/protocol.md` 的 `session.send` 第 6 到 8 条）：带 `from` 的记成
//! `harness`、带着名字，闲着开一轮，她收到的请求里是带标签的那一块；不带的、`null` 照旧是本人；名字去掉控制字符、截到 128
//! 字节；空的、不是字符串的参数不对，什么都没写；正忙时不排进这一轮、打断不撤回；`session.create`、`session.redo` 不理它。

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::origin::By;
use miyu_kernel::request::Message;
use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 日志里人这边的话：`by` 和原话，照先后。
fn said(home: &Home, session: &str) -> Vec<(By, Vec<Block>)> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::MessageUser(message) => Some((event.by, message.blocks)),
            _ => None,
        })
        .collect()
}

/// `name` 报的 `by`。
fn harness(name: &str) -> By {
    serde_json::from_value(json!({"kind": "harness", "name": name})).expect("合写法")
}

/// 本人。
fn person() -> By {
    serde_json::from_value(json!({"kind": "person", "account": "alice"})).expect("合写法")
}

/// 一块字。
fn words(text: &str) -> Vec<Block> {
    vec![Block::Text(Text {
        text: text.to_string(),
    })]
}

/// 带 `from` 说一句。
async fn send_from(client: &mut Client, id: &str, session: &str, from: Value) -> Value {
    client
        .call(
            id,
            "session.send",
            json!({"session": session, "text": "CI 修好了。", "from": from}),
        )
        .await
}

/// 命令 `id` 引起的第一条这种事件。
fn caused<'a>(log: &'a [Event], id: &str, kind: &str) -> &'a Event {
    log.iter()
        .find(|event| {
            event.cause.as_ref().map(|cause| cause.as_str()) == Some(id)
                && event.body.kind() == kind
        })
        .unwrap_or_else(|| panic!("没有 {id} 引起的 {kind}：{log:#?}"))
}

#[tokio::test]
async fn a_message_from_another_harness_is_its_own() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = send_from(&mut client, "c2", &session, json!("claude-code")).await;
    assert_eq!(reply["result"]["events"], json!([2]), "{reply}");
    home.until_turns(&session, 1).await;
    assert_eq!(
        said(&home, &session),
        [(harness("claude-code"), words("CI 修好了。"))]
    );
    let log = home.log(&session);
    let message = caused(&log, "c2", "message.user");
    assert_eq!(message.turn, None, "别处来的，不带回合编号");
    assert!(
        matches!(&caused(&log, "c2", "turn.started").body, Body::TurnStarted(started) if started.trigger == Some(message.seq)),
        "闲着由它开一轮"
    );
    let requests = script.requests();
    let Some(Message::User { blocks }) = requests[0].1.messages.last() else {
        panic!("最后一条是人这边的");
    };
    assert_eq!(
        blocks.last(),
        Some(&Block::Text(Text {
            text: "<agent-message from=\"claude-code\">\nCI 修好了。\n</agent-message>\n"
                .to_string()
        })),
        "她收到的是带标签的那一块"
    );
}

#[tokio::test]
async fn without_from_it_is_the_person() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    send_from(&mut client, "c3", &session, Value::Null).await;
    home.until_turns(&session, 2).await;
    let by: Vec<By> = said(&home, &session)
        .into_iter()
        .map(|(by, _)| by)
        .collect();
    assert_eq!(by, [person(), person()], "不写的、写 null 的照旧是本人");
}

#[tokio::test]
async fn the_name_loses_control_characters_and_is_cut_to_128_bytes() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    send_from(
        &mut client,
        "c2",
        &session,
        json!("\u{1b}[31mclaude\t-code\n"),
    )
    .await;
    home.until_turns(&session, 1).await;
    // 「界」三个字节：43 个是 129 字节，截到 42 个，不截断一个字。
    send_from(&mut client, "c3", &session, json!("界".repeat(43))).await;
    home.until_turns(&session, 2).await;
    let by: Vec<By> = said(&home, &session)
        .into_iter()
        .map(|(by, _)| by)
        .collect();
    assert_eq!(by, [harness("[31mclaude-code"), harness(&"界".repeat(42))]);
}

#[tokio::test]
async fn a_bad_name_is_refused_and_nothing_is_written() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let bad = [
        json!(""),
        json!("\u{7}\n\r"),
        json!(1),
        json!(["claude-code"]),
        json!({"name": "claude-code"}),
        json!(true),
    ];
    for (k, from) in bad.into_iter().enumerate() {
        let reply = send_from(&mut client, &format!("b{k}"), &session, from.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{from}：{reply}");
    }
    assert_eq!(home.log(&session).len(), 1, "只有造会话那一条");
}

#[tokio::test]
async fn while_she_is_busy_it_is_not_queued_and_an_interrupt_keeps_it() {
    let home = Home::new();
    let script = Script::new([Play::Holds]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "跑个长活").await;
    until("请求发出去", || !script.requests().is_empty()).await;
    send_from(&mut client, "c3", &session, json!("claude-code")).await;
    let reply = client
        .call(
            "c4",
            "session.interrupt",
            json!({"session": session, "queued": "return"}),
        )
        .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&session, 1).await;
    let log = home.log(&session);
    assert_eq!(
        caused(&log, "c3", "message.user").turn,
        None,
        "不排进这一轮"
    );
    assert!(
        !log.iter()
            .any(|event| matches!(event.body, Body::MessageWithdrawn(_))),
        "不是人说的话，打断不撤回：{log:#?}"
    );
}

#[tokio::test]
async fn create_and_redo_do_not_take_it() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("再说一遍。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let reply = client
        .call(
            "c1",
            "session.create",
            json!({"cwd": "~", "from": "claude-code"}),
        )
        .await;
    let session = reply["result"]["session"].as_str().unwrap().to_string();
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let reply = client
        .call(
            "c3",
            "session.redo",
            json!({"session": session, "from": "claude-code"}),
        )
        .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&session, 2).await;
    let log = home.log(&session);
    assert_eq!(log[0].by, person(), "会话照旧是本人造的");
    assert_eq!(caused(&log, "c3", "turn.reverted").by, person());
    assert_eq!(
        caused(&log, "c3", "message.user").by,
        person(),
        "重发的照原来的"
    );
}
