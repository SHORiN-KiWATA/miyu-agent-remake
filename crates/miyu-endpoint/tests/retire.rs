//! 闲够了的会话退下（施工 V-2 再补，`docs/designs/07-存储.md` 第七节「会话按需载入」第 3 条）：会话表隔一阵问一遍，没人订阅、
//! 闲够了的从表里拿掉；再说话照常载入回来接着用。订阅着的不退，放下订阅以后再闲够了才退。

use std::time::Duration;

use serde_json::json;

use crate::support::{Client, Home, TOKEN};
use miyu_endpoint::Core;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

/// 测试里闲多久退下。
const IDLE: Duration = Duration::from_millis(200);

/// 等到载入着 `count` 个会话，最多六十秒。
async fn until_loaded(core: &Core, count: usize) {
    let waited = tokio::time::timeout(Duration::from_secs(60), async {
        while core.loaded().await != count {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "六十秒内载入着的没变成 {count} 个");
}

/// 闲 [`IDLE`] 退下的核心。剧本要排上标题：没排的，假模型不回起标题的请求，会话一直有事。
fn core(home: &Home, script: &Script) -> std::sync::Arc<Core> {
    std::sync::Arc::new(
        home.core_full(script, Catalog::default(), None, TOKEN)
            .with_sandbox(miyu_sandbox::Availability::Usable("miyu-sandbox".into()))
            .with_session_idle(IDLE),
    )
}

#[tokio::test]
async fn an_idle_session_leaves_the_table_and_comes_back_when_spoken_to() {
    let home = Home::new();
    let script = Script::new([Play::Says("在。"), Play::Says("还在。")]);
    let core = core(&home, &script.titles([Play::Says("测试")]));
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "/work").await;
    client.say("m1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    until_loaded(&core, 0).await;
    let reply = client.say("m2", &session, "还在吗").await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 2).await;
    assert_eq!(core.loaded().await, 1, "说话时载入回来");
    until_loaded(&core, 0).await;
    drop(client);
    // 核心重启以后，先载入（不是先造）的会话也照样退下。
    let script = Script::new([Play::Says("又在。")]).titles([Play::Says("测试")]);
    let core = self::core(&home, &script);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    client.say("m3", &session, "又来了").await;
    home.until_turns(&session, 3).await;
    until_loaded(&core, 0).await;
}

#[tokio::test]
async fn a_subscribed_session_stays_until_the_head_lets_go() {
    let home = Home::new();
    let core = core(
        &home,
        &Script::new([Play::Says("在。")]).titles([Play::Says("测试")]),
    );
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "/work").await;
    let subscribed = client.subscribe("s1", &session).await;
    assert!(subscribed.get("error").is_none(), "{subscribed}");
    client.say("m1", &session, "在吗").await;
    client.until_turn_ends(&session).await;
    tokio::time::sleep(IDLE * 5).await;
    assert_eq!(core.loaded().await, 1, "订阅着，不退");
    drop(client);
    until_loaded(&core, 0).await;
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let listed = client.call("l1", "session.list", json!({})).await;
    let sessions = listed["result"]["sessions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        sessions
            .iter()
            .any(|item| item["session"] == json!(session)),
        "退下的会话照样列得出：{listed}"
    );
}
