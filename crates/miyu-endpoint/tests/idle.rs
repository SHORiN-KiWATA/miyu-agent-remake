//! 空闲（施工 3-9 上）：没有连接、也没有在跑的回合，核心才算空闲；头走了但回合还在跑的，不算。

use std::time::Duration;

use serde_json::json;

use crate::support::{Client, Home};
use miyu_endpoint::Core;
use miyu_session::testkit::{Play, Script};

/// 等到 `done` 成立，最多十秒。
async fn until_core(what: &str, core: &Core, done: impl AsyncFn(&Core) -> bool) {
    let waited = tokio::time::timeout(Duration::from_secs(10), async {
        while !done(core).await {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "十秒内没等到{what}");
}

#[tokio::test]
async fn a_connection_keeps_the_core_busy_until_it_goes() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    assert!(core.idle().await, "刚起来");
    let mut client = Client::connect(core.clone());
    client.hello().await;
    assert_eq!(core.connections(), 1);
    assert!(!core.idle().await, "连着一个");
    drop(client);
    until_core("连接走了", &core, async |core| core.idle().await).await;
    assert_eq!(core.connections(), 0);
}

#[tokio::test]
async fn a_running_turn_keeps_the_core_busy_after_the_head_goes() {
    let home = Home::new();
    let core = home.core(&Script::new([Play::Holds]));
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "/work").await;
    client.say("m1", &session, "在吗").await;
    drop(client);
    until_core("连接走了", &core, async |core| core.connections() == 0).await;
    assert!(!core.idle().await, "头走了，这一轮还在跑（模型一直不回）");

    let mut client = Client::connect(core.clone());
    client.hello().await;
    let reply = client
        .call(
            "i1",
            "session.interrupt",
            json!({"session": session, "queued": "return"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    drop(client);
    until_core("打断以后空闲", &core, async |core| core.idle().await).await;
}

#[tokio::test]
async fn stopping_the_sessions_ends_a_running_turn_as_restarted() {
    let home = Home::new();
    let core = home.core(&Script::new([Play::Holds]));
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "/work").await;
    client.say("m1", &session, "在吗").await;
    core.stop_sessions().await;
    let ended = home
        .log(&session)
        .into_iter()
        .find_map(|event| match event.body {
            miyu_kernel::event::Body::TurnEnded(ended) => Some(ended.reason),
            _ => None,
        });
    assert_eq!(ended, Some(miyu_kernel::event::EndReason::Restarted));
    assert!(!core.idle().await, "会话都停了，连接还在");
    drop(client);
    until_core("空闲", &core, async |core| core.idle().await).await;
}
