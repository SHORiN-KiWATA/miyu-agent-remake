//! `model.call`（施工 8-20，`docs/blueprint/models.md`「协议」`model.call`、「怎么走」第十二条）：经一次性入口叫本机回环上
//! 的假服务器。回应的形状；不造会话、不进会话日志；参数校验；图照这个账号的 blob 认；几种出错的 `data`；照剧本回的核心
//! 没有一次性入口，答 `no_model`。

use serde_json::{Value, json};

use crate::support::providers::{core, data, profiles, routed, said};
use crate::support::*;
use miyu_http::testkit::{Reply, Server};
use miyu_kernel::id::AccountId;
use miyu_store::blob::Blobs;

/// 一家 `a` 在假服务器上，不带 key；`a/v` 收图；`models.chat` 是 `a/m`。
fn config(server: &Server) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.v]\ninputs = [\"text\", \"image\"]\n\n[models]\nchat = \"a/m\"\n",
        server.base_url
    )
}

/// 照 `config` 写好系统配置，连上请求模型是真路由的核心、握好手。
async fn connected(home: &Home, config: &str) -> Client {
    home.write("system/config.toml", config);
    let mut client = Client::connect(routed(home, &[], data(profiles(json!({})))));
    client.hello().await;
    client
}

/// 问一句 `text`，用途 `platform`。
fn asked(text: &str) -> Value {
    json!({"purpose": "platform", "messages": [{"role": "user", "text": text}]})
}

/// 一张 `width` × `height` 的 PNG 的开头：签名和 IHDR，量宽高只看它。
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

#[tokio::test]
async fn the_answer_has_text_provider_model_and_usage_and_no_session_is_made() {
    let server = Server::start(vec![said("Hi there.")]).await;
    let home = Home::new();
    let mut client = connected(&home, &config(&server)).await;
    let reply = client.call("c1", "model.call", asked("hi")).await;
    assert_eq!(
        reply["result"],
        json!({"text": "Hi there.", "provider": "a", "model": "m",
            "usage": {"uncached": 12, "cache_read": 0, "cache_write": 0, "output": 3}}),
        "{reply}"
    );
    let sent: Value = serde_json::from_slice(&server.received()[0].body).expect("JSON");
    assert_eq!(sent["messages"], json!([{"role": "user", "content": "hi"}]));
    let listed = client.call("c2", "session.list", json!({})).await;
    assert_eq!(listed["result"]["sessions"], json!([]), "{listed}");
    let made = home
        .root
        .sessions(&alice())
        .map_or(0, |sessions| sessions.len());
    assert_eq!(made, 0, "一个会话都没造");
}

#[tokio::test]
async fn params_of_the_wrong_shape_are_bad_params_and_nothing_is_sent() {
    let server = Server::start(Vec::new()).await;
    let home = Home::new();
    let mut client = connected(&home, &config(&server)).await;
    let user = json!({"role": "user", "text": "hi"});
    let with = |field: &str, value: Value| {
        let mut params = asked("hi");
        params[field] = value;
        params
    };
    let messages = |list: Value| with("messages", list);
    let cases = vec![
        json!({"messages": [user]}),
        with("purpose", json!("")),
        with("purpose", json!("Vision")),
        with("purpose", json!("a b")),
        with("purpose", json!("v".repeat(33))),
        with("purpose", json!(1)),
        with("model", json!("")),
        with("model", json!(3)),
        with("max_tokens", json!(0)),
        with("max_tokens", json!(4_294_967_296_u64)),
        with("max_tokens", json!(-1)),
        with("max_tokens", json!(1.5)),
        json!({"purpose": "platform"}),
        messages(json!([])),
        messages(json!("hi")),
        messages(json!([{"text": "hi"}])),
        messages(json!([{"role": "tool", "text": "hi"}])),
        messages(json!([{"role": "user"}])),
        messages(json!([{"role": "user", "text": ""}])),
        messages(json!([{"role": "user", "text": "hi", "images": "x"}])),
        messages(json!([{"role": "user", "text": "hi", "images": ["zz"]}])),
        messages(json!([{"role": "system", "text": "Be brief."}])),
        messages(json!([{"role": "system", "text": ""}, user])),
        messages(json!([user, {"role": "system", "text": "Be brief."}, user])),
        messages(json!([{"role": "system", "text": "a"}, {"role": "system", "text": "b"}, user])),
        messages(json!([user, {"role": "assistant", "text": "ok"}])),
        messages(json!([{"role": "assistant", "text": ""}, user])),
        messages(json!([{"role": "assistant", "text": "ok", "images": []}, user])),
        messages(json!([{"role": "system", "text": "s", "images": []}, user])),
    ];
    for (n, params) in cases.into_iter().enumerate() {
        let reply = client
            .call(&format!("c{n}"), "model.call", params.clone())
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params} → {reply}");
    }
    assert!(server.received().is_empty(), "一个都没发");
}

