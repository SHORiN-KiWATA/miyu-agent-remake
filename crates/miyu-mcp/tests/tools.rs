//! 列工具：翻页列全、照先后；写法不对的不收、说哪一项；新时代的 `ttlMs`；服务说变了的数一次。

use std::time::Duration;

use serde_json::json;

use miyu_mcp::testkit::{Kind, Script, tools};

use crate::support::connected;

#[tokio::test]
async fn pages_are_followed_to_the_end() {
    let script = Script {
        page: 3,
        ..Script::legacy(Kind::LegacyRefuses)
    };
    let (client, fake) = connected(script).await;
    let listed = client.tools().await.expect("列得出");
    let names: Vec<&str> = listed.tools.iter().map(|tool| tool.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "echo",
            "picture",
            "fail",
            "structured",
            "slow",
            "ask",
            "input",
            "die"
        ]
    );
    let cursors: Vec<_> = fake
        .received()
        .iter()
        .filter(|message| message["method"] == "tools/list")
        .map(|message| message["params"]["cursor"].clone())
        .collect();
    assert_eq!(cursors, [json!(null), json!("3"), json!("6")]);
    assert_eq!(listed.fresh_for, None);
    assert!(listed.tools[0].annotations.read_only == Some(true));
}

#[tokio::test]
async fn a_broken_tool_is_skipped_and_named() {
    let mut list = tools();
    list.insert(1, json!({"name": "broken"}));
    let script = Script {
        tools: list,
        ..Script::modern()
    };
    let (client, _) = connected(script).await;
    let listed = client.tools().await.expect("列得出");
    assert_eq!(listed.tools.len(), 8);
    assert_eq!(listed.skipped.len(), 1);
    assert!(listed.skipped[0].contains("broken"), "{:?}", listed.skipped);
}

#[tokio::test]
async fn a_modern_list_says_how_long_it_stays_fresh() {
    let script = Script {
        ttl_ms: Some(60_000),
        ..Script::modern()
    };
    let (client, _) = connected(script).await;
    let listed = client.tools().await.expect("列得出");
    assert_eq!(listed.fresh_for, Some(Duration::from_secs(60)));
}

#[tokio::test]
async fn a_change_is_counted() {
    let script = Script {
        change_after_list: true,
        ..Script::legacy(Kind::LegacyRefuses)
    };
    let (client, _) = connected(script).await;
    let mut changes = client.changes();
    assert_eq!(*changes.borrow(), 0);
    client.tools().await.expect("列得出");
    tokio::time::timeout(
        Duration::from_secs(10),
        changes.wait_for(|count| *count == 1),
    )
    .await
    .expect("十秒内数到")
    .expect("连着");
}
