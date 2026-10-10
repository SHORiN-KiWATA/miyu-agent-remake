//! 会话状态里的会话树（施工 9-8 补下，`docs/blueprint/view.md`「会话状态」）：派出去的子代理在跑一轮时，那一项带 `busy`、
//! `spawned`、`running_deep`、`usage`；整份带 `running_deep`、连子代理一起的 `usage_tree`。子会话动了（一轮开始）父会话的
//! 视图流跟着推。

use serde_json::{Value, json};

use crate::support::{Client, Home, TOKEN, default_resources, until};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

/// 一直读推送，读到一份让 `wanted` 说是的 `view.status`，交回它。
async fn status_until(client: &mut Client, wanted: impl Fn(&Value) -> bool) -> Value {
    loop {
        let next = client.next().await.expect("没断开");
        if next["method"] == "view.status" && wanted(&next["params"]["status"]) {
            return next["params"]["status"].clone();
        }
    }
}

#[tokio::test]
async fn a_running_subagent_shows_up_in_the_tree() {
    let home = Home::new();
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("出厂的工具"))
        .expect("合写法");
    let args = json!({"description": "查 crate", "prompt": "Read Cargo.toml."}).to_string();
    // 派出去以后，父会话的下一次请求和子会话的第一次请求都停住。
    let script = Script::new([
        Play::calls(&[("subagent", &args)]),
        Play::Holds,
        Play::Holds,
    ]);
    let mut client = Client::connect(home.core_with_tools(&script, tools, TOKEN));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    let subscribed = client
        .call(
            "v1",
            "subscribe",
            json!({"session": parent, "stream": "view"}),
        )
        .await;
    assert_eq!(subscribed["result"]["status"]["running_deep"], 0);
    let request = json!({"jsonrpc": "2.0", "id": "s1", "method": "session.send",
        "params": {"session": parent, "text": "派一个去查"}});
    client.line(&request.to_string()).await;
    until("两次请求都停住", || script.requests().len() == 3).await;
    let status = status_until(&mut client, |status| {
        status["jobs"][0]["busy"] == json!(true)
    })
    .await;
    let job = &status["jobs"][0];
    assert_eq!(job["what"], "agent", "{status}");
    assert_eq!(job["state"], "running");
    assert_eq!(job["spawned"], 0, "子代理自己没派：{job}");
    assert_eq!(job["running_deep"], 0);
    assert!(job["usage"].is_object(), "那一支的用量：{job}");
    assert_eq!(
        status["running_deep"], 1,
        "这个会话在跑的一共一个：{status}"
    );
    assert!(
        status["usage_tree"].is_object(),
        "连子代理一起的用量：{status}"
    );
    assert!(
        status["usage_tree"]["requests"].as_u64() >= status["usage"]["requests"].as_u64(),
        "连子代理的不比自己的少：{status}"
    );
}

/// 子代理再派一个（施工 9-8 补下）：脚本是几个会话共用的一条，父会话第二次请求和子会话第一次请求谁先拿到「再派一个」不一定。
/// 父会话拿到的是又派了一个子代理，子会话拿到的是派了孙代理；两样都是这个会话在跑的一共两个。孙代理那一样，父会话的日志
/// 里没有新事件，只能照会话列表报的「动了」重量。
#[tokio::test]
async fn a_grandchild_counts_in_the_tree() {
    let home = Home::new();
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("出厂的工具"))
        .expect("合写法");
    let args = json!({"description": "查", "prompt": "Look around."}).to_string();
    let script = Script::new([
        Play::calls(&[("subagent", &args)]),
        Play::calls(&[("subagent", &args)]),
        Play::Holds,
        Play::Holds,
        Play::Holds,
    ]);
    let mut client = Client::connect(home.core_with_tools(&script, tools, TOKEN));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client
        .call(
            "v1",
            "subscribe",
            json!({"session": parent, "stream": "view"}),
        )
        .await;
    let request = json!({"jsonrpc": "2.0", "id": "s1", "method": "session.send",
        "params": {"session": parent, "text": "派"}});
    client.line(&request.to_string()).await;
    until("五次请求都到了", || script.requests().len() == 5).await;
    let status = status_until(&mut client, |status| status["running_deep"] == json!(2)).await;
    let jobs = status["jobs"].as_array().expect("有任务表");
    if jobs.len() == 1 {
        assert_eq!(jobs[0]["spawned"], 1, "子代理派了孙代理：{status}");
        assert_eq!(jobs[0]["running_deep"], 1);
    } else {
        assert_eq!(jobs.len(), 2, "{status}");
    }
}

/// 连子代理一起的用量（施工 9-8 补下）：父会话说了两次、子代理说了一次，谁先到都一样，一共三次请求。子代理报回来时父会话
/// 那一轮已经结束的，回报另开一轮、多一次请求（机器忙时会这样，施工 9-8 补下修）：剧本多备一句，量到三次就断言。
#[tokio::test]
async fn the_tree_usage_adds_the_children() {
    let home = Home::new();
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("出厂的工具"))
        .expect("合写法");
    let args = json!({"description": "查", "prompt": "Look around."}).to_string();
    let script = Script::new([
        Play::calls(&[("subagent", &args)]),
        Play::Says("好。"),
        Play::Says("查完了。"),
        Play::Says("收到。"),
    ]);
    let mut client = Client::connect(home.core_with_tools(&script, tools, TOKEN));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client
        .call(
            "v1",
            "subscribe",
            json!({"session": parent, "stream": "view"}),
        )
        .await;
    let request = json!({"jsonrpc": "2.0", "id": "s1", "method": "session.send",
        "params": {"session": parent, "text": "派"}});
    client.line(&request.to_string()).await;
    let status = status_until(&mut client, |status| {
        status["usage_tree"]["requests"] == json!(3)
    })
    .await;
    assert!(
        status["usage"]["requests"].as_u64() < Some(3),
        "自己的不算子代理的：{status}"
    );
}
