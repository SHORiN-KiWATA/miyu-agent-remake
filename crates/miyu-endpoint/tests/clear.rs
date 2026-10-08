//! 清空上下文（施工 6-8 补，`docs/blueprint/protocol.md` 的 `session.clear`）：协议上清一次，回应是单开的那一轮的开头，推送
//! 里是那一轮的开头、空的检查点、结束，下一次请求里没有清空以前的；撤掉那一轮，回应数的是清空、不是压缩，上下文回来了。
//! 有回合在进行、上下文本来就是空的，照头的语言拒绝；会话编号不对的是参数不对。

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, CompactTrigger};
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 一次请求里人这边说的每一段字，照先后。
fn user_texts(request: &Request) -> Vec<String> {
    request
        .messages
        .iter()
        .filter_map(|message| match message {
            Message::User { blocks } => Some(blocks),
            _ => None,
        })
        .flatten()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_session_is_cleared_over_the_protocol() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("不知道。"),
        Play::Says("你说了 hi。"),
    ]);
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let mut watcher = Client::connect(core);
    watcher.hello().await;
    watcher.subscribe("w1", &session).await;

    let reply = client
        .call("c3", "session.clear", json!({"session": session}))
        .await;
    let log = home.log(&session);
    let started = log
        .iter()
        .rfind(|event| matches!(event.body, Body::TurnStarted(_)))
        .expect("开了一轮");
    assert_eq!(
        reply["result"],
        json!({"events": [started.seq.get()]}),
        "{reply}"
    );
    assert_eq!(started.cause.as_ref().map(|id| id.as_str()), Some("c3"));
    let compacted = log
        .iter()
        .find_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some((event.seq.get(), compacted)),
            _ => None,
        })
        .expect("写了检查点");
    assert_eq!(compacted.0, started.seq.get() + 1, "开头后面就是检查点");
    let compacted = compacted.1;
    assert_eq!(compacted.trigger, Some(CompactTrigger::Clear));
    assert_eq!(compacted.upto.get(), started.seq.get() - 1);
    assert!(compacted.summary.is_empty());
    let pushed = watcher.until_turn_ends(&session).await;
    assert_eq!(
        kinds(&pushed),
        ["turn.started", "context.compacted", "turn.ended"]
    );
    assert_eq!(script.requests().len(), 1, "不请求模型");

    // 下一轮看不到清空以前的。
    client.say("c4", &session, "刚才说了什么？").await;
    home.until_turns(&session, 3).await;
    let requests = script.requests();
    let texts = user_texts(&requests[1].1);
    assert_eq!(texts.last().map(String::as_str), Some("刚才说了什么？"));
    assert!(
        texts
            .iter()
            .all(|text| text != "hi" && !text.contains("<conversation-checkpoint")),
        "{texts:?}"
    );
    assert!(
        !requests[1]
            .1
            .messages
            .iter()
            .any(|message| matches!(message, Message::Assistant { .. })),
        "她以前的回复也清掉了"
    );

    // 撤掉清空那一轮（连同后来那一轮）：撤掉了一次清空，不算压缩；那一轮没有触发，回应里没有人说的话；再问，看得到了。
    let reply = client
        .call(
            "c5",
            "session.revert",
            json!({"session": session, "turn": started.seq.get()}),
        )
        .await;
    let result = &reply["result"];
    assert_eq!(result["turns"], json!(2), "{reply}");
    assert_eq!(result["clears"], json!(1), "{reply}");
    assert!(result.get("compactions").is_none(), "{reply}");
    assert!(result.get("said").is_none(), "{reply}");
    client.say("c6", &session, "刚才说了什么？").await;
    home.until_turns(&session, 4).await;
    let requests = script.requests();
    assert!(
        user_texts(&requests[2].1).iter().any(|text| text == "hi"),
        "{:?}",
        user_texts(&requests[2].1)
    );
}

#[tokio::test]
async fn nothing_to_clear_is_refused_in_the_heads_language() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call("c2", "session.clear", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("nothing_to_clear"), "{reply}");
    assert_eq!(reply["error"]["message"], json!("上下文为空"));
    let mut english = Client::connect(core);
    english.hello_without_input().await;
    let reply = english
        .call("e1", "session.clear", json!({"session": session}))
        .await;
    assert_eq!(reply["error"]["message"], json!("The context is empty."));
    assert_eq!(home.log(&session).len(), 1, "拒绝的什么都不写");
}

#[tokio::test]
async fn a_running_turn_refuses_a_clear() {
    let home = Home::new();
    let script = Script::new([Play::Holds]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    until("请求停住", || script.requests().len() == 1).await;
    let reply = client
        .call("c3", "session.clear", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("turn_running"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        json!("有回合在进行：先打断，或者等它做完。")
    );
}

#[tokio::test]
async fn a_bad_session_is_bad_params() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = client
        .call("c1", "session.clear", json!({"session": "nope"}))
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    let reply = client.call("c2", "session.clear", json!({})).await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}
