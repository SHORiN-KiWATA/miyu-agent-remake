//! 下架的模型移出池（施工 8-23，`docs/construction/8-23-下架的模型移出池（要补）.md`）：真路由、两台假服务器。`model.call`
//! 照池发，池里的第一家回 404、当场拉的列表里没有它：系统配置、个人设置里写着它的池都拿掉它，推 `config.changed`（`by` 是
//! 核心自己）；删空了的池留着，`model.list` 里 `usable` 是假；直接写在 `models.chat` 的不动。列表里还有它的不删。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_http::testkit::{Piece, Reply, Server};

use crate::support::providers::{data, profiles, routed, said};
use crate::support::*;

/// 模型不存在：404。
fn gone() -> Reply {
    Reply::error(
        404,
        &[],
        r#"{"error":{"message":"The model `m` does not exist","code":"model_not_found"}}"#,
    )
}

/// 列出 `models` 的回应。
fn listing(models: &[&str]) -> Reply {
    let data: Vec<Value> = models.iter().map(|id| json!({"id": id})).collect();
    Reply::stream(vec![Piece::Bytes(
        json!({"object": "list", "data": data})
            .to_string()
            .into_bytes(),
    )])
}

/// 系统配置：两家 `a`、`b`，钉住的池 `p` 是 `a/m`、`b/m`，池 `q` 只有 `b/m`；`models.chat` 直接写 `a/m`。
fn system(first: &Server, second: &Server) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n\
         [pools.p]\nmodels = [\"a/m\", \"b/m\"]\nstrategy = \"pin\"\n\n[pools.q]\nmodels = [\"b/m\"]\n\n[models]\nchat = \"a/m\"\n",
        first.base_url, second.base_url
    )
}

/// 个人设置：池 `solo` 只有 `a/m`。
const PERSONAL: &str = "[pools.solo]\nmodels = [\"a/m\"]\n";

/// 连上、握手、订阅配置的推送。
async fn watching(core: &std::sync::Arc<miyu_endpoint::Core>) -> Client {
    let mut client = Client::connect(std::sync::Arc::clone(core));
    client.hello().await;
    let reply = client
        .call("s1", "subscribe", json!({"stream": "config"}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    client
}

/// 照池 `@p` 问一次。
fn asked() -> Value {
    json!({"purpose": "platform", "model": "@p", "messages": [{"role": "user", "text": "hi"}]})
}

/// `model.list` 里名字是 `name` 的池。
async fn pool(client: &mut Client, name: &str) -> Value {
    let reply = client.call("l", "model.list", json!({})).await;
    reply["result"]["pools"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|pool| pool["name"] == name)
        .cloned()
        .unwrap_or_else(|| panic!("没有池 {name}：{reply}"))
}

#[tokio::test]
async fn a_gone_member_leaves_every_pool_that_lists_it() {
    let (first, second) = (
        Server::start(vec![gone(), listing(&["other"])]).await,
        Server::start(vec![said("好。")]).await,
    );
    let home = Home::new();
    home.write("system/config.toml", &system(&first, &second));
    home.write("home/alice/settings.toml", PERSONAL);
    let core = routed(&home, &[], data(profiles(json!({}))));
    core.start_retirement();
    let mut watcher = watching(&core).await;
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let reply = client.call("c1", "model.call", asked()).await;
    assert_eq!(
        reason(&reply),
        Some("model_failed"),
        "这一次照旧出错：{reply}"
    );

    let mut layers = Vec::new();
    for _ in 0..2 {
        let next = watcher.next().await.expect("推了");
        assert_eq!(next["method"], "config.changed", "{next}");
        assert_eq!(next["params"]["by"], json!({"kind": "kernel"}), "{next}");
        layers.push(
            next["params"]["layer"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        );
    }
    layers.sort();
    assert_eq!(layers, ["personal", "system"], "两层里写着它的都拿掉");
    let system = read(&home, "system/config.toml");
    assert!(system.contains("[pools.p]\nmodels = [\"b/m\"]"), "{system}");
    assert!(
        system.contains("[pools.q]\nmodels = [\"b/m\"]"),
        "没写它的池不动：{system}"
    );
    assert!(
        system.contains("chat = \"a/m\""),
        "直接写着的不动：{system}"
    );
    let personal = read(&home, "home/alice/settings.toml");
    assert!(
        personal.contains("models = []"),
        "删空了的池留着：{personal}"
    );
    assert_eq!(pool(&mut client, "solo").await["usable"], false);
    assert_eq!(pool(&mut client, "p").await["usable"], true);
    assert_eq!(first.received().len(), 2, "发了一次、拉了一次列表");
}

#[tokio::test]
async fn a_member_still_listed_stays() {
    let (first, second) = (
        Server::start(vec![gone(), listing(&["m"])]).await,
        Server::start(Vec::new()).await,
    );
    let home = Home::new();
    home.write("system/config.toml", &system(&first, &second));
    let core = routed(&home, &[], data(profiles(json!({}))));
    core.start_retirement();
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let reply = client.call("c1", "model.call", asked()).await;
    assert_eq!(reason(&reply), Some("model_failed"), "{reply}");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while first.received().len() < 2 {
        assert!(tokio::time::Instant::now() < deadline, "拉了列表");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // 拉到了、确认完了也不写：再等一会儿看配置没变。
    tokio::time::sleep(Duration::from_millis(300)).await;
    let system = read(&home, "system/config.toml");
    assert!(system.contains("models = [\"a/m\", \"b/m\"]"), "{system}");
}

/// 数据根里一份文件的字。
fn read(home: &Home, relative: &str) -> String {
    std::fs::read_to_string(home.root.path().join(relative)).unwrap_or_default()
}
