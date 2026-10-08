//! 订阅的回应带会话的限额（施工 6-3 补，`docs/blueprint/protocol.md` 的 `subscribe`）：窗口、压缩线，没有的不写。
//! 头每一种接进来的时候都拿得到：造完会话、别的连接中途接进来、订阅着再订阅、核心重启以后载入。回应还带会话接下来请求的
//! 模型（施工 8-10）：替身的是 deepseek 的 deepseek-v4，没配 `models.chat` 的会话没有引用。
//!
//! 替身没报最大输出，输出预留照出厂策略的上限 20000，余量 13000：压缩线 = 窗口 − 33000。

use serde_json::json;

use crate::support::*;
use miyu_models::settings::{ProviderSettings, UseSettings};
use miyu_session::testkit::Script;

/// 限额是 `limits` 的那一份回应：模型是替身的 deepseek 的 deepseek-v4，没有引用。新会话的「当前的」几格（施工 9-6 上）：
/// 什么都没花、权限是出厂的、没有在跑的任务；在 `~` 里造的会话在账号的工作区里干活（施工 9-7 上）。
fn reply_of(home: &Home, limits: serde_json::Value) -> serde_json::Value {
    json!({"jobs": [], "limits": limits, "model": {"endpoint": "deepseek", "model": "deepseek-v4"},
        "permission": {"level": "workspace", "read_only": false}, "preset": "full",
        "usage": {"amounts": [], "cache_breaks": 0, "compactions": 0, "requests": 0, "unpriced": 0,
            "main": {"cache_read": 0, "cache_write": 0, "output": 0, "uncached": 0},
            "usage": {"cache_read": 0, "cache_write": 0, "output": 0, "uncached": 0}},
        "workspace": {"cwd": workspace(home), "dirs": []}})
}

/// 账号的工作区，照会话记下的写法。
fn workspace(home: &Home) -> String {
    home.root.workspace(&alice()).to_string_lossy().into_owned()
}

/// 窗口是 `window` 的替身，一句都不用答。
fn script(window: u64) -> Script {
    Script::new([]).window(window)
}

/// 回应照蓝图的例子一字不差：窗口 1000000，压缩线 967000，会话照 `models.chat` 记下的引用，键照字母先后排。
#[tokio::test]
async fn the_reply_is_the_drawing_example() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[providers.deepseek]\nkeys = []\n\n[models]\nchat = \"deepseek/deepseek-v4\"\n",
    );
    let items = [ProviderSettings::ITEMS, UseSettings::ITEMS].concat();
    let mut client = Client::connect(home.core_with_items(&script(1_000_000), None, &[], &items));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client.subscribe("c2", &session).await;
    // 蓝图里工作区写成 `<工作区>`：换成这个测试的数据根里账号的工作区（照 JSON 的写法转义，Windows 上有反斜杠）。
    let example = r#"{"id":"c2","jsonrpc":"2.0","result":{"jobs":[],"limits":{"compaction_line":967000,"window":1000000},"model":{"endpoint":"deepseek","model":"deepseek-v4","ref":"deepseek/deepseek-v4"},"permission":{"level":"workspace","read_only":false},"preset":"full","usage":{"amounts":[],"cache_breaks":0,"compactions":0,"main":{"cache_read":0,"cache_write":0,"output":0,"uncached":0},"requests":0,"unpriced":0,"usage":{"cache_read":0,"cache_write":0,"output":0,"uncached":0}},"workspace":{"cwd":"<工作区>","dirs":[]}}}"#;
    let quoted = serde_json::to_string(&workspace(&home)).expect("写得成 JSON");
    assert_eq!(
        serde_json::to_string(&reply).expect("写得成 JSON"),
        example.replace("\"<工作区>\"", &quoted)
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
    assert_eq!(reply["result"], reply_of(&home, json!({})), "{reply}");
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
        reply_of(&home, json!({"window": 33_000})),
        "{reply}"
    );
}

/// 订阅着、还在推的再订阅，还是那一个，回应照样带；别的连接中途接进来的也带；取消订阅还是空对象。
#[tokio::test]
async fn every_subscriber_gets_them() {
    let home = Home::new();
    let core = home.core(&script(60_000));
    let expected = reply_of(&home, json!({"compaction_line": 27_000, "window": 60_000}));
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
        reply_of(&home, json!({"compaction_line": 27_000, "window": 60_000})),
        "{reply}"
    );
}
