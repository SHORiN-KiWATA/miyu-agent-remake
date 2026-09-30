//! 自动起标题（施工 3-8 五补，`docs/blueprint/protocol.md` 的 `session.meta_changed`）：没起名的会话第一轮答完，订阅着的头
//! 收到起标题的 `model.called`（`purpose: "title"`）和 `session.meta_changed`，`by` 是内核、不带回合编号和 `cause`；
//! `session.list` 里带上这个标题，核心重启以后照样；请求是单独的一次，没有 system、工具面，只喂第一轮；人先起过名的不起。

mod support;

use serde_json::{Value, json};

use miyu_kernel::block::Block;
use miyu_kernel::request::Message;
use miyu_session::testkit::{Play, Script};

use support::*;

/// 读推送，读到会话 `session` 的 `session.meta_changed` 为止，交回读到的事件，照先后。
async fn until_renamed(client: &mut Client, session: &str) -> Vec<Value> {
    let mut events = Vec::new();
    loop {
        let next = client.next().await.expect("没断开");
        if next["method"] != json!("event") || next["params"]["session"] != json!(session) {
            continue;
        }
        let event = next["params"]["event"].clone();
        let done = event["kind"] == json!("session.meta_changed");
        events.push(event);
        if done {
            return events;
        }
    }
}

/// 等会话 `session` 的执行器把手上的一批动作做完：发一个什么都不改的命令，它回了，之前那一批（落了盘以后要起标题的那个
/// 请求）一定已经交给了端口。
async fn settled(client: &mut Client, id: &str, session: &str) {
    let reply = client
        .call(
            id,
            "session.set_meta",
            json!({"session": session, "pinned": false}),
        )
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
}

/// `session.list` 里会话 `session` 那一项。
async fn listed(client: &mut Client, id: &str, session: &str) -> Value {
    let reply = client.call(id, "session.list", json!({})).await;
    reply["result"]["sessions"]
        .as_array()
        .expect("有会话列表")
        .iter()
        .find(|item| item["session"] == json!(session))
        .cloned()
        .unwrap_or_else(|| panic!("列表里有 {session}：{reply}"))
}

#[tokio::test]
async fn the_first_answer_gets_a_title_every_head_sees() {
    let home = Home::new();
    let script =
        Script::new([Play::Says("好。"), Play::Says("嗯。")]).titles([Play::Says(" 打个招呼\n")]);
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    let mut watcher = Client::connect(core.clone());
    watcher.hello().await;
    watcher.subscribe("w1", &session).await;
    client.say("c2", &session, "hi").await;
    let events = until_renamed(&mut watcher, &session).await;
    let [.., called, renamed] = events.as_slice() else {
        panic!("{events:?}");
    };
    assert_eq!(called["kind"], json!("model.called"));
    assert_eq!(called["body"]["purpose"], json!("title"));
    assert_eq!(renamed["body"], json!({"title": "打个招呼"}));
    for event in [called, renamed] {
        assert_eq!(event["by"], json!({"kind": "kernel"}), "{event}");
        assert!(
            event.get("turn").is_none() && event.get("cause").is_none(),
            "{event}"
        );
    }
    let [(upto, request)] = script.titled().try_into().expect("一次起标题的请求");
    assert_eq!(json!(upto.get()), called["body"]["seen"]);
    assert!(request.tools.is_empty() && request.system.is_empty());
    let [Message::User { blocks }] = request.messages.as_slice() else {
        panic!("一条 user：{:?}", request.messages);
    };
    let [Block::Text(text)] = blocks.as_slice() else {
        panic!("一块字：{blocks:?}");
    };
    assert!(
        text.text.starts_with("Write a title of 3 to 7 words")
            && text
                .text
                .ends_with("Conversation:\nUser: hi\n\nAssistant: 好。"),
        "{}",
        text.text
    );
    assert_eq!(
        listed(&mut client, "c3", &session).await["title"],
        json!("打个招呼")
    );
    // 起过了，第二轮答完不再起；核心重启以后列表照样带着。
    watcher.say("c4", &session, "再说一句").await;
    watcher.until_turn_ends(&session).await;
    settled(&mut watcher, "c5", &session).await;
    assert_eq!(script.titled().len(), 1);
    drop((client, watcher, core));
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    assert_eq!(
        listed(&mut client, "c6", &session).await["title"],
        json!("打个招呼")
    );
}

#[tokio::test]
async fn a_session_named_by_a_person_gets_no_title() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]).titles([Play::Says("打个招呼")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let named = client
        .call(
            "c2",
            "session.set_meta",
            json!({"session": session, "title": "我起的"}),
        )
        .await;
    assert_eq!(named["result"], json!({}), "{named}");
    client.subscribe("s1", &session).await;
    client.say("c3", &session, "hi").await;
    client.until_turn_ends(&session).await;
    settled(&mut client, "c5", &session).await;
    assert!(script.titled().is_empty(), "人起过名的不起");
    assert_eq!(
        listed(&mut client, "c6", &session).await["title"],
        json!("我起的")
    );
}
