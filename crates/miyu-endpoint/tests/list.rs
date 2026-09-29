//! 列出会话（`docs/construction/3-9-miyu-ask（下）.md`）：造会话时标了一次性的，`session.list` 看得出来；
//! 从新到旧；只要一次性的、最多几个。主会话的 `parent` 是 `null`（施工 7-5；子会话的见 `spawn.rs`）。

mod support;

use serde_json::json;

use miyu_session::testkit::Script;
use support::{Client, Home};

#[tokio::test]
async fn sessions_are_listed_newest_first_and_oneshot_ones_can_be_picked() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let oneshot = |id: &'static str| (id, json!({"cwd": "/work", "oneshot": true}));
    let mut made = Vec::new();
    for (id, params) in [oneshot("c1"), oneshot("c2")] {
        let reply = client.call(id, "session.create", params).await;
        made.push(
            reply["result"]["session"]
                .as_str()
                .expect("造出来了")
                .to_string(),
        );
    }
    let (first, second) = (made[0].clone(), made[1].clone());
    // 最新的是个普通会话：只要一次性的，就得跳过它。
    let plain = client.create("c3", "/work").await;

    let reply = client.call("l1", "session.list", json!({})).await;
    assert_eq!(
        reply["result"]["sessions"],
        json!([
            {"session": plain, "oneshot": false, "parent": null},
            {"session": second, "oneshot": true, "parent": null},
            {"session": first, "oneshot": true, "parent": null},
        ]),
        "从新到旧"
    );
    let reply = client
        .call("l2", "session.list", json!({"oneshot": true, "limit": 1}))
        .await;
    assert_eq!(
        reply["result"]["sessions"],
        json!([{"session": second, "oneshot": true, "parent": null}]),
        "最新的一次性会话"
    );
    let reply = client
        .call("l4", "session.list", json!({"oneshot": true}))
        .await;
    assert_eq!(
        reply["result"]["sessions"].as_array().map(Vec::len),
        Some(2)
    );
    let reply = client
        .call("l3", "session.list", json!({"oneshot": "yes"}))
        .await;
    assert_eq!(reply["error"]["code"], json!(-32602), "参数不对：{reply}");
}

#[tokio::test]
async fn an_empty_home_lists_nothing() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = client
        .call("l1", "session.list", json!({"oneshot": true}))
        .await;
    assert_eq!(reply["result"]["sessions"], json!([]));
}