#[tokio::test]
async fn an_image_is_this_accounts_blob_and_goes_out_after_the_words() {
    let server = Server::start(vec![said("A dot.")]).await;
    let home = Home::new();
    let mut client = connected(&home, &config(&server)).await;
    let put = client
        .call(
            "p1",
            "blob.put",
            json!({"data": base64(&png(2, 3)), "name": "dot.png"}),
        )
        .await;
    let blob = put["result"]["blob"].as_str().expect("存下了").to_string();
    let looking = |images: Value| {
        json!({"model": "a/v", "purpose": "vision",
            "messages": [{"role": "user", "text": "What is this?", "images": images}]})
    };
    let reply = client
        .call("c1", "model.call", looking(json!([blob])))
        .await;
    assert_eq!(reply["result"]["text"], "A dot.", "{reply}");
    let sent: Value = serde_json::from_slice(&server.received()[0].body).expect("JSON");
    let content = &sent["messages"][0]["content"];
    assert_eq!(content[0], json!({"type": "text", "text": "What is this?"}));
    assert_eq!(content[1]["type"], "image_url", "先字后图：{sent}");
    // 别的账号的 blob、这个核心里没有的：不认。
    let elsewhere = Blobs::new(home.root.blobs(&AccountId::parse("bob").expect("合写法")))
        .put(&png(4, 4))
        .expect("存得进去");
    let missing = "0".repeat(64);
    for absent in [elsewhere.as_str().to_string(), format!("sha256:{missing}")] {
        let reply = client
            .call("c2", "model.call", looking(json!([absent])))
            .await;
        assert_eq!(reason(&reply), Some("unknown_attachment"), "{reply}");
    }
    // 是这个账号的、不是图：参数不对。
    let text = client
        .call(
            "p2",
            "blob.put",
            json!({"data": base64(b"just words"), "name": "notes.txt"}),
        )
        .await;
    let text = text["result"]["blob"].as_str().expect("存下了").to_string();
    let reply = client
        .call("c3", "model.call", looking(json!([text])))
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    assert_eq!(server.received().len(), 1, "只发了认得出的那一次");
}

