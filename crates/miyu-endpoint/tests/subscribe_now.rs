//! 订阅的回应带上「当前的」（施工 9-6 上，`docs/blueprint/protocol.md` 的 `subscribe`）：真核心走一遍。累计的用量和
//! `usage.query {"session"}` 那一行一样，另带压缩、缓存断了几次；人设的权限；还在跑的后台命令和子代理。核心重启、会话重新
//! 载入以后照日志算出来的还是那一份。

mod support;

use serde_json::{Value, json};

use miyu_models::catalog::{Price, Rates};
use miyu_models::price::Tariff;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;
use support::providers::{data, profiles, scripted};
use support::*;

/// 剧本端口的价格：每百万输入 1、读缓存 0.5、输出 2 美元。剧本一次报 60 没命中、40 命中、10 输出。
fn usd() -> Tariff {
    Tariff {
        price: Price::of(
            Rates {
                input: Some(1.0),
                output: Some(2.0),
                cache_read: Some(0.5),
                cache_write: None,
            },
            "USD",
        ),
        multiplier: 1.0,
        source: "local".to_string(),
    }
}

/// 订阅会话 `session` 的事件流（带 `after`：换一个新的），交回回应的 `result`。
async fn subscribed(client: &mut Client, id: &str, session: &str, after: u64) -> Value {
    let (_, reply) = {
        client
            .line(
                &json!({"jsonrpc": "2.0", "id": id, "method": "subscribe",
                    "params": {"session": session, "stream": "events", "after": after}})
                .to_string(),
            )
            .await;
        client.until_reply(id).await
    };
    reply["result"].clone()
}

#[tokio::test]
async fn the_totals_match_the_usage_query_and_survive_a_reload() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("再见。")]).priced(usd());
    let mut client = Client::connect(scripted(&home, data(profiles(json!({}))), script.clone()));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let fresh = subscribed(&mut client, "s0", &session, 0).await;
    assert_eq!(
        fresh["usage"],
        json!({"requests": 0, "usage": {"uncached": 0, "cache_read": 0, "cache_write": 0, "output": 0},
            "amounts": [], "unpriced": 0, "compactions": 0, "cache_breaks": 0,
            "main": {"uncached": 0, "cache_read": 0, "cache_write": 0, "output": 0}}),
        "{fresh}"
    );
    assert_eq!(
        fresh["permission"],
        json!({"level": "workspace", "read_only": false})
    );
    assert_eq!(fresh["jobs"], json!([]));
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    client.say("c3", &session, "bye").await;
    home.until_turns(&session, 2).await;
    let now = subscribed(&mut client, "s1", &session, 0).await;
    let query = client
        .call("q1", "usage.query", json!({"session": session}))
        .await;
    let mut row = query["result"]["rows"][0].clone();
    row["main"] = row["usage"].clone();
    row["compactions"] = json!(0);
    row["cache_breaks"] = now["usage"]["cache_breaks"].clone();
    assert_eq!(now["usage"], row, "和 usage.query 那一行同一份：{now}");
    assert_eq!(now["usage"]["requests"], 2);

    // 核心重启：会话照日志重新载入，算出来的还是那一份。
    drop(client);
    let mut again = Client::connect(scripted(&home, data(profiles(json!({}))), script));
    again.hello().await;
    let reloaded = subscribed(&mut again, "s2", &session, 0).await;
    assert_eq!(reloaded["usage"], now["usage"], "{reloaded}");
}

#[tokio::test]
async fn the_permission_is_what_the_person_set() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let set = client
        .call(
            "p1",
            "session.set_permission_level",
            json!({"session": session, "read_only": true}),
        )
        .await;
    assert_eq!(set["result"], json!({}), "{set}");
    let now = subscribed(&mut client, "s1", &session, 0).await;
    assert_eq!(
        now["permission"],
        json!({"level": "workspace", "read_only": true}),
        "{now}"
    );
    // 已经订阅着、再订阅一次不带 after 的：还是那一个，回应照样带。
    let again = client
        .call(
            "s2",
            "subscribe",
            json!({"session": session, "stream": "events"}),
        )
        .await;
    assert_eq!(again["result"]["permission"], now["permission"], "{again}");
}

#[tokio::test]
async fn running_jobs_are_listed_with_what_started_them() {
    let home = Home::new();
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).unwrap()).unwrap();
    let args = json!({"description": "查 crate", "prompt": "Read Cargo.toml."}).to_string();
    // 派出去以后，父会话的下一次请求和子会话的第一次请求都停住：子代理一直在跑。
    let script = Script::new([
        Play::calls(&[("subagent", &args)]),
        Play::Holds,
        Play::Holds,
    ]);
    let mut client = Client::connect(home.core_with_tools(&script, tools, TOKEN));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client.say("c2", &parent, "派一个去查").await;
    until("两次请求都停住", || script.requests().len() == 3).await;
    let now = subscribed(&mut client, "s1", &parent, 0).await;
    let jobs = now["jobs"].as_array().expect("是数组");
    assert_eq!(jobs.len(), 1, "{now}");
    assert_eq!(
        (&jobs[0]["job"], &jobs[0]["what"], &jobs[0]["title"]),
        (&json!("j1"), &json!("agent"), &json!("查 crate"))
    );
    assert!(jobs[0]["session"].as_str().is_some(), "子代理带它的会话");
    client
        .line(
            &json!({"jsonrpc": "2.0", "id": "j1", "method": "job.stop",
                "params": {"session": parent, "job": "j1"}})
            .to_string(),
        )
        .await;
    let (_, stopped) = client.until_reply("j1").await;
    assert_eq!(stopped["result"], json!({}), "{stopped}");
    let after = subscribed(&mut client, "s2", &parent, 0).await;
    assert_eq!(after["jobs"], json!([]), "停了就不在：{after}");
}
