//! 重做（施工 4-7 再补，`docs/blueprint/protocol.md` 的 `session.redo`）：协议上重做一次，回应照撤销写、`events` 最后是重发的
//! 那一句；推送里是一批撤销、原话、新的一轮，新的一轮的请求和撤掉的那一轮的一字不差；换了话的推送里是新的话，`said` 是原来
//! 的；改过文件的先改回；重做以后恢复不了。一轮都没有、最后一轮是清空的、有回合在进行、换成空的，照头的语言拒绝；`text`
//! 不是字符串、会话编号不对的是参数不对，写 `null` 当没写。附件照 `session.send` 传：不写的照带原来的，写了的换掉，空的是不要，
//! 核心里没有的拒绝。

mod support;

use std::path::Path;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, MessageUser};
use miyu_session::testkit::{Play, Script};

use support::*;

/// 回应里的 `result`。
fn result_of(reply: &Value) -> &Value {
    assert!(reply.get("error").is_none(), "{reply}");
    &reply["result"]
}

/// 会话里 `id` 那条命令开的一轮的开头：它的序号。
fn opened_by(home: &Home, session: &str, id: &str) -> u64 {
    home.log(session)
        .iter()
        .find(|event| {
            matches!(event.body, Body::TurnStarted(_))
                && event.cause.as_ref().map(|cause| cause.as_str()) == Some(id)
        })
        .map(|event| event.seq.get())
        .unwrap_or_else(|| panic!("{id} 没开一轮"))
}

/// 会话日志里第 `seq` 条人的话，写成字。
fn words_at(home: &Home, session: &str, seq: u64) -> Value {
    let log = home.log(session);
    let event = log
        .iter()
        .find(|event| event.seq.get() == seq)
        .expect("有这一条");
    match &event.body {
        Body::MessageUser(MessageUser { blocks }) => {
            serde_json::to_value(blocks).expect("写得成 JSON")
        }
        body => panic!("第 {seq} 条不是人的话：{body:?}"),
    }
}

#[tokio::test]
async fn a_turn_is_redone_over_the_protocol() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("又好。")]);
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
        .call("c3", "session.redo", json!({"session": session}))
        .await;
    let result = result_of(&reply);
    let events: Vec<u64> = result["events"]
        .as_array()
        .expect("是数组")
        .iter()
        .map(|seq| seq.as_u64().expect("是序号"))
        .collect();
    assert_eq!(events.len(), 2, "撤销、重发的那一句：{result}");
    assert_eq!(result["turns"], json!(1));
    assert_eq!(result["said"], json!("hi"));
    assert_eq!(result["commands"], json!(0));
    assert_eq!(result["files"], json!([]));
    assert!(result["cwd"].is_string(), "{result}");
    assert_eq!(
        words_at(&home, &session, events[1]),
        json!([{"type": "text", "text": "hi"}])
    );
    let opened = opened_by(&home, &session, "c3");
    assert_eq!(opened, events[1] + 1, "由重发的那一句开一轮");
    let pushed = watcher.until_turn_ends(&session).await;
    assert_eq!(
        kinds(&pushed)[..3],
        ["turn.reverted", "message.user", "turn.started"]
    );
    // 撤掉的那一轮的第一次请求和新的一轮的一字不差。
    let requests = script.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1].1.canonical_bytes(),
        requests[0].1.canonical_bytes()
    );
    // 新的一轮开了：恢复不了。
    let reply = client
        .call("c4", "session.unrevert", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("nothing_to_unrevert"), "{reply}");
}

#[tokio::test]
async fn new_words_are_said_instead() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("换了也好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let reply = client
        .call(
            "c3",
            "session.redo",
            json!({"session": session, "text": "换个说法"}),
        )
        .await;
    let result = result_of(&reply);
    assert_eq!(result["said"], json!("hi"), "said 是撤掉的那一轮原来那一句");
    let resent = result["events"][1].as_u64().expect("是序号");
    assert_eq!(
        words_at(&home, &session, resent),
        json!([{"type": "text", "text": "换个说法"}])
    );
    // 写 `null` 当没写：原样重做（这回重做的是换过的那一句）。
    let reply = client
        .call(
            "c4",
            "session.redo",
            json!({"session": session, "text": null}),
        )
        .await;
    let resent = result_of(&reply)["events"][1].as_u64().expect("是序号");
    assert_eq!(
        words_at(&home, &session, resent),
        json!([{"type": "text", "text": "换个说法"}])
    );
}

