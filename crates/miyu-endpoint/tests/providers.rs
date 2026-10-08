//! 第一次接入的三个方法（施工 8-11，`docs/blueprint/models.md`「协议」、「怎么走」第七条）：`provider.detect` 照交进来的
//! 环境找、值不交，本机的服务几家一起探、300 毫秒没回的当没有；`provider.catalog` 搜、排；`provider.test` 在
//! `providers_test.rs`。目录是真目录裁出来的一份，本机的服务是本机回环上的假服务器。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::support::providers::{core, data, profiles};
use crate::support::*;
use miyu_http::testkit::{Piece, Reply, Server};

/// 一眼看得出是假的 key。
const FAKE: &str = "sk-FAKE-KEY-FOR-TESTS-0001";

/// 照配置 `config`、环境 `env`、档案 `extra` 起核心，问一次 `method`。
async fn ask(
    home: &Home,
    env: &[(&str, &str)],
    extra: Value,
    method: &str,
    params: Value,
) -> Value {
    let core = core(home, env, data(profiles(extra)));
    let mut client = Client::connect(core);
    client.hello().await;
    client.call("ask-1", method, params).await
}

/// 一家本机的服务列出 `models`，回之前先走 `pieces`。
fn listing(pieces: Vec<Piece>, models: &[&str]) -> Reply {
    let data: Vec<Value> = models.iter().map(|id| json!({"id": id})).collect();
    let body = json!({"object": "list", "data": data}).to_string();
    let mut body_pieces = pieces;
    body_pieces.push(Piece::Bytes(body.into_bytes()));
    Reply::stream(body_pieces)
}

#[tokio::test]
async fn keys_are_found_in_the_core_environment_and_their_values_never_leave() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[providers.ds]\nkeys = [{ env = \"DEEPSEEK_API_KEY\" }]\n\n[providers.zz]\nkeys = [{ env = \"DEEPSEEK_API_KEY\" }]\n",
    );
    let env = [
        ("DEEPSEEK_API_KEY", FAKE),
        ("OPENCODE_API_KEY", FAKE),
        ("ANTHROPIC_API_KEY", " \t"),
        ("NOT_LOOKED_FOR", FAKE),
    ];
    let reply = ask(&home, &env, json!({}), "provider.detect", json!({})).await;
    assert!(!reply.to_string().contains("FAKE"), "值不交：{reply}");
    let result = &reply["result"];
    assert_eq!(
        result["keys"],
        json!([
            {"env": "DEEPSEEK_API_KEY", "provider": "deepseek", "name": "DeepSeek", "driver": "openai-chat",
                "supported": true, "configured": "ds"},
            {"env": "OPENCODE_API_KEY", "provider": "opencode-go", "name": "OpenCode Go", "driver": "openai-chat",
                "supported": true},
            {"env": "OPENCODE_API_KEY", "provider": "opencode", "name": "OpenCode Zen", "driver": "openai-chat",
                "supported": true},
        ]),
        "空白的不算设了；配好的写编号照字节排第一的"
    );
    assert_eq!(result["local"], json!([]));
    let looked: Vec<&str> = result["looked_for"]
        .as_array()
        .expect("是列表")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(looked.contains(&"ANTHROPIC_API_KEY") && looked.contains(&"DEEPSEEK_API_KEY"));
    assert!(!looked.contains(&"NOT_LOOKED_FOR"));
    let mut sorted = looked.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(looked, sorted, "照字节排、去重");

    let anthropic = ask(
        &home,
        &[("ANTHROPIC_API_KEY", FAKE)],
        json!({}),
        "provider.detect",
        json!({}),
    )
    .await;
    assert_eq!(
        anthropic["result"]["keys"],
        json!([{"env": "ANTHROPIC_API_KEY", "provider": "anthropic", "name": "Anthropic", "driver": "anthropic",
            "supported": false}]),
        "用不了的也列，标出来"
    );
    let bad = ask(&home, &[], json!({}), "provider.detect", json!([1])).await;
    assert_eq!(reason(&bad), Some("bad_params"));
}

