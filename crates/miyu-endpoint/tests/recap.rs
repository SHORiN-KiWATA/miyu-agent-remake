//! 回顾（施工 3-8 四补，`docs/blueprint/protocol.md` 的 `session.recap`）：协议上要一句回顾，回应是那一句、照到的、是不是
//! 交回的；订阅着的头先收到回顾的 `model.called` 和 `session.recapped`，都不带回合编号，再收到回应；请求是单独的一次，没有
//! system、工具面；中间没有新内容再要一次，交回上一句、不请求；有回合在进行时照收；一个回复都没有的、没写成的，照头的语言
//! 拒绝；会话编号不对的是参数不对，没有这个会话的找不到。

use serde_json::{Value, json};

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, ErrorClass};
use miyu_kernel::request::Message;
use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 第 `k` 次请求里唯一那一条 user 的字：回顾的请求没有 system、工具面。
fn asked(script: &Script, k: usize) -> String {
    let requests = script.requests();
    let request = &requests[k].1;
    assert!(
        request.tools.is_empty() && request.system.is_empty(),
        "没有 system、工具面"
    );
    let [Message::User { blocks }] = request.messages.as_slice() else {
        panic!("一条 user：{:?}", request.messages);
    };
    match blocks.as_slice() {
        [Block::Text(text)] => text.text.clone(),
        other => panic!("一块字：{other:?}"),
    }
}

/// 推送里的事件，照先后。
fn events(pushed: &[Value]) -> Vec<&Value> {
    pushed
        .iter()
        .filter(|push| push["method"] == json!("event"))
        .map(|push| &push["params"]["event"])
        .collect()
}

/// 她最近一次回复的序号。
fn answered(home: &Home, session: &str) -> u64 {
    home.log(session)
        .iter()
        .rfind(|event| matches!(event.body, Body::MessageAssistant(_)))
        .map(|event| event.seq.get())
        .expect("答了")
}

#[tokio::test]
async fn a_recap_over_the_protocol() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says(" 在打招呼。 ")]);
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let answered = answered(&home, &session);
    let mut watcher = Client::connect(core);
    watcher.hello().await;
    watcher.subscribe("w1", &session).await;
    client.subscribe("s1", &session).await;

    let request = json!({"jsonrpc": "2.0", "id": "c3", "method": "session.recap", "params": {"session": session}});
    client.line(&request.to_string()).await;
    let (pushed, reply) = client.until_reply("c3").await;
    assert_eq!(
        reply["result"],
        json!({"text": "在打招呼。", "upto": answered, "cached": false}),
        "{reply}"
    );
    let pushed = events(&pushed);
    let kinds: Vec<&Value> = pushed.iter().map(|event| &event["kind"]).collect();
    assert_eq!(
        kinds,
        [&json!("model.called"), &json!("session.recapped")],
        "先见结果，后见回应"
    );
    for event in &pushed {
        assert!(event.get("turn").is_none(), "不带回合编号：{event}");
        assert_eq!(event["cause"], json!("c3"));
    }
    assert_eq!(pushed[0]["body"]["purpose"], json!("recap"));
    assert_eq!(
        pushed[1]["body"],
        json!({"text": "在打招呼。", "upto": answered})
    );
    // 别的头也收到了。
    for kind in ["model.called", "session.recapped"] {
        let push = watcher.next().await.expect("推给了别的头");
        assert_eq!(push["params"]["event"]["kind"], json!(kind), "{push}");
    }
    let text = asked(&script, 1);
    assert!(
        text.ends_with("Conversation:\nUser: hi\n\nAssistant: 好。"),
        "{text}"
    );

    // 中间没有新内容：交回上一句，不请求。
    let reply = client
        .call("c4", "session.recap", json!({"session": session}))
        .await;
    assert_eq!(
        reply["result"],
        json!({"text": "在打招呼。", "upto": answered, "cached": true}),
        "{reply}"
    );
    assert_eq!(script.requests().len(), 2);
}

/// 有回合在进行时照收：照这时落了盘的，照到的是这一轮那句话；这一轮照常。
#[tokio::test]
async fn a_recap_is_taken_while_a_turn_runs() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Holds, Play::Says("在等她答。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let said = client.say("c3", &session, "再说一句").await;
    let said = said["result"]["events"][0].clone();
    until("请求停住", || script.requests().len() == 2).await;
    let reply = client
        .call("c4", "session.recap", json!({"session": session}))
        .await;
    assert_eq!(
        reply["result"],
        json!({"text": "在等她答。", "upto": said, "cached": false}),
        "{reply}"
    );
    assert!(asked(&script, 2).ends_with("User: 再说一句"));
    let reply = client
        .call(
            "c5",
            "session.interrupt",
            json!({"session": session, "queued": "send"}),
        )
        .await;
    assert!(reply.get("result").is_some(), "那一轮还在：{reply}");
}

#[tokio::test]
async fn nothing_to_recap_and_a_failed_recap_are_refused_in_the_heads_language() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("好。"),
        Play::Fails {
            class: ErrorClass::Unclassified,
            wait_ms: None,
        },
        Play::Fails {
            class: ErrorClass::Unclassified,
            wait_ms: None,
        },
    ]);
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let mut english = Client::connect(core);
    english.hello_without_input().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call("c2", "session.recap", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("nothing_to_recap"), "{reply}");
    assert_eq!(reply["error"]["message"], json!("还没有可回顾的内容"));
    let reply = english
        .call("e1", "session.recap", json!({"session": session}))
        .await;
    assert_eq!(
        reply["error"]["message"],
        json!("There is nothing to recap yet.")
    );
    assert_eq!(home.log(&session).len(), 1, "拒绝的什么都不写");

    client.say("c3", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let reply = client
        .call("c4", "session.recap", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("recap_failed"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        json!("回顾没写成：请求模型出错了。")
    );
    let reply = english
        .call("e2", "session.recap", json!({"session": session}))
        .await;
    assert_eq!(
        reply["error"]["message"],
        json!("The recap could not be written: the model request failed.")
    );
    assert_eq!(script.requests().len(), 3, "没写成的不再来");
}

#[tokio::test]
async fn bad_params_and_a_missing_session() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    for params in [json!({"session": "nope"}), json!({}), json!({"session": 1})] {
        let reply = client.call("c1", "session.recap", params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    }
    let reply = client
        .call(
            "c2",
            "session.recap",
            json!({"session": "01a0f233-cfec-7023-8ed5-2a037a1d5ec8"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
}
