//! `usage.query`（施工 8-15，`docs/blueprint/models.md`「协议」`usage.query`）：会话的请求、一次性调用都记得进，照分组加起来；
//! 金额照币种各加各的，照 `usage.currency` 排；一次性调用照用途分组、写进账号日志、没有会话；参数不对的 `bad_params`。

use serde_json::{Value, json};

use crate::support::providers::{data, profiles, routed, scripted};
use crate::support::*;
use miyu_http::testkit::{Reply, Server};
use miyu_models::catalog::{Price, Rates};
use miyu_models::price::Tariff;
use miyu_session::testkit::{Play, Script};
use miyu_store::journal;

/// 一家 `a` 在假服务器上：`a/m` 每百万输入 1、输出 2 人民币，`a/f` 没价格；`models.chat` 是 `a/m`。还有 `usage` 那一段。
/// 假服务器在回环地址上，不写 `local = false` 会当本机的服务，没写价格的照免费算。
fn config(server: &Server, usage: &str) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nlocal = false\n\n[providers.a.models.m.price]\ninput = 1.0\noutput = 2.0\ncurrency = \"CNY\"\n\n[models]\nchat = \"a/m\"\n{usage}",
        server.base_url
    )
}

/// 假服务器说一句，报 12 输入、3 输出。
fn said(text: &str) -> Reply {
    crate::support::providers::said(text)
}

/// 问一句，用途 `purpose`，模型 `model`。
fn asked(model: &str, purpose: &str) -> Value {
    json!({"model": model, "purpose": purpose, "messages": [{"role": "user", "text": "hi"}]})
}

#[tokio::test]
async fn one_shot_calls_count_by_purpose_and_land_in_the_account_journal() {
    let server = Server::start(vec![said("a"), said("b"), said("c")]).await;
    let home = Home::new();
    home.write("system/config.toml", &config(&server, ""));
    let mut client = Client::connect(routed(&home, &[], data(profiles(json!({})))));
    client.hello().await;
    for (id, model, purpose) in [
        ("c1", "a/m", "platform"),
        ("c2", "a/m", "platform"),
        ("c3", "a/f", "notes"),
    ] {
        let reply = client.call(id, "model.call", asked(model, purpose)).await;
        assert!(reply.get("result").is_some(), "{reply}");
    }
    let reply = client
        .call(
            "q1",
            "usage.query",
            json!({"group": ["purpose", "model", "session"]}),
        )
        .await;
    assert_eq!(
        reply["result"]["rows"],
        json!([
            {"purpose": "notes", "model": "a/f", "session": null, "requests": 1,
             "usage": {"uncached": 12, "cache_read": 0, "cache_write": 0, "output": 3},
             "amounts": [], "unpriced": 1},
            {"purpose": "platform", "model": "a/m", "session": null, "requests": 2,
             "usage": {"uncached": 24, "cache_read": 0, "cache_write": 0, "output": 6},
             "amounts": [{"currency": "CNY", "amount": 2.0 * (12.0 + 3.0 * 2.0) / 1e6}], "unpriced": 0},
        ]),
        "{reply}"
    );
    let journal = std::fs::read_to_string(home.root.account_dir(&alice()).join(journal::FILE))
        .expect("有账号日志");
    assert_eq!(
        journal.matches(r#""kind":"usage.oneshot""#).count(),
        3,
        "{journal}"
    );
    assert!(
        journal.contains(r#""cost":{"amount":0.000018,"currency":"CNY","price":{"input":1,"output":2},"multiplier":1,"source":"config:system/config.toml:"#),
        "{journal}"
    );
}

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

#[tokio::test]
async fn a_session_counts_under_its_session_venue_and_account() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("再见。")]).priced(usd());
    let mut client = Client::connect(scripted(&home, data(profiles(json!({}))), script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    client.say("c3", &session, "bye").await;
    home.until_turns(&session, 2).await;
    let reply = client
        .call(
            "q1",
            "usage.query",
            json!({"session": session, "group": ["session", "venue", "account"]}),
        )
        .await;
    assert_eq!(
        reply["result"]["rows"],
        json!([{"session": session, "venue": "local", "account": "alice", "requests": 2,
            "usage": {"uncached": 120, "cache_read": 80, "cache_write": 0, "output": 20},
            "amounts": [{"currency": "USD", "amount": 2.0 * (60.0 + 40.0 * 0.5 + 10.0 * 2.0) / 1e6}],
            "unpriced": 0}]),
        "{reply}"
    );
}

/// 两次 `model.call`：`a/m` 人民币、`a/u` 美元，`usage.currency` 照 `usage` 那一段；交回不分组的那一行的币种。
async fn currencies(usage: &str) -> Vec<String> {
    let server = Server::start(vec![said("a"), said("b")]).await;
    let home = Home::new();
    let extra = format!("\n[providers.a.models.u.price]\ninput = 1.0\noutput = 1.0\n{usage}");
    home.write("system/config.toml", &(config(&server, "") + &extra));
    let mut client = Client::connect(routed(&home, &[], data(profiles(json!({})))));
    client.hello().await;
    for (id, model) in [("c1", "a/m"), ("c2", "a/u")] {
        let reply = client
            .call(id, "model.call", asked(model, "platform"))
            .await;
        assert!(reply.get("result").is_some(), "{reply}");
    }
    let reply = client.call("q1", "usage.query", json!({})).await;
    let rows = reply["result"]["rows"].as_array().expect("有 rows").clone();
    assert_eq!(rows.len(), 1, "不分组是一行：{reply}");
    assert_eq!(rows[0]["requests"], json!(2));
    rows[0]["amounts"]
        .as_array()
        .expect("数组")
        .iter()
        .map(|amount| amount["currency"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[tokio::test]
async fn amounts_stay_apart_and_the_preferred_currency_comes_first() {
    assert_eq!(currencies("").await, ["USD", "CNY"], "默认 USD 排最前");
    assert_eq!(
        currencies("\n[usage]\ncurrency = \"CNY\"\n").await,
        ["CNY", "USD"],
        "照 usage.currency"
    );
}

#[tokio::test]
async fn bad_parameters_are_refused() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    for (n, params) in [
        json!({"group": ["provider"]}),
        json!({"from": "yesterday"}),
        json!({"offset": "+9"}),
        json!({"session": "not-a-session"}),
        json!({"tree": "yes"}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client
            .call(&format!("q{n}"), "usage.query", params.clone())
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}: {reply}");
    }
    let empty = client
        .call(
            "q9",
            "usage.query",
            json!({"group": ["day"], "offset": "+09:00"}),
        )
        .await;
    assert_eq!(empty["result"], json!({"rows": []}), "{empty}");
}
