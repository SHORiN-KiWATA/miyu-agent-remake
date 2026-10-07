//! 列出会话（`docs/construction/3-9-miyu-ask（下）.md`）：造会话时标了一次性的，`session.list` 看得出来；
//! 从新到旧；只要一次性的、最多几个。主会话的 `parent` 是 `null`（施工 7-5；子会话的见 `spawn.rs`）。每一项带工作目录、最近
//! 一次动静，忙的写 `busy`（施工 C-3；忙不忙、工作目录跟着头换、她列会话的见 `sessions.rs`）。

mod support;

use serde_json::json;

use miyu_kernel::time::Timestamp;
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
    let mut listed = reply["result"]["sessions"].clone();
    // 最近一次动静是造会话那一刻，每次跑不一样：先看写法，再拿掉比别的。
    for item in listed.as_array_mut().expect("有会话列表") {
        let when = item
            .as_object_mut()
            .and_then(|item| item.remove("last_active"))
            .expect("每一项都有 last_active");
        let when = when.as_str().expect("是字符串");
        assert!(Timestamp::parse(when).is_ok(), "时刻的写法：{when}");
    }
    assert_eq!(
        listed,
        json!([
            {"session": plain, "oneshot": false, "parent": null, "cwd": "/work", "persona": "engineer"},
            {"session": second, "oneshot": true, "parent": null, "cwd": "/work", "persona": "engineer"},
            {"session": first, "oneshot": true, "parent": null, "cwd": "/work", "persona": "engineer"},
        ]),
        "从新到旧，闲着的不写 busy"
    );
    let reply = client
        .call("l2", "session.list", json!({"oneshot": true, "limit": 1}))
        .await;
    let listed = &reply["result"]["sessions"];
    assert_eq!(listed.as_array().map(Vec::len), Some(1));
    assert_eq!(listed[0]["session"], json!(second), "最新的一次性会话");
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
