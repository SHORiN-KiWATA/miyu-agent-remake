//! `provider.test` 的运行日志（施工 8-11，`docs/blueprint/models.md`「出错」、「怎么走」第七条第 4 条第 6、7 款）：试一次记
//! 一行 `INFO provider tested`；候选的 `{value}` 的 key 只在那一次的内存里，整份运行日志（`TRACE`）、回应里都搜不到。
//!
//! 全局装一个写进内存的订阅者，一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use serde_json::json;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_log::{LevelFilter, Memory};
use support::providers::{core, data, profiles};
use support::*;

/// 一眼看得出是假的 key。
const FAKE: &str = "sk-FAKE-KEY-FOR-TESTS-0001";

#[tokio::test]
async fn a_test_is_one_line_and_the_pasted_key_is_nowhere() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::TRACE,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let words = |text: &str| {
        let event = json!({"id": "c1", "object": "chat.completion.chunk", "model": "x",
            "choices": [{"index": 0, "delta": {"content": text}, "finish_reason": null}]});
        Piece::Bytes(format!("data: {event}\n\n").into_bytes())
    };
    let listing = json!({"object": "list", "data": [{"id": "deepseek-flash"}]}).to_string();
    let server = Server::start(vec![
        Reply::stream(vec![Piece::Bytes(listing.into_bytes())]),
        Reply::stream(vec![words("OK"), Piece::Stall]),
        Reply::error(401, &[], r#"{"error":{"message":"bad key"}}"#),
        Reply::error(401, &[], r#"{"error":{"message":"bad key"}}"#),
    ])
    .await;
    let home = Home::new();
    let core = core(&home, &[], data(profiles(json!({}))));
    let mut client = Client::connect(core);
    client.hello().await;
    let candidate = |key: &str| json!({"candidate": {"catalog": "deepseek", "base_url": server.base_url, "key": {"value": key}}});
    let worked = client
        .call("test-1", "provider.test", candidate(FAKE))
        .await;
    assert_eq!(worked["result"]["ok"], true, "{worked}");
    let failed = client
        .call(
            "test-2",
            "provider.test",
            json!({"candidate": {"driver": "openai-chat", "base_url": server.base_url,
                "key": {"value": FAKE}}, "model": "m"}),
        )
        .await;
    assert_eq!(failed["result"]["ok"], false, "{failed}");
    for reply in [&worked, &failed] {
        assert!(!reply.to_string().contains("FAKE"), "{reply}");
    }
    let lines = memory.lines();
    let has = |what: &str| lines.iter().any(|line: &String| line.contains(what));
    assert!(
        has("provider tested provider=deepseek model=deepseek-flash ok=true"),
        "{lines:?}"
    );
    assert!(
        has("provider tested provider=candidate model=m ok=false"),
        "{lines:?}"
    );
    let log = lines.join("\n");
    assert!(!log.contains("FAKE"), "运行日志里有 key：{log}");
}
