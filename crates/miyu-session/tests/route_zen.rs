//! 路由接 opencode 的 Go、Zen（`docs/blueprint/models.md`「怎么走」第八条、`drivers/openai-chat.md`「接 opencode Zen」，施工
//! 8-14）：只写 key，驱动、地址照档案和目录推；同一家的模型照目录各走各的驱动，发到各自的路径；没有驱动的模型当场 `no_model`，
//! 别的照常；请求带档案另配的头，值照种子换（会话是会话编号，一次性的是用途）；交错思考的照目录回传；8-14 补：Zen 免费档的
//! `User-Agent` 和占位工具（工具面里缺 `shell`、`read` 时补）照档案。
//!
//! 假服务器在本机回环上，手写的地址指着它；编号和目录里一样，照编号认出是目录里的那一家。

mod support;

use std::sync::Arc;
use std::time::Duration;

use miyu_config::secret::Reference;
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::block::{Block, Text};
use miyu_kernel::request::Message;
use miyu_models::catalog::{Catalog, CatalogSource, Loaded};
use miyu_models::headers::digest;
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_session::{ModelData, Observed, Routes, Unanswered};
use serde_json::json;
use support::Home;
use support::calling::{asking, blobs, body, entry, frozen, user};
use support::routing::{configs, routes_with, turn};

/// 真目录裁出来的一份（`miyu-models` 的测试也用它）：Go 有走三种驱动的模型，Zen 有一个走 Google 的。
fn trimmed() -> Loaded {
    let text = include_str!("../../miyu-models/testdata/models-dev-trimmed.json");
    Loaded {
        catalog: Catalog::parse(text).expect("读得进").catalog,
        source: CatalogSource::Snapshot,
        fetched: "2026-10-01T03:25:54.000Z".to_string(),
    }
}

/// 档案照出厂的写法：`[npm]` 和 Go 的头。目录读完了。
fn zen_routes() -> Routes {
    let profiles = json!({
        "npm": {"@ai-sdk/openai-compatible": "openai-chat", "@ai-sdk/anthropic": "anthropic",
                "@ai-sdk/openai": "openai-responses"},
        "providers": {
            "opencode-go": {"headers": {"x-opencode-session": "ses_{session_digest}"}},
            "opencode": {
                "headers": {"User-Agent": "opencode/2.0.21", "x-opencode-client": "cli",
                            "x-opencode-project": "global", "x-opencode-session": "ses_{session_digest}"},
                "placeholder_tools": ["read", "shell"]
            }
        }
    });
    let data = ModelData::new(
        Profiles::parse(&profiles).expect("档案写法对"),
        Vendors::default(),
        None,
    )
    .with_placeholder_tool(
        include_str!("../../../resources/core/drivers/placeholder-tool.txt").to_string(),
    );
    data.loaded(Some(trimmed()), Observed::default());
    routes_with(Arc::new(data), Duration::from_secs(60))
}

/// `opencode-go`、`opencode` 两家都在 `base_url`，只写 key；`models.chat` 是 `chat`。
fn source(base_url: &str, chat: &str) -> String {
    format!(
        "[providers.opencode-go]\nbase_url = \"{base_url}\"\nkeys = [{{ env = \"GO_KEY\" }}]\n\n[providers.opencode]\nbase_url = \"{base_url}\"\nkeys = [{{ env = \"GO_KEY\" }}]\n\n[models]\nchat = \"{chat}\"\n"
    )
}

fn key() -> Vec<(Reference, &'static str)> {
    vec![(Reference::Env("GO_KEY".to_string()), "sk-go")]
}

/// 驱动 `driver` 的样本里说「你好！」的那份流。
fn hello(driver: &str) -> Reply {
    let file = match driver {
        "openai-chat" => "openai-chat/streams/openai-text.sse",
        other => &format!("{other}/streams/text.sse"),
    };
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/drivers")
        .join(file);
    Reply::stream(vec![Piece::Bytes(
        std::fs::read(&path).expect("样本读得到"),
    )])
}

