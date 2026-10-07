//! 钉着的没了、退回默认的那一行运行日志（`docs/blueprint/models.md`「出错」运行时那张表、`log.md`，施工 8-10）：
//! `model fallback`，写原来的和退回的，带会话编号。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能漏听。

mod support;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;

use miyu_http::testkit::Server;
use miyu_log::{LevelFilter, Memory};
use miyu_session::ConfigSource;
use miyu_tool::Catalog;
use support::routing::{configs, hellos, routes, turn};
use support::{Home, Lines, Opening};

#[tokio::test]
async fn a_fallback_is_logged_with_both_references() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    let server = Server::start(hellos(4)).await;
    let provider = |id: &str| {
        format!(
            "[providers.{id}]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n",
            server.base_url
        )
    };
    let source =
        |text: String| -> Arc<dyn ConfigSource> { Arc::clone(&*configs(&text, &[]).borrow()) };
    let both = format!(
        "{}{}[models]\nchat = \"b/m\"\n",
        provider("a"),
        provider("b")
    );
    let (switch, receiving) = watch::channel(source(both));
    let mut home = Home::new();
    home.configs = receiving;
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let lines = Lines {
        model: Some("a/x".to_string()),
        ..Lines::default()
    };
    let handle = home
        .create_full(&routes, &Catalog::default(), Opening::default(), lines)
        .await;
    switch.send_replace(source(format!(
        "{}[models]\nchat = \"b/m\"\n",
        provider("b")
    )));
    turn(&handle, "cmd-1").await;

    let lines = memory.lines();
    let wanted = format!(
        " INFO  session  {} model fallback from=a/x to=b/m",
        handle.id().as_str()
    );
    assert!(
        lines.iter().any(|line| line.contains(&wanted)),
        "「{wanted}」应该在 {lines:#?} 里"
    );
}
