//! 路由接上 Anthropic 的消息接口（`docs/blueprint/drivers/anthropic.md`、`models.md`「驱动要守的约定」第 2、11、13 条，施工
//! 8-12）：驱动照供应商的 `driver` 造，发到 `/messages`，带 `x-api-key` 和版本头；输出上限照一次性入口写的、再照模型资料的最大
//! 输出、再是 8192，openai-chat 照旧不写；思考强度照这一家的写法；回来的流照这一家解。
//!
//! 假服务器在本机回环上，档案是空的、没有目录：资料全照手写的。

mod support;

use std::time::Duration;

use miyu_config::secret::Reference;
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::event::Usage;
use miyu_session::Answer;
use serde_json::json;
use support::calling::{asking, blobs, body, entry, frozen};
use support::routing::{hellos, routes};

/// Anthropic 说「你好！」的流，`n` 份。
fn anthropic_hellos(n: usize) -> Vec<Reply> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/drivers/anthropic/streams/text.sse");
    let bytes = std::fs::read(&path).expect("样本读得到");
    (0..n)
        .map(|_| Reply::stream(vec![Piece::Bytes(bytes.clone())]))
        .collect()
}

/// 一家 `claude` 在 `base_url`，驱动 `anthropic`，key 是 `{ env = "CLAUDE_KEY" }`；`models.chat` 是 `claude/m`。`models` 接在
/// 后面：模型手写的资料。
fn config(base_url: &str, models: &str) -> String {
    format!(
        "[providers.claude]\ndriver = \"anthropic\"\nbase_url = \"{base_url}\"\nkeys = [{{ env = \"CLAUDE_KEY\" }}]\n\n{models}\n[models]\nchat = \"claude/m\"\n"
    )
}

fn key() -> Vec<(Reference, &'static str)> {
    vec![(Reference::Env("CLAUDE_KEY".to_string()), "sk-ant-test")]
}

#[tokio::test]
async fn a_call_goes_to_messages_with_the_anthropic_headers_and_is_decoded() {
    let server = Server::start(anthropic_hellos(1)).await;
    let config = frozen(&config(&server.base_url, ""), &key());
    let routes = routes(json!({}), Duration::from_secs(60));
    let (_scratch, blobs) = blobs();
    let answered = entry(&routes)
        .call(&config, &blobs, asking(None, "platform", "hi"))
        .await;
    assert_eq!(
        answered,
        Ok(Answer {
            text: "你好！".to_string(),
            provider: "claude".to_string(),
            model: "m".to_string(),
            usage: Some(Usage {
                uncached: 25,
                cache_read: 3000,
                cache_write: 1200,
                output: 12,
            }),
        })
    );
    let received = &server.received()[0];
    assert!(received.path.ends_with("/messages"), "{}", received.path);
    assert_eq!(received.header("x-api-key"), Some("sk-ant-test"));
    assert_eq!(received.header("anthropic-version"), Some("2023-06-01"));
    assert_eq!(received.header("authorization"), None, "不写 Bearer");
    let sent = body(&server, 0);
    assert_eq!(sent["max_tokens"], 8192, "资料没有最大输出的照驱动的兜底");
    assert_eq!(sent["stream"], true);
    assert_eq!(
        sent["messages"],
        json!([{"role": "user", "content": [
            {"type": "text", "text": "hi", "cache_control": {"type": "ephemeral"}}
        ]}])
    );
}

#[tokio::test]
async fn the_output_limit_comes_from_the_call_then_the_model_facts() {
    let server = Server::start(anthropic_hellos(2)).await;
    let models = "[providers.claude.models.m]\nmax_output = 32000\n";
    let config = frozen(&config(&server.base_url, models), &key());
    let routes = routes(json!({}), Duration::from_secs(60));
    let (_scratch, blobs) = blobs();
    let entry = entry(&routes);
    let _answered = entry
        .call(&config, &blobs, asking(None, "platform", "hi"))
        .await;
    let mut limited = asking(None, "platform", "hi");
    limited.max_tokens = Some(100);
    let _answered = entry.call(&config, &blobs, limited).await;
    assert_eq!(
        body(&server, 0)["max_tokens"],
        32000,
        "照模型资料的最大输出"
    );
    assert_eq!(body(&server, 1)["max_tokens"], 100, "一次性入口写了的照它");
}

#[tokio::test]
async fn openai_chat_still_leaves_the_output_limit_out() {
    let server = Server::start(hellos(1)).await;
    let text = format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.m]\nmax_output = 32000\n\n[models]\nchat = \"a/m\"\n",
        server.base_url
    );
    let config = frozen(&text, &[]);
    let routes = routes(json!({}), Duration::from_secs(60));
    let (_scratch, blobs) = blobs();
    let _answered = entry(&routes)
        .call(&config, &blobs, asking(None, "platform", "hi"))
        .await;
    assert!(body(&server, 0).get("max_tokens").is_none());
}

#[tokio::test]
async fn the_configured_effort_is_written_the_anthropic_way() {
    let server = Server::start(anthropic_hellos(1)).await;
    let models = "[providers.claude.models.m]\nreasoning = [\"off\", \"low\", \"high\"]\neffort = \"high\"\n";
    let config = frozen(&config(&server.base_url, models), &key());
    let routes = routes(json!({}), Duration::from_secs(60));
    let (_scratch, blobs) = blobs();
    let _answered = entry(&routes)
        .call(&config, &blobs, asking(None, "platform", "hi"))
        .await;
    let sent = body(&server, 0);
    assert_eq!(
        sent["thinking"],
        json!({"type": "adaptive", "display": "summarized"})
    );
    assert_eq!(sent["output_config"], json!({"effort": "high"}));
    assert!(sent.get("reasoning_effort").is_none());
}