fn session_header(server: &Server, at: usize) -> Option<String> {
    server.received()[at]
        .header("x-opencode-session")
        .map(str::to_string)
}

#[tokio::test]
async fn each_go_model_goes_through_its_own_driver_with_the_session_header() {
    let server = Server::start(vec![
        hello("anthropic"),
        hello("openai-responses"),
        hello("openai-chat"),
    ])
    .await;
    let config = frozen(
        &source(&server.base_url, "opencode-go/deepseek-v4.1-flash"),
        &key(),
    );
    let routes = zen_routes();
    let (_scratch, blobs) = blobs();
    let entry = entry(&routes);
    for model in ["minimax-m3", "gpt-5.6-luna", "deepseek-v4.1-flash"] {
        let reference = format!("opencode-go/{model}");
        let answered = entry
            .call(&config, &blobs, asking(Some(&reference), "title", "hi"))
            .await
            .expect("答得上来");
        assert_eq!(answered.text, "你好！", "{model}");
        assert_eq!(answered.model, model);
    }
    let received = server.received();
    let paths: Vec<&str> = received.iter().map(|each| each.path.as_str()).collect();
    assert!(paths[0].ends_with("/messages"), "{paths:?}");
    assert!(paths[1].ends_with("/responses"), "{paths:?}");
    assert!(paths[2].ends_with("/chat/completions"), "{paths:?}");
    assert_eq!(received[0].header("x-api-key"), Some("sk-go"));
    assert_eq!(received[0].header("authorization"), None);
    assert_eq!(received[1].header("authorization"), Some("Bearer sk-go"));
    assert_eq!(received[2].header("authorization"), Some("Bearer sk-go"));
    let wanted = format!("ses_{}", digest("title"));
    for at in 0..3 {
        assert_eq!(
            session_header(&server, at).as_deref(),
            Some(wanted.as_str()),
            "一次性的照用途"
        );
    }
    assert_eq!(
        body(&server, 0)["max_tokens"],
        131_072,
        "照模型的驱动填输出上限：目录的最大输出"
    );
    assert!(body(&server, 2).get("max_tokens").is_none());
}

#[tokio::test]
async fn interleaved_thinking_is_replayed_for_go_deepseek() {
    let server = Server::start(vec![hello("openai-chat")]).await;
    let config = frozen(
        &source(&server.base_url, "opencode-go/deepseek-v4.1-flash"),
        &key(),
    );
    let routes = zen_routes();
    let (_scratch, blobs) = blobs();
    let mut ask = asking(None, "title", "hi");
    ask.messages = vec![
        user("hi"),
        Message::Assistant {
            blocks: vec![Block::Text(Text {
                text: "hello".to_string(),
            })],
        },
        user("again"),
    ];
    entry(&routes)
        .call(&config, &blobs, ask)
        .await
        .expect("答得上来");
    let sent = body(&server, 0);
    assert_eq!(sent["messages"][1]["role"], "assistant");
    assert_eq!(
        sent["messages"][1]["reasoning_content"], "",
        "没有思考的 assistant 也带空串"
    );
}

