//! HTTP 的日志带上会话编号（`docs/construction/3-7-会话actor（下）.md` 验收第 2 条）：请求在派出去的任务里
//! 发，任务带着会话的 span，`sent`、`ended` 两行写在会话编号后面。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，
//! 这里装的订阅者可能漏听。

mod support;

use std::path::PathBuf;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_log::{LevelFilter, Memory};
use support::{Home, ask, say, until_turn_ends, watch};

#[tokio::test]
async fn the_http_lines_carry_the_session() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::DEBUG,
        None,
    ));
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/drivers/openai-chat/streams/openai-text.sse");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
    let server = Server::start(vec![Reply::stream(vec![Piece::Bytes(bytes)])]).await;
    let (models, configs) = support::routing::served(&server.base_url, serde_json::json!({}), "");
    let mut home = Home::new();
    home.configs = configs;
    let handle = home.create(&models).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;

    let lines = memory.lines();
    let session = handle.id().as_str();
    for wanted in [
        format!(" DEBUG http     {session} sent host=127.0.0.1 bytes="),
        format!(" DEBUG http     {session} ended host=127.0.0.1 status=200 took_ms="),
    ] {
        assert!(
            lines.iter().any(|line| line.contains(&wanted)),
            "「{wanted}」应该在 {lines:#?} 里"
        );
    }
    assert!(
        !lines.iter().any(|line| line.contains("sk-test")),
        "key 不进日志：{lines:#?}"
    );
}
