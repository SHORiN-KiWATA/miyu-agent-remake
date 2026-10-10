//! 软件包列表的推送（施工 F-8 三补，`docs/blueprint/protocol.md`「软件包列表的推送」）：订阅的回应是整份列表，之后别的连接
//! 装、卸、装回，列表里变了的那一项照这个连接的语言推过来；卸没了的推 `entry: null`。取消了就不推。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 一个界面包。
const PANE: &str =
    "[package]\nprotocol = [1, 1]\nname = { en = \"Pane\", zh = \"面板\" }\n\n[ui]\n";

/// 读到一条 `package.changed`、编号是 `id` 为止，交回它的 `entry`：别的消息跳过，最多 60 秒。
async fn changed(client: &mut Client, id: &str) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = client
            .next_within(left)
            .await
            .unwrap_or_else(|| panic!("等不到 {id} 的推送"));
        if message["method"] == "package.changed" && message["params"]["package"] == id {
            return message["params"]["entry"].clone();
        }
    }
}

#[tokio::test]
async fn subscribers_get_the_list_then_each_package_that_changed() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut watching = Client::connect(Arc::clone(&core));
    watching.hello().await;
    let mut acting = Client::connect(Arc::clone(&core));
    acting.hello().await;
    let subscribed = watching
        .call("s", "subscribe", json!({"stream": "packages"}))
        .await;
    let listed = subscribed["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{subscribed}"));
    assert!(
        listed.iter().any(|one| one["package"] == "basesystem"),
        "回应是整份列表"
    );

    let folder = home.work.join("pane");
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(folder.join("package.toml"), PANE).expect("写得进");
    let installed = acting
        .call("i", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(installed["result"]["package"], "pane", "{installed}");
    let entry = changed(&mut watching, "pane").await;
    assert_eq!(
        (entry["name"].clone(), entry["layer"].clone()),
        (json!("面板"), json!("home")),
        "照这个连接的语言：{entry}"
    );

    acting
        .call("r", "package.remove", json!({"package": "pane"}))
        .await;
    assert_eq!(
        changed(&mut watching, "pane").await,
        Value::Null,
        "卸没了的推 null"
    );
    let removed = acting
        .call("r", "package.remove", json!({"package": "net"}))
        .await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    assert_eq!(
        changed(&mut watching, "net").await["removed"],
        true,
        "出厂的照样列着，标卸掉了"
    );
    acting
        .call("i", "package.install", json!({"package": "net"}))
        .await;
    assert!(
        changed(&mut watching, "net").await.get("removed").is_none(),
        "装回来了"
    );

    let off = watching
        .call("u", "unsubscribe", json!({"stream": "packages"}))
        .await;
    assert_eq!(off["result"], json!({}), "{off}");
    acting
        .call("r", "package.remove", json!({"package": "net"}))
        .await;
    while let Some(message) = watching.next_within(Duration::from_millis(200)).await {
        assert_ne!(
            message["method"], "package.changed",
            "取消了就不推：{message}"
        );
    }
}

#[tokio::test]
async fn the_packages_stream_needs_a_hello_and_takes_no_after() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut client = Client::connect(Arc::clone(&core));
    let early = client
        .call("s", "subscribe", json!({"stream": "packages"}))
        .await;
    assert_eq!(early["error"]["data"]["reason"], "hello_first", "{early}");
    client.hello().await;
    let after = client
        .call("s", "subscribe", json!({"stream": "packages", "after": 3}))
        .await;
    assert_eq!(after["error"]["data"]["reason"], "bad_params", "{after}");
}