#[tokio::test]
async fn a_model_without_a_driver_is_no_model_and_the_rest_still_work() {
    let server = Server::start(vec![hello("openai-chat")]).await;
    let config = frozen(
        &source(&server.base_url, "opencode-go/deepseek-v4.1-flash"),
        &key(),
    );
    let routes = zen_routes();
    let (_scratch, blobs) = blobs();
    let entry = entry(&routes);
    let google = entry
        .call(
            &config,
            &blobs,
            asking(Some("opencode/gemini-3-pro"), "title", "hi"),
        )
        .await;
    assert_eq!(
        google,
        Err(Unanswered::NoModel(
            r#"model "opencode/gemini-3-pro" needs driver "@ai-sdk/google", which is not available yet"#
                .to_string()
        ))
    );
    assert!(server.received().is_empty(), "没发");
    let answered = entry
        .call(&config, &blobs, asking(None, "title", "hi"))
        .await
        .expect("别的照常");
    assert_eq!(answered.provider, "opencode-go");
    assert_eq!(
        server.received()[0].header("x-opencode-session"),
        Some(format!("ses_{}", digest("title")).as_str())
    );
    // 没写头、没写占位的一家：不带头，也不补（另造一份两样都没有的档案）。
    let plain_profiles =
        json!({"npm": {"@ai-sdk/openai-compatible": "openai-chat"}, "providers": {}});
    let plain_data = ModelData::new(
        Profiles::parse(&plain_profiles).expect("档案写法对"),
        Vendors::default(),
        None,
    );
    plain_data.loaded(Some(trimmed()), Observed::default());
    let plain_routes = routes_with(Arc::new(plain_data), Duration::from_secs(60));
    let plain = Server::start(vec![hello("openai-chat")]).await;
    let config = frozen(
        &source(&plain.base_url, "opencode/deepseek-v4.1-flash"),
        &key(),
    );
    let plain_entry = support::calling::entry(&plain_routes);
    plain_entry
        .call(&config, &blobs, asking(None, "title", "hi"))
        .await
        .expect("答得上来");
    assert_eq!(plain.received()[0].header("x-opencode-session"), None);
    assert!(
        plain.received()[0]
            .header("user-agent")
            .is_some_and(|ua| ua.starts_with("miyu/")),
        "没写头照客户端的默认"
    );
    assert!(body(&plain, 0).get("tools").is_none(), "没点名的不补");
}

#[tokio::test]
async fn a_session_sends_its_own_digest() {
    let server = Server::start(vec![hello("openai-chat"), hello("openai-chat")]).await;
    let mut home = Home::new();
    home.configs = configs(
        &source(&server.base_url, "opencode-go/deepseek-v4.1-flash"),
        &key(),
    );
    let handle = home.create(&zen_routes()).await;
    turn(&handle, "cmd-1").await;
    assert_eq!(
        session_header(&server, 0),
        Some(format!("ses_{}", digest(handle.id().as_str()))),
        "会话照会话编号"
    );
}

/// Zen 免费档（施工 8-14 补，2026-10-04 实测）：User-Agent 盖成 opencode 的形状，另配三个头；工具面里没有
/// `shell`、`read` 的补占位——免费档要这两件才放行。
#[tokio::test]
async fn the_zen_free_tier_headers_and_placeholders_go_out() {
    let server = Server::start(vec![hello("openai-chat")]).await;
    let config = frozen(
        &source(&server.base_url, "opencode/deepseek-v4.1-flash"),
        &key(),
    );
    let routes = zen_routes();
    let (_scratch, blobs) = blobs();
    entry(&routes)
        .call(&config, &blobs, asking(None, "title", "hi"))
        .await
        .expect("答得上来");
    let received = &server.received()[0];
    assert_eq!(
        received.header("user-agent"),
        Some("opencode/2.0.21"),
        "User-Agent 盖掉客户端默认的 miyu/<版本>"
    );
    assert_eq!(received.header("x-opencode-client"), Some("cli"));
    assert_eq!(received.header("x-opencode-project"), Some("global"));
    assert_eq!(
        received.header("x-opencode-session"),
        Some(format!("ses_{}", digest("title")).as_str()),
        "一次性的照用途"
    );
    let sent = body(&server, 0);
    let tools = sent["tools"].as_array().expect("补了占位，工具面在");
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["function"]["name"].as_str().expect("名字"))
        .collect();
    assert_eq!(names, ["read", "shell"], "缺的两件照名字排着补上");
    let description = tools[0]["function"]["description"].as_str().expect("说明");
    assert!(description.starts_with("Placeholder"), "{description}");
    assert_eq!(
        tools[0]["function"]["parameters"],
        json!({"type": "object", "properties": {}}),
        "占位的参数格式是空的"
    );
}