#[tokio::test]
async fn the_files_come_back_before_it_is_said_again() {
    let home = Home::new();
    std::fs::write(home.work.join("a.txt"), "old\n").expect("写得进");
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools =
        miyu_tool::Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
            .expect("合写法");
    // 先读再写：没读过的文件不让写（`tools/write.md`）。
    let script = Script::new([
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[("write", r#"{"file_path":"a.txt","content":"new\n"}"#)]),
        Play::Says("改好了。"),
        Play::Says("这回不改了。"),
    ]);
    let mut client = Client::connect(home.core_with_tools(&script, tools, TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "改一下").await;
    home.until_turns(&session, 1).await;
    assert_eq!(
        std::fs::read_to_string(home.work.join("a.txt")).expect("在"),
        "new\n"
    );
    let reply = client
        .call("c3", "session.redo", json!({"session": session}))
        .await;
    let result = result_of(&reply);
    assert_eq!(
        result["events"].as_array().map(Vec::len),
        Some(3),
        "撤销、改回的结局、重发的那一句：{result}"
    );
    assert_eq!(result["files"][0]["outcome"], json!("restored"), "{result}");
    assert_eq!(
        std::fs::read_to_string(home.work.join("a.txt")).expect("在"),
        "old\n"
    );
    home.until_turns(&session, 2).await;
    assert_eq!(
        opened_by(&home, &session, "c3"),
        result["events"][2].as_u64().expect("是序号") + 1
    );
}

#[tokio::test]
async fn a_turn_not_opened_by_a_person_is_refused_in_the_heads_language() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    // 一轮都没有。
    let reply = client
        .call("c2", "session.redo", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("not_redoable"), "{reply}");
    assert_eq!(reply["error"]["message"], json!("无法重做"));
    // 最后一轮是清空的。
    client.say("c3", &session, "hi").await;
    home.until_turns(&session, 1).await;
    result_of(
        &client
            .call("c4", "session.clear", json!({"session": session}))
            .await,
    );
    let before = home.log(&session).len();
    let reply = client
        .call("c5", "session.redo", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("not_redoable"), "{reply}");
    let mut english = Client::connect(core);
    english.hello_without_input().await;
    let reply = english
        .call("e1", "session.redo", json!({"session": session}))
        .await;
    assert_eq!(reply["error"]["message"], json!("Cannot redo."));
    assert_eq!(home.log(&session).len(), before, "拒绝的什么都不写");
}

#[tokio::test]
async fn a_running_turn_or_an_empty_text_is_refused() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Holds]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let reply = client
        .call(
            "c3",
            "session.redo",
            json!({"session": session, "text": ""}),
        )
        .await;
    assert_eq!(reason(&reply), Some("empty_message"), "{reply}");
    assert_eq!(reply["error"]["message"], json!("消息是空的。"));
    client.say("c4", &session, "再来").await;
    until("请求停住", || script.requests().len() == 2).await;
    let reply = client
        .call("c5", "session.redo", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("turn_running"), "{reply}");
}

#[tokio::test]
async fn bad_params_are_bad_params() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    for (id, params) in [
        ("c2", json!({"session": session, "text": 5})),
        ("c3", json!({"session": "nope"})),
        ("c4", json!({})),
    ] {
        let reply = client.call(id, "session.redo", params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    }
}

/// 一张 `width` × `height` 的 PNG 的开头。
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

/// 传一个文件，交回 `blob.put` 的回应。
async fn put(client: &mut Client, home: &Home, name: &str, bytes: &[u8]) -> Value {
    let path = home.work.join(name);
    std::fs::write(&path, bytes).expect("写得了");
    let reply = client.call("p", "blob.put", json!({"path": path})).await;
    reply["result"].clone()
}

/// 日志里第 `seq` 条人的话里的图，写成宽。
fn widths(home: &Home, session: &str, seq: u64) -> Vec<u64> {
    words_at(home, session, seq)
        .as_array()
        .expect("是数组")
        .iter()
        .filter(|block| block["type"] == json!("image"))
        .map(|block| block["width"].as_u64().expect("有宽"))
        .collect()
}

#[tokio::test]
async fn attachments_go_along_or_are_replaced() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("看到了。"),
        Play::Says("又看了。"),
        Play::Says("换了一张。"),
        Play::Says("只剩字。"),
    ]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let small = put(&mut client, &home, "small.png", &png(8, 6)).await;
    let big = put(&mut client, &home, "big.png", &png(80, 60)).await;
    let session = client.create("c1", "~").await;
    client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "看看", "attachments": [small]}),
        )
        .await;
    home.until_turns(&session, 1).await;
    // 只换话：原来的附件照带。
    let reply = client
        .call(
            "c3",
            "session.redo",
            json!({"session": session, "text": "再看看", "attachments": null}),
        )
        .await;
    let resent = result_of(&reply)["events"][1].as_u64().expect("是序号");
    assert_eq!(
        words_at(&home, &session, resent)[0]["text"],
        json!("再看看")
    );
    assert_eq!(widths(&home, &session, resent), [8]);
    // 换附件：新的那张替掉原来的，话照原来的。
    let reply = client
        .call(
            "c4",
            "session.redo",
            json!({"session": session, "attachments": [big]}),
        )
        .await;
    let resent = result_of(&reply)["events"][1].as_u64().expect("是序号");
    assert_eq!(
        words_at(&home, &session, resent)[0]["text"],
        json!("再看看")
    );
    assert_eq!(widths(&home, &session, resent), [80]);
    // 不要附件：只剩话。
    let reply = client
        .call(
            "c5",
            "session.redo",
            json!({"session": session, "attachments": []}),
        )
        .await;
    let resent = result_of(&reply)["events"][1].as_u64().expect("是序号");
    assert_eq!(
        words_at(&home, &session, resent),
        json!([{"type": "text", "text": "再看看"}])
    );
    // 这个核心里没有的 blob：照 `session.send` 拒绝，什么都不写。
    let before = home.log(&session).len();
    let mut missing = small.clone();
    missing["blob"] = json!(format!("sha256:{}", "0".repeat(64)));
    let reply = client
        .call(
            "c6",
            "session.redo",
            json!({"session": session, "attachments": [missing]}),
        )
        .await;
    assert_eq!(reason(&reply), Some("unknown_attachment"), "{reply}");
    assert_eq!(home.log(&session).len(), before);
}
