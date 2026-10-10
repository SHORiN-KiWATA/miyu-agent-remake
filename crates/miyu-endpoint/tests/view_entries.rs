//! 视图投影接上协议（施工 9-8 中，`docs/blueprint/view.md`）：握手报 `view: 1`；`view.page` 写了 `view: true` 的交条目、
//! 不交事件，字照连接的语言，页的边界照旧；`view.detail` 多交调用的结果原文，另认 `compaction` 交那一次压缩的摘要。

use std::path::Path;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

fn tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

/// 读一个文件再说一句：交回客户端和会话。
async fn read_then_say(home: &Home) -> (Client, String) {
    std::fs::write(home.work.join("a.txt"), "hello\n").expect("写得进工作区");
    let script = Script::new([
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::Says("读完了。"),
    ]);
    let mut client = Client::connect(home.core_with_tools(&script, tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "读一下").await;
    home.until_turns(&session, 1).await;
    (client, session)
}

fn kinds(entries: &Value) -> Vec<&str> {
    entries
        .as_array()
        .unwrap_or_else(|| panic!("{entries}"))
        .iter()
        .filter_map(|entry| entry["kind"].as_str())
        .collect()
}

#[tokio::test]
async fn hello_says_the_view_version() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    let reply = client.hello().await;
    assert_eq!(reply["result"]["view"], json!(1), "{reply}");
}

#[tokio::test]
async fn a_page_of_entries_is_drawn_in_the_connections_language() {
    let home = Home::new();
    let (mut client, session) = read_then_say(&home).await;
    let events = client
        .call("p1", "view.page", json!({"session": session}))
        .await;
    let entries = client
        .call("p2", "view.page", json!({"session": session, "view": true}))
        .await;
    let result = &entries["result"];
    assert!(
        result.get("events").is_none(),
        "交条目就不交事件：{entries}"
    );
    for key in ["first", "last", "more"] {
        assert_eq!(result[key], events["result"][key], "页的边界照旧：{key}");
    }
    assert_eq!(
        kinds(&result["entries"]),
        ["user", "group", "tool", "reply", "end"],
        "{entries}"
    );
    let tool = &result["entries"][2];
    assert_eq!(tool["state"], "ok", "{tool}");
    assert_eq!(tool["title"]["name"], "读取", "握手说的中文：{tool}");
    assert_eq!(result["entries"][3]["text"], "读完了。");
}

#[tokio::test]
async fn detail_brings_the_output_of_the_call() {
    let home = Home::new();
    let (mut client, session) = read_then_say(&home).await;
    let call = home
        .log(&session)
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => Some(result.call_id.to_string()),
            _ => None,
        })
        .expect("调了工具");
    let reply = client
        .call(
            "d",
            "view.detail",
            json!({"session": session, "call": call}),
        )
        .await;
    assert_eq!(reply["result"]["files"], json!([]), "读文件的没有改动");
    assert!(
        reply["result"]["output"]
            .as_str()
            .is_some_and(|output| output.contains("hello")),
        "{reply}"
    );
}

#[tokio::test]
async fn detail_brings_the_summary_of_a_compaction() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("<analysis>a</analysis>\n<summary>\nThe user said hi.\n</summary>"),
    ])
    .window(1_000_000);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, &"x".repeat(70_000)).await;
    home.until_turns(&session, 1).await;
    client
        .call("c3", "session.compact", json!({"session": session}))
        .await;
    home.until_turns(&session, 2).await;
    let upto = home
        .log(&session)
        .into_iter()
        .find_map(|event| match event.body {
            Body::ContextCompacted(compacted) => Some(compacted.upto.get()),
            _ => None,
        })
        .expect("压了");
    let reply = client
        .call(
            "d1",
            "view.detail",
            json!({"session": session, "compaction": upto}),
        )
        .await;
    assert_eq!(
        reply["result"],
        json!({"summary": "The user said hi."}),
        "{reply}"
    );
    let missing = client
        .call(
            "d2",
            "view.detail",
            json!({"session": session, "compaction": upto + 1_000}),
        )
        .await;
    assert_eq!(reason(&missing), Some("unknown_call"), "{missing}");
    let both = client
        .call(
            "d3",
            "view.detail",
            json!({"session": session, "compaction": upto, "call": "call_2_1"}),
        )
        .await;
    assert_eq!(reason(&both), Some("bad_params"), "{both}");
}
