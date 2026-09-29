//! 订阅的回应带会话的限额（施工 6-3 补，`docs/blueprint/protocol.md` 的 `subscribe`）：窗口、压缩线，没有的不写。
//! 头每一种接进来的时候都拿得到：造完会话、别的连接中途接进来、订阅着再订阅、核心重启以后载入。
//!
//! 替身没报最大输出，输出预留照出厂策略的上限 20000，余量 13000：压缩线 = 窗口 − 33000。

mod support;

use serde_json::json;

use miyu_session::testkit::Script;
use support::*;

/// 窗口是 `window` 的替身，一句都不用答。
fn script(window: u64) -> Script {
    Script::new([]).window(window)
}

/// 回应照蓝图的例子一字不差：窗口 1000000，压缩线 967000，键照字母先后排。
#[tokio::test]
async fn the_reply_is_the_drawing_example() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&script(1_000_000)));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client.subscribe("c2", &session).await;
    assert_eq!(
        serde_json::to_string(&reply).expect("写得成 JSON"),
        r#"{"id":"c2","jsonrpc":"2.0","result":{"limits":{"compaction_line":967000,"window":1000000}}}"#
    );
}

/// 模型的资料没报窗口：两格都不写，`limits` 还在。
#[tokio::test]
async fn without_a_window_the_limits_are_empty() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client.subscribe("c2", &session).await;
    assert_eq!(reply["result"], json!({"limits": {}}), "{reply}");
}

/// 窗口太小，算不出正数的压缩线：只有窗口。
#[tokio::test]
async fn too_small_a_window_has_no_line() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&script(33_000)));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client.subscribe("c2", &session).await;
    assert_eq!(
        reply["result"],
        json!({"limits": {"window": 33_000}}),
        "{reply}"
    );
}

/// 订阅着、还在推的再订阅，还是那一个，回应照样带；别的连接中途接进来的也带；取消订阅还是空对象。
#[tokio::test]
async fn every_subscriber_gets_them() {
    let home = Home::new();
    let core = home.core(&script(60_000));
    let expected = json!({"limits": {"compaction_line": 27_000, "window": 60_000}});
    let mut first = Client::connect(core.clone());
    first.hello().await;
    let session = first.create("c1", "~").await;
    let reply = first.subscribe("c2", &session).await;
    assert_eq!(reply["result"], expected, "{reply}");
    let reply = first.subscribe("c3", &session).await;
    assert_eq!(reply["result"], expected, "订阅着再订阅：{reply}");
    let mut second = Client::connect(core);
    second.hello().await;
    let reply = second.subscribe("d1", &session).await;
    assert_eq!(reply["result"], expected, "别的连接接进来：{reply}");
    let reply = first
        .call(
            "c4",
            "unsubscribe",
            json!({"session": session, "stream": "events"}),
        )
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
}

/// 核心重启以后，订阅把会话载入：限额照载入时交给内核的。
#[tokio::test]
async fn a_session_loaded_after_a_restart_has_them_too() {
    let home = Home::new();
    let script = script(60_000);
    let first = home.core(&script);
    let mut client = Client::connect(first.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    first.stop_sessions().await;
    drop(client);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let reply = client.subscribe("c2", &session).await;
    assert_eq!(
        reply["result"],
        json!({"limits": {"compaction_line": 27_000, "window": 60_000}}),
        "{reply}"
    );
}