#[tokio::test]
async fn each_failure_has_its_reason_and_data() {
    let unauthorized = Reply::error(
        401,
        &[],
        r#"{"error":{"message":"bad key","type":"authentication_error"}}"#,
    );
    let limited = || Reply::error(429, &[], r#"{"error":{"message":"slow down"}}"#);
    let server = Server::start(vec![unauthorized, limited(), limited()]).await;
    let home = Home::new();
    // 池里两家（施工 8-25 起一家一个 key，几个候选只来自池）。
    let two = format!(
        "{}\n[providers.b1]\ndriver = \"openai-chat\"\nbase_url = \"{url}\"\nkey = {{ env = \"K1\" }}\n\n\
         [providers.b2]\ndriver = \"openai-chat\"\nbase_url = \"{url}\"\nkey = {{ env = \"K2\" }}\n\n\
         [pools.q]\nmodels = [\"b1/m\", \"b2/m\"]\nstrategy = \"pin\"\n",
        config(&server),
        url = server.base_url
    );
    home.write("system/config.toml", &two);
    let core = routed(
        &home,
        &[("K1", "sk-1"), ("K2", "sk-2")],
        data(profiles(json!({}))),
    );
    let mut client = Client::connect(core);
    client.hello().await;
    let reply = client.call("c1", "model.call", asked("hi")).await;
    assert_eq!(reason(&reply), Some("model_failed"), "{reply}");
    let data = &reply["error"]["data"];
    assert_eq!(
        (&data["class"], &data["status"]),
        (&json!("auth"), &json!(401))
    );
    assert!(
        data["message"]
            .as_str()
            .is_some_and(|said| said.contains("bad key")),
        "{reply}"
    );
    let mut on_b = asked("hi");
    on_b["model"] = json!("@q");
    let reply = client.call("c2", "model.call", on_b.clone()).await;
    assert_eq!(
        reason(&reply),
        Some("model_failed"),
        "池里两家都限速：{reply}"
    );
    assert_eq!(reply["error"]["data"]["class"], "rate_limited");
    let reply = client.call("c3", "model.call", on_b).await;
    assert_eq!(reason(&reply), Some("cooling"), "{reply}");
    let data = &reply["error"]["data"];
    assert!(
        data["message"]
            .as_str()
            .is_some_and(|said| said.starts_with("all candidates cooling: ")),
        "{reply}"
    );
    assert!(
        data["wait_ms"].as_u64().is_some_and(|wait| wait > 0),
        "{reply}"
    );
    let mut unknown = asked("hi");
    unknown["model"] = json!("nope/x");
    let reply = client.call("c4", "model.call", unknown).await;
    assert_eq!(reason(&reply), Some("unknown_model"), "{reply}");
    assert_eq!(server.received().len(), 3);
}

#[tokio::test]
async fn no_model_says_why_and_a_scripted_core_has_no_one_shot() {
    let home = Home::new();
    let mut client = connected(&home, "").await;
    let reply = client.call("c1", "model.call", asked("hi")).await;
    assert_eq!(reason(&reply), Some("no_model"), "{reply}");
    assert_eq!(
        reply["error"]["data"]["message"],
        "no model configured: set models.chat"
    );
    let server = Server::start(Vec::new()).await;
    let scripted = Home::new();
    scripted.write("system/config.toml", &config(&server));
    let mut client = Client::connect(core(&scripted, &[], data(profiles(json!({})))));
    client.hello().await;
    let reply = client.call("c2", "model.call", asked("hi")).await;
    assert_eq!(reason(&reply), Some("no_model"), "{reply}");
    assert!(server.received().is_empty());
}

/// 标准的 base64。
fn base64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[tokio::test]
async fn a_pool_is_resolved_and_a_429_member_fails_over_to_the_next() {
    let slow = Server::start(vec![Reply::error(
        429,
        &[("Retry-After", "20")],
        r#"{"error":{"message":"slow down"}}"#,
    )])
    .await;
    let ok = Server::start(vec![said("OK.")]).await;
    let config = format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"b/m\"\n\n[pools.duo]\nmodels = [\"a/m\", \"b/m\"]\nstrategy = \"pin\"\n",
        slow.base_url, ok.base_url
    );
    let home = Home::new();
    let mut client = connected(&home, &config).await;
    let mut params = asked("hi");
    params["model"] = json!("@duo");
    let reply = client.call("c1", "model.call", params).await;
    assert_eq!(reply["result"]["text"], json!("OK."), "{reply}");
    assert_eq!(
        reply["result"]["provider"],
        json!("b"),
        "钉着的第一个回了 429，换到下一个"
    );
    assert_eq!(slow.received().len(), 1);
    assert_eq!(ok.received().len(), 1);
}
