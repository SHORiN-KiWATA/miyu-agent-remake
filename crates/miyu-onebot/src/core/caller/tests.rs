//! 并着发的调用口（施工 O-23 下，`onebot.md`「施工时定的」第 86 条）：几个一起等，回应照编号各拿各的，先后不要紧；别的回应、
//! 推送原样交给跟核心的那一头；不等了的编号拿掉，回应晚来了丢掉；读的一头停了，等着的和再来的都交回断开。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader, DuplexStream};

use super::{Caller, Gone, Waiting};

/// 编号的前缀。
const PREFIX: &str = "onebot-ab-side-";

/// 一个调用口，写的一头接在内存里的管道上：交回它、等着的表、核心那一头读请求的一行行。
fn caller() -> (Caller, Waiting, BufReader<DuplexStream>) {
    let (ours, theirs) = tokio::io::duplex(64 * 1024);
    let waiting = Waiting::new(PREFIX.to_string());
    let writer: super::Writer = Arc::new(tokio::sync::Mutex::new(Box::new(ours)));
    (
        Caller::new(writer, waiting.clone()),
        waiting,
        BufReader::new(theirs),
    )
}

/// 核心那一头读到的下一条请求。
async fn request(lines: &mut BufReader<DuplexStream>) -> Value {
    let mut line = String::new();
    lines.read_line(&mut line).await.expect("读得到");
    serde_json::from_str(&line).expect("是 JSON")
}

/// 给编号 `id` 的回应。
fn reply(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

#[tokio::test]
async fn each_gets_its_own_answer_whatever_the_order() {
    let (caller, waiting, mut lines) = caller();
    let one = tokio::spawn({
        let caller = caller.clone();
        async move { caller.call("venue.records", json!({"n": 1})).await }
    });
    let first = request(&mut lines).await;
    let two = tokio::spawn({
        let caller = caller.clone();
        async move { caller.call("model.call", json!({"n": 2})).await }
    });
    let second = request(&mut lines).await;
    assert_eq!(first["method"], "venue.records");
    assert_eq!(first["params"], json!({"n": 1}));
    assert_eq!(first["jsonrpc"], "2.0");
    let id = first["id"].as_str().expect("是字");
    assert!(id.starts_with(PREFIX), "{id}");
    assert_ne!(first["id"], second["id"], "编号各是各的");
    // 后发的先回。
    assert_eq!(waiting.sort(reply(&second["id"], json!("two"))), None);
    assert_eq!(waiting.sort(reply(&first["id"], json!("one"))), None);
    let one = one.await.expect("没崩").expect("回了");
    let two = two.await.expect("没崩").expect("回了");
    assert_eq!(one["result"], "one");
    assert_eq!(two["result"], "two");
}

#[tokio::test]
async fn other_messages_pass_through_and_late_answers_are_dropped() {
    let (_, waiting, _) = caller();
    let pushed = json!({"jsonrpc": "2.0", "method": "event", "params": {"session": "s"}});
    assert_eq!(waiting.sort(pushed.clone()), Some(pushed), "推送原样交回");
    let route = reply(&json!("onebot-ab-7"), json!({}));
    assert_eq!(
        waiting.sort(route.clone()),
        Some(route),
        "跟核心的那一头的回应原样交回"
    );
    let numbered = reply(&json!(7), json!({}));
    assert_eq!(waiting.sort(numbered.clone()), Some(numbered));
    let late = reply(&json!(format!("{PREFIX}99")), json!({}));
    assert_eq!(waiting.sort(late), None, "没人等的丢掉");
    // 带 `method` 的是核心发来的请求（反向调用这类），编号碰巧像也不是这里的回应。
    let asked = json!({"jsonrpc": "2.0", "id": format!("{PREFIX}1"), "method": "x"});
    assert_eq!(waiting.sort(asked.clone()), Some(asked));
}

#[tokio::test]
async fn a_call_given_up_forgets_its_number() {
    let (caller, waiting, mut lines) = caller();
    let given_up = tokio::time::timeout(
        Duration::from_millis(1),
        caller.call("model.call", json!({})),
    )
    .await;
    assert!(given_up.is_err(), "没人回，等到时限");
    let sent = request(&mut lines).await;
    assert!(
        waiting
            .lock()
            .as_ref()
            .is_some_and(|table| table.is_empty()),
        "不等了的编号拿掉了"
    );
    assert_eq!(
        waiting.sort(reply(&sent["id"], json!({}))),
        None,
        "晚来的丢掉"
    );
}

#[tokio::test]
async fn once_reading_stops_every_call_is_gone() {
    let (caller, waiting, mut lines) = caller();
    let waiter = tokio::spawn({
        let caller = caller.clone();
        async move { caller.call("model.call", json!({})).await }
    });
    request(&mut lines).await;
    waiting.close();
    assert!(
        matches!(waiter.await.expect("没崩"), Err(Gone)),
        "等着的断开"
    );
    assert!(
        matches!(caller.call("model.call", json!({})).await, Err(Gone)),
        "再来的断开"
    );
}

#[tokio::test]
async fn a_closed_pipe_is_gone() {
    let (caller, _waiting, lines) = caller();
    drop(lines);
    assert!(matches!(
        caller.call("model.call", json!({})).await,
        Err(Gone)
    ));
}

/// 同一条连接上另拿的调用口（`Core::caller` 每次造一个：登记工具、登记方法、问判官各拿一份）编号也不撞：撞了的回应交错了人
/// （施工 O-28 上碰到：登记方法和登记工具都编成 `-side-1`，登记方法的回应被当成登记工具的）。
#[tokio::test]
async fn callers_on_one_connection_never_share_a_number() {
    let (caller, waiting, mut lines) = caller();
    let other = Caller::new(Arc::clone(&caller.writer), waiting.clone());
    let one = caller.send("provide", json!({})).await.expect("写得出");
    let first = request(&mut lines).await;
    let two = other
        .send("package.methods", json!({}))
        .await
        .expect("写得出");
    let second = request(&mut lines).await;
    assert_ne!(first["id"], second["id"], "编号各是各的");
    assert_eq!(waiting.sort(reply(&second["id"], json!("two"))), None);
    assert_eq!(waiting.sort(reply(&first["id"], json!("one"))), None);
    assert_eq!(one.wait().await.expect("回了")["result"], "one");
    assert_eq!(two.wait().await.expect("回了")["result"], "two");
}
