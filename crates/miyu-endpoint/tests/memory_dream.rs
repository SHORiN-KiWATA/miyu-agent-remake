//! 现在就整理记忆（施工 R-7 补，`docs/blueprint/memory.md` 第七条第 9 款，`protocol.md` 的 `memory.dream`、`/dream`）：真核心，
//! 请求模型是真的路由、整理记忆照 `models.chat` 发给假服务器。
//!
//! `memory.dream` 照人格那一间现在就合、交回几样数，没有要整理的是零、不发；在后台答，同一个连接后面的请求先回；同一间同时
//! 两个的回 `memory_busy`；没装人格记忆的回 `memory_not_installed`。`/dream` 在命令表里，在会话里先抽这一段再合，回执报数字；
//! 在后台答也先见结果、后见回应：订阅着这个会话的，它那条 `command.ran` 先到。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::event::Body;

use crate::support::providers::{data, profiles, routed, said};
use crate::support::*;

/// 一家 `a` 在假服务器上，`models.chat` 是 `a/m`（整理记忆没写的照它）。
fn config(server: &Server) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"a/m\"\n",
        server.base_url
    )
}

async fn connected(home: &Home, server: &Server) -> Client {
    home.write("system/config.toml", &config(server));
    let mut client = Client::connect(routed(home, &[], data(profiles(json!({})))));
    client.hello().await;
    client
}

/// 合并交回的一段，晚 `wait` 才回。
fn slow(text: &str, wait: Duration) -> Reply {
    let mut reply = said(text);
    reply.body.insert(0, Piece::Wait(wait));
    reply
}

async fn remember(client: &mut Client, id: &str, text: &str) {
    let reply = client
        .call(
            id,
            "memory.remember",
            json!({"persona": "engineer", "class": "user", "text": text}),
        )
        .await;
    assert!(reply["result"]["id"].is_string(), "{reply}");
}

#[tokio::test]
async fn a_dream_merges_now_and_says_how_much() {
    let server = Server::start(vec![
        said(
            &json!({"retired": [{"id": "m1", "why": "重复"}], "summary": "用户养猫。"}).to_string(),
        ),
        said("{}"),
    ])
    .await;
    let home = Home::new();
    let mut client = connected(&home, &server).await;
    remember(&mut client, "c1", "用户养了一只猫").await;
    remember(&mut client, "c2", "用户养了一只猫，叫团子").await;
    let reply = client
        .call("d1", "memory.dream", json!({"persona": "engineer"}))
        .await;
    assert_eq!(
        reply["result"],
        json!({"given": 2, "revised": 0, "retired": 1, "summary": true}),
        "{reply}"
    );
    let again = client
        .call("d2", "memory.dream", json!({"persona": "engineer"}))
        .await;
    assert_eq!(
        again["result"],
        json!({"given": 0, "revised": 0, "retired": 0, "summary": false}),
        "{again}"
    );
    assert_eq!(server.received().len(), 1, "没有要整理的不发");
}

#[tokio::test]
async fn a_dream_is_answered_in_the_background_and_one_at_a_time() {
    let server = Server::start(vec![slow("{}", Duration::from_millis(800)), said("{}")]).await;
    let home = Home::new();
    let mut client = connected(&home, &server).await;
    remember(&mut client, "c1", "用户养了一只猫").await;
    let dream = |id: &str| {
        json!({"jsonrpc": "2.0", "id": id, "method": "memory.dream", "params": {"persona": "engineer"}})
            .to_string()
    };
    client.line(&dream("d1")).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    client.line(&dream("d2")).await;
    client
        .line(
            &json!({"jsonrpc": "2.0", "id": "l1", "method": "memory.list", "params": {"persona": "engineer"}})
                .to_string(),
        )
        .await;
    let mut order = Vec::new();
    let mut replies = Vec::new();
    while order.len() < 3 {
        let message = client.next().await.expect("连着");
        if let Some(id) = message["id"].as_str() {
            order.push(id.to_string());
            replies.push(message.clone());
        }
    }
    assert_eq!(
        order.last().map(String::as_str),
        Some("d1"),
        "后面的请求先回：{order:?}"
    );
    let busy = replies
        .iter()
        .find(|reply| reply["id"] == "d2")
        .expect("d2 回了");
    assert_eq!(reason(busy), Some("memory_busy"), "{busy}");
}

