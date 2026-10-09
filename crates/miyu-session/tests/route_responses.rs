//! 路由接上 OpenAI 的 Responses 接口（`docs/blueprint/drivers/openai-responses.md`、`models.md`「驱动要守的约定」第 2、11、13
//! 条，施工 8-13）：驱动照供应商的 `driver` 造，发到 `/responses`，带 `Bearer`；输出上限照旧不替它填；思考强度照这一家的写法；
//! 回来的流照这一家解。
//!
//! 假服务器在本机回环上，档案是空的、没有目录：资料全照手写的。

use std::time::Duration;

use crate::support::calling::{asking, bearer, blobs, body, entry, frozen};
use crate::support::routing::routes;
use miyu_config::secret::Reference;
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::event::Usage;
use miyu_session::Answer;
use serde_json::json;

/// Responses 说「你好！」的流，`n` 份。
fn hellos(n: usize) -> Vec<Reply> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/drivers/openai-responses/streams/text.sse");
    let bytes = std::fs::read(&path).expect("样本读得到");
    (0..n)
        .map(|_| Reply::stream(vec![Piece::Bytes(bytes.clone())]))
        .collect()
}

/// 一家 `gpt` 在 `base_url`，驱动 `openai-responses`，key 是 `{ env = "GPT_KEY" }`；`models.chat` 是 `gpt/m`。`models` 接在后面。
fn config(base_url: &str, models: &str) -> String {
    format!(
        "[providers.gpt]\ndriver = \"openai-responses\"\nbase_url = \"{base_url}\"\nkey = {{ env = \"GPT_KEY\" }}\n\n{models}\n[models]\nchat = \"gpt/m\"\n"
    )
}

fn key() -> Vec<(Reference, &'static str)> {
    vec![(Reference::Env("GPT_KEY".to_string()), "sk-test")]
}

#[tokio::test]
async fn a_call_goes_to_responses_with_a_bearer_and_is_decoded() {
    let server = Server::start(hellos(2)).await;
    let models = "[providers.gpt.models.m]\nmax_output = 32000\n";
    let config = frozen(&config(&server.base_url, models), &key());
    let routes = routes(json!({}), Duration::from_secs(60));
    let (_scratch, blobs) = blobs();
    let entry = entry(&routes);
    let answered = entry
        .call(&config, &blobs, asking(None, "platform", "hi"))
        .await;
    assert_eq!(
        answered,
        Ok(Answer {
            text: "你好！".to_string(),
            provider: "gpt".to_string(),
            model: "m".to_string(),
            usage: Some(Usage {
                uncached: 80,
                cache_read: 1920,
                cache_write: 0,
                output: 3,
                reasoning: None,
            }),
        })
    );
    let received = &server.received()[0];
    assert!(received.path.ends_with("/responses"), "{}", received.path);
    assert_eq!(bearer(&server, 0).as_deref(), Some("Bearer sk-test"));
    let sent = body(&server, 0);
    assert_eq!(sent["store"], false);
    assert_eq!(sent["input"], json!([{"role": "user", "content": "hi"}]));
    assert!(
        sent.get("max_output_tokens").is_none(),
        "资料有最大输出也不替它填"
    );
    let mut limited = asking(None, "platform", "hi");
    limited.max_tokens = Some(100);
    let _answered = entry.call(&config, &blobs, limited).await;
    assert_eq!(body(&server, 1)["max_output_tokens"], 100, "写了的照它");
}

#[tokio::test]
async fn the_configured_effort_is_written_the_responses_way() {
    let server = Server::start(hellos(1)).await;
    let models =
        "[providers.gpt.models.m]\nreasoning = [\"off\", \"low\", \"high\"]\neffort = \"low\"\n";
    let config = frozen(&config(&server.base_url, models), &key());
    let routes = routes(json!({}), Duration::from_secs(60));
    let (_scratch, blobs) = blobs();
    let _answered = entry(&routes)
        .call(&config, &blobs, asking(None, "platform", "hi"))
        .await;
    let sent = body(&server, 0);
    assert_eq!(
        sent["reasoning"],
        json!({"effort": "low", "summary": "auto"})
    );
    assert_eq!(sent["include"], json!(["reasoning.encrypted_content"]));
    assert!(sent.get("reasoning_effort").is_none());
}