/// 三家要一起到了才回（闸），挨个探的话一家都等不到；慢的、回错的、读不出的当没有。
#[tokio::test]
async fn local_services_are_probed_together_and_slow_ones_count_as_none() {
    let gate = Arc::new(tokio::sync::Barrier::new(3));
    let gated = |models: &[&str]| listing(vec![Piece::Gate(Arc::clone(&gate))], models);
    let a = Server::start(vec![gated(&["qwen3-8b", "llama-3", "qwen3-8b"])]).await;
    let b = Server::start(vec![gated(&["gemma"])]).await;
    let c = Server::start(vec![gated(&[])]).await;
    let slow = Server::start(vec![listing(
        vec![Piece::Wait(Duration::from_secs(2))],
        &["late"],
    )])
    .await;
    let broken = Server::start(vec![Reply::error(500, &[], "down")]).await;
    let garbled = Server::start(vec![Reply::stream(vec![Piece::Bytes(
        b"not json".to_vec(),
    )])])
    .await;
    let local = |name: &str, server: &Server| json!({"name": name, "driver": "openai-chat", "base_url": format!("{}/", server.base_url)});
    let extra = json!({
        "lab-a": local("Lab A", &a), "lab-b": local("Lab B", &b), "lab-c": local("Lab C", &c),
        "lab-slow": local("Slow", &slow), "lab-broken": local("Broken", &broken),
        "lab-garbled": local("Garbled", &garbled),
        "lab-later": {"driver": "google", "base_url": "http://127.0.0.1:9/v1"}
    });
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[providers.mine]\ncatalog = \"lab-b\"\nkeys = []\n",
    );
    let reply = ask(&home, &[], extra, "provider.detect", json!({})).await;
    let found = &reply["result"]["local"];
    assert_eq!(
        found,
        &json!([
            {"provider": "lab-a", "name": "Lab A", "base_url": format!("{}/", a.base_url), "models": ["llama-3", "qwen3-8b"]},
            {"provider": "lab-b", "name": "Lab B", "base_url": format!("{}/", b.base_url), "models": ["gemma"],
                "configured": "mine"},
            {"provider": "lab-c", "name": "Lab C", "base_url": format!("{}/", c.base_url), "models": []},
        ]),
        "{reply}"
    );
    assert_eq!(a.received()[0].path, "/v1/models", "照驱动的 models_path");
    assert_eq!(a.received()[0].header("authorization"), None, "不带 key");
}

#[tokio::test]
async fn the_catalog_is_searched_and_usable_ones_come_first() {
    let home = Home::new();
    let found = ask(
        &home,
        &[],
        json!({}),
        "provider.catalog",
        json!({"query": "DEEP"}),
    )
    .await;
    assert_eq!(
        found["result"]["providers"],
        json!([
            {"id": "deepseek", "name": "DeepSeek", "driver": "openai-chat", "base_url": "https://api.deepseek.com",
                "env": ["DEEPSEEK_API_KEY"], "doc": "https://api-docs.deepseek.com/quick_start/pricing",
                "models": 4, "supported": true, "local": false},
            {"id": "deepinfra", "name": "Deep Infra", "driver": null, "base_url": null,
                "env": ["DEEPINFRA_API_KEY"], "doc": "https://deepinfra.com/models", "models": 1,
                "supported": false, "local": false},
        ])
    );
    let local = json!({"here": {"name": "Here", "driver": "openai-chat", "base_url": "http://localhost:1/v1"}});
    let all = ask(&home, &[], local.clone(), "provider.catalog", json!({})).await;
    let providers = all["result"]["providers"].as_array().expect("是列表");
    assert_eq!(providers.len(), 11, "裁出来的十家加档案里的一家");
    let here = providers
        .iter()
        .find(|entry| entry["id"] == "here")
        .expect("档案里的一家也列");
    assert_eq!(here["local"], true);
    let usable = providers
        .iter()
        .take_while(|entry| entry["supported"] == true)
        .count();
    assert!(
        providers[usable..]
            .iter()
            .all(|entry| entry["supported"] == false)
    );
    let two = ask(&home, &[], local, "provider.catalog", json!({"limit": 2})).await;
    assert_eq!(two["result"]["providers"].as_array().map(Vec::len), Some(2));
    assert_eq!(two["result"]["providers"][0], providers[0]);
    for params in [
        json!({"limit": 0}),
        json!({"limit": -1}),
        json!({"limit": 1.5}),
        json!({"query": 5}),
        json!([]),
    ] {
        let reply = ask(&home, &[], json!({}), "provider.catalog", params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}");
    }
}