#[tokio::test]
async fn without_the_package_there_is_no_dream() {
    let server = Server::start(vec![said("{}")]).await;
    let home = Home::new();
    let mut client = connected(&home, &server).await;
    remember(&mut client, "c1", "用户养了一只猫").await;
    let removed = client
        .call("r1", "package.remove", json!({"package": "memory"}))
        .await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    let reply = client
        .call("d1", "memory.dream", json!({"persona": "engineer"}))
        .await;
    assert_eq!(reason(&reply), Some("memory_not_installed"), "{reply}");
    assert!(server.received().is_empty());
}

/// 抽取交回的候选：1 到 40 每一轮各一条 `text`（只有这一段里真有的那几轮留得下）。
fn every_turn(text: &str) -> Reply {
    let memories: Vec<Value> = (1..=40)
        .map(|turn| json!({"class": "user", "text": text, "turn": turn}))
        .collect();
    said(&json!({ "memories": memories }).to_string())
}

#[tokio::test]
async fn slash_dream_extracts_the_session_merges_in_the_background_and_says_how_much() {
    let server = Server::start(vec![
        said("好。"),
        said("养猫"),
        every_turn("用户养了一只猫"),
        slow(
            &json!({"summary": "用户养猫。"}).to_string(),
            Duration::from_millis(500),
        ),
    ])
    .await;
    let home = Home::new();
    let mut client = connected(&home, &server).await;
    let session = client
        .create_with(
            "c1",
            json!({"cwd": "~", "persona": "engineer", "preset": "full"}),
        )
        .await;
    client.say("s1", &session, "我养了一只猫").await;
    // 答完头一轮起标题，也照 `models.chat` 发：起好了再叫，请求的先后才定。
    until("起好标题", || {
        home.log(&session)
            .iter()
            .any(|event| matches!(event.body, Body::MetaChanged(_)))
    })
    .await;
    let subscribed = client.subscribe("w1", &session).await;
    assert!(subscribed.get("result").is_some(), "{subscribed}");
    let line = |id: &str, method: &str, params: Value| {
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
    };
    client
        .line(&line(
            "k1",
            "command.run",
            json!({"session": session, "text": "/dream"}),
        ))
        .await;
    client
        .line(&line("k2", "command.catalog", json!({"session": session})))
        .await;
    let mut replies = Vec::new();
    let mut ran_before_reply = false;
    while replies.len() < 2 {
        let message = client.next().await.expect("连着");
        if message["id"].is_string() {
            replies.push(message);
        } else if message["params"]["event"]["kind"] == "command.ran" {
            ran_before_reply = replies.iter().all(|reply| reply["id"] != "k1");
        }
    }
    assert!(ran_before_reply, "先见结果、后见回应：command.ran 先到");
    let [catalog, dreamed] = replies.as_slice() else {
        unreachable!("两条")
    };
    assert_eq!(catalog["id"], "k2", "后面的请求先回");
    assert!(
        catalog["result"]["commands"]
            .as_array()
            .expect("有")
            .iter()
            .any(|command| command["name"] == "dream"),
        "{catalog}"
    );
    assert_eq!(dreamed["result"]["command"], "dream", "{dreamed}");
    assert_eq!(
        dreamed["result"]["said"],
        "整理完了：看了 1 条，改了 0 条，作废 0 条，摘要更新了。"
    );
    assert_eq!(
        server.received().len(),
        4,
        "聊了一轮、起了标题、抽了一次、合了一次"
    );
    assert!(
        home.log(&session)
            .iter()
            .any(|event| matches!(&event.body, Body::CommandRan(ran) if ran.command == "dream"))
    );
    client
        .line(&line(
            "k3",
            "command.run",
            json!({"session": session, "text": "/dream"}),
        ))
        .await;
    let (pushed, again) = client.until_reply("k3").await;
    assert_eq!(again["result"]["said"], "没有要整理的。", "{again}");
    assert!(
        pushed
            .iter()
            .any(|push| push["params"]["event"]["cause"] == "k3/ran"),
        "{pushed:?}"
    );
}
