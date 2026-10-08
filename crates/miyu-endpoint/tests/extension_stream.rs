//! 扩展的状态的推送（施工 9-4 补，`docs/blueprint/extensions.md`「推送」）：真核心拉起真进程，订阅着的连接收到整份和每一次
//! 变化。夹具同 `tests/extensions.rs`（`support/extensions.rs`）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::support::extensions::*;
use crate::support::*;

/// 读到一条 `extension.changed`、它那一项合 `wanted` 为止，交回那一项：别的消息（回应）跳过，最多 60 秒。
async fn until_pushed(client: &mut Client, wanted: impl Fn(&Value) -> bool) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = client
            .next_within(left)
            .await
            .unwrap_or_else(|| panic!("等不到推送"));
        if message["method"] == "extension.changed" && wanted(&message["params"]["entry"]) {
            return message["params"]["entry"].clone();
        }
    }
}

#[tokio::test]
async fn subscribers_get_the_whole_list_then_each_change() {
    let home = Home::new();
    let program = Program::new();
    install(
        &home,
        "echo",
        &program.name(),
        "manual",
        &steps(&["hello", "wait"]),
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let subscribed = client
        .call("sub", "subscribe", json!({"stream": "extensions"}))
        .await;
    // 出厂的清单里也有 `process` 包（施工 O-18 起有桥）：只看测试装的这一个。
    let mine: Vec<_> = subscribed["result"]["extensions"]
        .as_array()
        .unwrap_or_else(|| panic!("{subscribed}"))
        .iter()
        .filter(|one| one["package"] == "echo")
        .collect();
    assert_eq!(
        mine,
        [
            &json!({"package": "echo", "name": "回声", "start": "manual", "on": false, "state": "off", "failures": 0})
        ],
        "回应是整份，名字照连接的语言"
    );
    client
        .line(&json!({"jsonrpc": "2.0", "id": "e", "method": "extension.enable", "params": {"package": "echo"}}).to_string())
        .await;
    let running = until_pushed(&mut client, |entry| entry["state"] == "running").await;
    assert_eq!(
        (&running["package"], &running["on"]),
        (&json!("echo"), &json!(true))
    );
    assert!(running["pid"].as_u64().is_some(), "{running}");
    client
        .line(&json!({"jsonrpc": "2.0", "id": "d", "method": "extension.disable", "params": {"package": "echo"}}).to_string())
        .await;
    let off = until_pushed(&mut client, |entry| entry["on"] == false).await;
    assert_eq!(off["state"], "off", "{off}");
}

#[tokio::test]
async fn the_extensions_stream_takes_no_session_or_after_and_unsubscribing_stops_it() {
    let home = Home::new();
    let program = Program::new();
    install(
        &home,
        "echo",
        &program.name(),
        "manual",
        &steps(&["hello", "wait"]),
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    for params in [
        json!({"stream": "extensions", "after": 0}),
        json!({"stream": "extensions", "session": "s"}),
    ] {
        let refused = client.call("bad", "subscribe", params.clone()).await;
        assert_eq!(reason(&refused), Some("bad_params"), "{params}：{refused}");
    }
    client
        .call("sub", "subscribe", json!({"stream": "extensions"}))
        .await;
    let gone = client
        .call("un", "unsubscribe", json!({"stream": "extensions"}))
        .await;
    assert_eq!(gone["result"], json!({}), "{gone}");
    let enabled = call(&mut client, "extension.enable", "echo").await;
    assert_eq!(enabled["result"]["on"], true, "{enabled}");
    until_state(&mut client, "echo", |one| one["state"] == "running").await;
    while let Some(message) = client.next_within(Duration::from_millis(200)).await {
        assert_ne!(
            message["method"], "extension.changed",
            "取消了就不推：{message}"
        );
    }
    core.stop_extensions().await;
}