/// 常用的几家（施工 8-11 再补）：照资源目录 `models/featured.toml` 的先后，目录、档案里没有的跳过；写法和搜到的同一家一样，
/// 名字换成它写的、照连接的语言挑；中文的 Kimi、通义用国内的那一家。不看 `query`、`limit`。
#[tokio::test]
async fn featured_ones_follow_the_resource_order_and_the_language() {
    let profile = |name: &str, base_url: &str| json!({"name": name, "driver": "openai-chat", "base_url": base_url});
    let extra = json!({
        "moonshotai": profile("Moonshot AI", "https://api.moonshot.ai/v1"),
        "moonshotai-cn": profile("Moonshot AI (China)", "https://api.moonshot.cn/v1"),
        "alibaba-cn": profile("Alibaba (China)", "https://dashscope.aliyuncs.com/compatible-mode/v1"),
    });
    let shown = |reply: &Value| -> Vec<(String, String)> {
        let providers = reply["result"]["providers"].as_array().expect("是列表");
        providers
            .iter()
            .map(|entry| (text(&entry["id"]), text(&entry["name"])))
            .collect()
    };
    let chinese = Home::new();
    let params = json!({"featured": true, "query": "nothing-like-this", "limit": 1});
    let reply = ask(&chinese, &[], extra.clone(), "provider.catalog", params).await;
    assert_eq!(
        shown(&reply),
        pairs(&[
            ("deepseek", "DeepSeek"),
            ("openai", "OpenAI"),
            ("anthropic", "Anthropic"),
            ("moonshotai-cn", "Kimi"),
            ("alibaba-cn", "通义千问"),
            ("opencode", "opencode Zen"),
        ])
    );
    let searched = ask(
        &chinese,
        &[],
        extra.clone(),
        "provider.catalog",
        json!({"query": "deepseek"}),
    )
    .await;
    assert_eq!(
        reply["result"]["providers"][0], searched["result"]["providers"][0],
        "写法和搜到的同一家一样"
    );

    let english = Home::new();
    english.write("system/config.toml", "[ui]\nlanguage = \"en\"\n");
    let reply = ask(
        &english,
        &[],
        extra,
        "provider.catalog",
        json!({"featured": true}),
    )
    .await;
    assert_eq!(
        shown(&reply),
        pairs(&[
            ("deepseek", "DeepSeek"),
            ("openai", "OpenAI"),
            ("anthropic", "Anthropic"),
            ("moonshotai", "Kimi"),
            ("opencode", "opencode Zen"),
        ]),
        "国际的通义目录里没有，跳过"
    );

    let plain = ask(
        &chinese,
        &[],
        json!({}),
        "provider.catalog",
        json!({"featured": false, "limit": 2}),
    )
    .await;
    assert_eq!(
        plain["result"]["providers"].as_array().map(Vec::len),
        Some(2)
    );
    let wrong = ask(
        &chinese,
        &[],
        json!({}),
        "provider.catalog",
        json!({"featured": "yes"}),
    )
    .await;
    assert_eq!(reason(&wrong), Some("bad_params"));
}

/// 一格字。
fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_string()
}

/// 写成 `(编号, 名字)` 的列表。
fn pairs(wanted: &[(&str, &str)]) -> Vec<(String, String)> {
    wanted
        .iter()
        .map(|(id, name)| ((*id).to_string(), (*name).to_string()))
        .collect()
}
